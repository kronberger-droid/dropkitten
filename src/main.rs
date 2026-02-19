use clap::{Parser, ValueEnum};
use futures_lite::StreamExt;
use shell_escape::unix::escape;
use std::str::FromStr;
use swayipc_async::{Connection, Event, EventType, Node, WindowChange};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

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
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    futures_lite::future::block_on(async {
        let mut conn = Connection::new().await?;
        spawn_dropdown(&mut conn, &cli).await
    })
}

async fn get_mouse_warping(conn: &mut Connection) -> String {
    let config = match conn.get_config().await {
        Ok(cfg) => cfg.config,
        Err(_) => return "none".to_string(),
    };

    config
        .lines()
        .find_map(|line| {
            let trimmed = line.trim();
            let rest = trimmed.strip_prefix("mouse_warping")?;
            rest.split_whitespace().next().map(String::from)
        })
        .unwrap_or_else(|| "none".to_string())
}

async fn focus_change_watcher(conn: &mut Connection) -> Result<()> {
    let subs_conn = Connection::new().await?;

    let mut events = subs_conn.subscribe([EventType::Window]).await?;

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

fn find_node_by_app_id(node: &Node, app_id: &str) -> Option<i32> {
    if node.app_id.as_deref() == Some(app_id) {
        return Some(node.rect.x);
    }
    for child in &node.nodes {
        if let Some(x) = find_node_by_app_id(child, app_id) {
            return Some(x);
        }
    }
    for child in &node.floating_nodes {
        if let Some(x) = find_node_by_app_id(child, app_id) {
            return Some(x);
        }
    }
    None
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
) -> Result<(i32, i32, i32, i32, i32, i32)> {
    let out = conn
        .get_outputs()
        .await?
        .into_iter()
        .find(|o| o.active)
        .ok_or("no active output")?;
    Ok((
        resolve(&opts.width, out.rect.width, 0.30),
        resolve(&opts.height, out.rect.height, 0.40),
        resolve(&opts.yshift, out.rect.height, 0.1),
        resolve(&opts.xshift, out.rect.width, 0.0),
        out.rect.x,
        out.rect.y,
    ))
}

/// Applies the for_window rules for dropdown window (float + resize)
async fn apply_rules(conn: &mut Connection, w: i32, h: i32) -> Result<()> {
    conn.run_command(format!("for_window [app_id=\"{APP_ID}\"] floating enable"))
        .await?;

    conn.run_command(format!(
        "for_window [app_id=\"{APP_ID}\"] resize set {w} {h}"
    ))
    .await?;

    Ok(())
}

/// spawns the dropdown window
async fn spawn_dropdown(conn: &mut Connection, cli: &Cli) -> Result<()> {
    let original_mouse_warping = get_mouse_warping(conn).await;

    if original_mouse_warping != "container" {
        conn.run_command("mouse_warping container").await?;
    }

    let (w, h, y, x, _out_x, out_y) = compute_dimensions(conn, cli).await?;
    apply_rules(conn, w, h).await?;

    // Subscribe BEFORE spawning so we don't miss the Window::New event
    let subs_conn = Connection::new().await?;
    let mut events = subs_conn.subscribe([EventType::Window]).await?;

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
            // Explicitly place at cursor (for_window rule may not have applied yet)
            if cli.center {
                conn.run_command(format!(
                    "[app_id=\"{APP_ID}\"] move position center"
                ))
                .await?;
            } else {
                conn.run_command(format!(
                    "[app_id=\"{APP_ID}\"] move position cursor"
                ))
                .await?;
            }

            // Now query the tree to get the actual X position after placement
            let tree = conn.get_tree().await?;
            let wx = find_node_by_app_id(&tree, APP_ID).unwrap_or(0);

            let final_x = wx + x;
            let final_y = if cli.center {
                let wy = tree.rect.height / 2; // approximate center Y
                wy + y
            } else {
                out_y + y
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
