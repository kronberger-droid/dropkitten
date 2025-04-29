use clap::Parser;
use futures::StreamExt;
use shell_escape::unix::escape;
use std::env;
use swayipc::{
    Connection, EventType,
    async_std::println,
    reply::{Event, Output, WindowChange},
};
use thiserror::Error;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    #[arg(trailing_var_arg = true)]
    command: Vec<String>,
}

#[derive(Debug, Error)]
enum AppError {
    #[error("Sway IPC error: {0}")]
    Swayipc(String),
    #[error("Environment error: {0}")]
    Env(#[from] env::VarError),
    #[error("No active output detected")]
    NoOutput,
}

impl From<swayipc::Error> for AppError {
    fn from(e: swayipc::Error) -> Self {
        AppError::Swayipc(e.to_string())
    }
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let cli = Cli::parse();

    let mut conn = Connection::new().await?;

    let (w, h) = compute_dimensions(&mut conn).await?;

    apply_rules(&mut conn, w, h).await?;

    conn.run_command("focus mouse_warping none").await?;

    spawn_dropdown(&mut conn, &cli.command).await?;

    conn.run_command("focus mouse_warping container").await?;

    let subs_conn = Connection::new().await?;

    let mut events = subs_conn.subscribe(&[EventType::Window]).await?;

    while let Some(msg) = events.next().await {
        let Event::Window(ev) = msg? else { continue };

        if ev.change == WindowChange::Focus && ev.container.app_id.as_deref() != Some("dropdown") {
            conn.run_command("[app_id=\"dropdown\"] kill").await?;
            break;
        }
    }

    Ok(())
}

async fn compute_dimensions(conn: &mut Connection) -> Result<(i32, i32), AppError> {
    let outputs: Vec<Output> = conn.get_outputs().await?;
    let out = outputs
        .into_iter()
        .find(|o| o.active)
        .ok_or(AppError::NoOutput)?;
    let w = (out.rect.width as f32 * 0.20).round() as i32;
    let h = (out.rect.height as f32 * 0.40).round() as i32;
    println!("width: {:?}, height: {:?}", w, h).await;
    Ok((w, h))
}

async fn apply_rules(conn: &mut Connection, w: i32, h: i32) -> Result<(), AppError> {
    conn.run_command(format!(
        "for_window [app_id=\"dropdown\"] floating enable, resize set {} {}",
        w, h
    ))
    .await?;
    conn.run_command("for_window [app_id=\"dropdown\"] move position cursor")
        .await?;
    conn.run_command("for_window [app_id=\"dropdown\"] move down 35")
        .await?;
    Ok(())
}

async fn spawn_dropdown(conn: &mut Connection, args: &[String]) -> Result<(), AppError> {
    let cmd_args = if args.is_empty() {
        vec![env::var("SHELL")?]
    } else {
        args.to_vec()
    };
    let mut cmd = String::from("exec kitty --class dropdown --");
    for a in cmd_args {
        cmd.push(' ');
        cmd.push_str(&escape(a.into()));
    }
    conn.run_command(cmd).await?;
    Ok(())
}
