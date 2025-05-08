use clap::Parser;
use futures::StreamExt;
use shell_escape::unix::escape;
use std::env;
use std::str::FromStr;
use swayipc::{
    Connection, EventType,
    async_std::println,
    reply::{Event, WindowChange},
};
use thiserror::Error;

#[derive(Debug, Clone)]
enum Size {
    Px(u32),
    Fr(f32),
}

impl FromStr for Size {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(px) = s.parse::<u32>() {
            Ok(Size::Px(px))
        } else if let Ok(fr) = s.parse::<f32>() {
            Ok(Size::Fr(fr))
        } else {
            Err("expect integer pixels or float fraction".into())
        }
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    /// window width (pixels or fraction)
    #[arg(short = 'W', long = "width")]
    width: Option<Size>,

    /// window height (pixels or fraction)
    #[arg(short = 'H', long = "height")]
    height: Option<Size>,

    /// sub-command to run + its args
    #[arg(last = true)]
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

    // read out current mouse_warping config waybe through warp enum
    conn.run_command("focus mouse_warping none").await?;

    spawn_dropdown(&mut conn, &cli).await?;

    // this needs to be read out in advace so it can be reset to the right one
    conn.run_command("focus mouse_warping container").await?;

    Ok(())
}

async fn focus_change_watcher(conn: &mut Connection) -> Result<(), AppError> {
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

fn resolve(opt: &Option<Size>, screen: i32, def_frac: f32) -> i32 {
    match opt {
        Some(Size::Px(px)) => *px as i32,
        Some(Size::Fr(fr)) => (screen as f32 * fr).round() as i32,
        None => (screen as f32 * def_frac).round() as i32,
    }
}

async fn compute_dimensions(conn: &mut Connection, opts: &Cli) -> Result<(i32, i32), AppError> {
    let out = conn
        .get_outputs()
        .await?
        .into_iter()
        .find(|o| o.active)
        .ok_or(AppError::NoOutput)?;
    Ok((
        resolve(&opts.width, out.rect.width as i32, 0.30),
        resolve(&opts.height, out.rect.height as i32, 0.40),
    ))
}

/// applies the rules for the app_id="dropdown" usign swayipc
async fn apply_rules(conn: &mut Connection, cli: &Cli) -> Result<(), AppError> {
    let (w, h) = compute_dimensions(conn, cli).await?;

    println!("width: {:?}, height: {:?}", w, h).await;

    conn.run_command("for_window [app_id=\"dropdown\"] floating enable")
        .await?;

    conn.run_command(format!(
        "for_window [app_id=\"dropdown\"] resize set {} {}",
        w, h
    ))
    .await?;

    conn.run_command("for_window [app_id=\"dropdown\"] move position cursor")
        .await?;

    conn.run_command("for_window [app_id=\"dropdown\"] move down 35")
        .await?;

    Ok(())
}

async fn spawn_dropdown(conn: &mut Connection, cli: &Cli) -> Result<(), AppError> {
    apply_rules(conn, cli).await?;

    let cmd_args: Vec<String> = if cli.command.is_empty() {
        Vec::new()
    } else {
        cli.command.clone()
    };

    let mut cmd = String::from("exec kitty --class dropdown --");

    for a in cmd_args {
        cmd.push(' ');
        cmd.push_str(&escape(a.into()));
    }
    conn.run_command(cmd).await?;

    focus_change_watcher(conn).await?;

    Ok(())
}
