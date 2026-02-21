use futures_lite::StreamExt;
use shell_escape::unix::escape;
use swayipc_async::{Connection, Event, EventType, Node, WindowChange};

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

async fn apply_rules(conn: &mut Connection, w: i32, h: i32) -> Result<()> {
    conn.run_command(format!(
        "for_window [app_id=\"{APP_ID}\"] floating enable"
    ))
    .await?;

    conn.run_command(format!(
        "for_window [app_id=\"{APP_ID}\"] resize set {w} {h}"
    ))
    .await?;

    Ok(())
}

pub async fn spawn_dropdown(cli: &Cli) -> Result<()> {
    let mut conn = Connection::new().await?;
    let original_mouse_warping = get_mouse_warping(&mut conn).await;

    let (w, h, y, x, _out_x, out_y) = compute_dimensions(&mut conn, cli).await?;
    apply_rules(&mut conn, w, h).await?;

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

            let tree = conn.get_tree().await?;
            let wx = find_node_by_app_id(&tree, APP_ID).unwrap_or(0);

            let final_x = wx + x;
            let final_y = if cli.center {
                let wy = tree.rect.height / 2;
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
