use futures_lite::StreamExt;
use shell_escape::unix::escape;
use std::process::Command;
use swayipc_async::{Connection, Event, EventType, WindowChange};

use crate::{resolve, Cli, Terminal, APP_ID, Result};

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

struct OutputRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

fn get_cursor_x() -> Option<i32> {
    let output = Command::new("swaymsg")
        .args(["-t", "get_seats", "--raw"])
        .output()
        .ok()?;
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let seats = json.as_array()?;
    let cursor = seats.first()?.get("cursor")?;
    Some(cursor.get("x")?.as_f64()? as i32)
}

async fn get_focused_output(conn: &mut Connection) -> Result<OutputRect> {
    let out = conn
        .get_outputs()
        .await?
        .into_iter()
        .find(|o| o.focused)
        .ok_or("no focused output")?;
    Ok(OutputRect {
        x: out.rect.x,
        y: out.rect.y,
        width: out.rect.width,
        height: out.rect.height,
    })
}

pub async fn spawn_dropdown(cli: &Cli) -> Result<()> {
    let mut conn = Connection::new().await?;
    let original_mouse_warping = get_mouse_warping(&mut conn).await;

    let out = get_focused_output(&mut conn).await?;

    let w = resolve(&cli.width, out.width, 0.30);
    let h = resolve(&cli.height, out.height, 0.40);
    let xshift = resolve(&cli.xshift, out.width, 0.0);
    let yshift = resolve(&cli.yshift, out.height, 0.0);

    // Horizontal: center on cursor, clamped to stay within the output
    let cursor_x = get_cursor_x().unwrap_or(out.x + out.width / 2);
    let final_x = (cursor_x - w / 2 + xshift).clamp(out.x, out.x + out.width - w);

    let final_y = if cli.center {
        out.y + (out.height - h) / 2 + yshift
    } else {
        out.y + yshift
    };

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

    // Wait for window to appear
    while let Some(msg) = events.next().await {
        let Event::Window(ev) = msg? else { continue };

        if ev.change == WindowChange::New && ev.container.app_id.as_deref() == Some(APP_ID) {
            break;
        }
    }

    // Apply float, resize, and move as separate commands to avoid Sway
    // re-centering the window when chained in a single for_window rule.
    let sel = format!("[app_id=\"{APP_ID}\"]");
    conn.run_command(format!("{sel} floating enable")).await?;
    conn.run_command(format!("{sel} resize set {w} {h}")).await?;
    conn.run_command(format!("{sel} move absolute position {final_x} {final_y}")).await?;
    conn.run_command(format!("{sel} focus")).await?;

    if original_mouse_warping != "container" {
        conn.run_command("mouse_warping container").await?;
    }

    focus_change_watcher(&mut conn).await?;

    if original_mouse_warping != "container" {
        conn.run_command(format!("mouse_warping {}", original_mouse_warping))
            .await?;
    }

    Ok(())
}
