use clap::{Parser, ValueEnum};
use futures::StreamExt;
use regex::Regex;
use shell_escape::unix::escape;
use std::env;
use std::str::FromStr;
use swayipc::{
    Connection, EventType,
    reply::{Event, WindowChange},
};
use thiserror::Error;

static APP_ID: &str = "test";

#[derive(Debug, Clone, ValueEnum)]
enum Terminal {
    Kitty,
    Alacritty,
    Rio,
}

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
    /// terminal to use
    #[arg(short = 't', long = "terminal", value_enum)]
    terminal: Terminal,

    /// window width (pixels or fraction)
    #[arg(short = 'W', long = "width")]
    width: Option<Size>,

    /// window height (pixels or fraction)
    #[arg(short = 'H', long = "height")]
    height: Option<Size>,

    #[arg(short = 'y', long = "yshift")]
    yshift: Option<Size>,

    #[arg(short = 'x', long = "xshift")]
    xshift: Option<Size>,

    #[arg(short = 'c', long = "center")]
    center: bool,

    /// subcommand to run + its arguments
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

    spawn_dropdown(&mut conn, &cli).await?;

    Ok(())
}

async fn get_mouse_warping(conn: &mut Connection) -> String {
    let config = match conn.get_config().await {
        Ok(cfg) => cfg.config,
        Err(_) => return "none".to_string(),
    };

    let re = Regex::new(r"mouse_warping\s+(\w+)").unwrap();

    re.captures(&config)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "none".to_string())
}

async fn focus_change_watcher(conn: &mut Connection) -> Result<(), AppError> {
    let subs_conn = Connection::new().await?;

    let mut events = subs_conn.subscribe(&[EventType::Window]).await?;

    while let Some(msg) = events.next().await {
        let Event::Window(ev) = msg? else { continue };

        if ev.change == WindowChange::Focus && ev.container.app_id.as_deref() != Some(APP_ID) {
            conn.run_command(format!("[app_id=\"{}\"] kill", APP_ID))
                .await?;
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

async fn compute_dimensions(
    conn: &mut Connection,
    opts: &Cli,
) -> Result<(i32, i32, i32, i32, i64, i64), AppError> {
    let out = conn
        .get_outputs()
        .await?
        .into_iter()
        .find(|o| o.active)
        .ok_or(AppError::NoOutput)?;
    Ok((
        resolve(&opts.width, out.rect.width as i32, 0.30),
        resolve(&opts.height, out.rect.height as i32, 0.40),
        resolve(&opts.yshift, out.rect.height as i32, 0.1),
        resolve(&opts.xshift, out.rect.width as i32, 0.0),
        out.rect.x,
        out.rect.y,
    ))
}

/// Applies the for_window rules for dropdown window (float + resize + initial placement)
async fn apply_rules(conn: &mut Connection, cli: &Cli, w: i32, h: i32) -> Result<(), AppError> {
    conn.run_command(format!("for_window [app_id=\"{APP_ID}\"] floating enable"))
        .await?;

    conn.run_command(format!(
        "for_window [app_id=\"{APP_ID}\"] resize set {w} {h}"
    ))
    .await?;

    if cli.center {
        conn.run_command(format!(
            "for_window [app_id=\"{APP_ID}\"] move position center"
        ))
        .await?;
    } else {
        conn.run_command(format!(
            "for_window [app_id=\"{APP_ID}\"] move position cursor"
        ))
        .await?;
    }

    Ok(())
}

/// spawns the dropdown window
async fn spawn_dropdown(conn: &mut Connection, cli: &Cli) -> Result<(), AppError> {
    let original_mouse_warping = get_mouse_warping(conn).await;

    if original_mouse_warping != "container" {
        conn.run_command("mouse_warping container").await?;
    }

    let (w, h, y, x, _out_x, out_y) = compute_dimensions(conn, cli).await?;
    apply_rules(conn, cli, w, h).await?;

    // Subscribe BEFORE spawning so we don't miss the Window::New event
    let subs_conn = Connection::new().await?;
    let mut events = subs_conn.subscribe(&[EventType::Window]).await?;

    let cmd_args: Vec<String> = if cli.command.is_empty() {
        Vec::new()
    } else {
        cli.command.clone()
    };

    let cmd = match cli.terminal {
        Terminal::Kitty => {
            let mut cmd = format!("exec kitty --class {APP_ID} --");
            for a in &cmd_args {
                cmd.push(' ');
                cmd.push_str(&escape(a.clone().into()));
            }
            cmd
        }
        Terminal::Alacritty => {
            let mut cmd = format!("exec alacritty --class {APP_ID}");
            if !cmd_args.is_empty() {
                cmd.push_str(" -e");
                for a in &cmd_args {
                    cmd.push(' ');
                    cmd.push_str(&escape(a.clone().into()));
                }
            }
            cmd
        }
        Terminal::Rio => {
            let mut cmd = format!("exec rio --class {APP_ID}");
            for a in &cmd_args {
                cmd.push(' ');
                cmd.push_str(&escape(a.clone().into()));
            }
            cmd
        }
    };

    conn.run_command(cmd).await?;

    // Wait for window to appear, then apply final positioning
    while let Some(msg) = events.next().await {
        let Event::Window(ev) = msg? else { continue };

        if ev.change == WindowChange::New && ev.container.app_id.as_deref() == Some(APP_ID) {
            let rect = ev.container.rect;

            let final_x = rect.x + x as i64;
            let final_y = if cli.center {
                rect.y + y as i64
            } else {
                out_y + y as i64
            };

            conn.run_command(format!(
                "[app_id=\"{APP_ID}\"] move absolute position {final_x} {final_y}"
            ))
            .await?;
            break;
        }
    }

    conn.run_command(format!("[app_id=\"{APP_ID}\"] focus"))
        .await?;

    focus_change_watcher(conn).await?;

    if original_mouse_warping != "container" {
        conn.run_command(format!("mouse_warping {}", original_mouse_warping))
            .await?;
    }

    Ok(())
}
