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

struct OutputRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
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

fn find_node_by_app_id<'a>(node: &'a Node, app_id: &str) -> Option<&'a Node> {
    if node.app_id.as_deref() == Some(app_id) {
        return Some(node);
    }
    for child in node.nodes.iter().chain(node.floating_nodes.iter()) {
        if let Some(found) = find_node_by_app_id(child, app_id) {
            return Some(found);
        }
    }
    None
}

pub async fn spawn_dropdown(cli: &Cli) -> Result<()> {
    let mut conn = Connection::new().await?;
    let original_mouse_warping = get_mouse_warping(&mut conn).await;

    let out = get_focused_output(&mut conn).await?;

    let w = resolve(&cli.width, out.width, 0.30);
    let h = resolve(&cli.height, out.height, 0.40);
    let xshift = resolve(&cli.xshift, out.width, 0.0);
    let yshift = resolve(&cli.yshift, out.height, 0.0);

    let final_y = if cli.center {
        out.y + (out.height - h) / 2 + yshift
    } else {
        out.y + yshift
    };

    // Float from creation so Sway places the window at cursor position.
    // Disable mouse warping so the cursor stays put during resize/move.
    conn.run_command(format!(
        "for_window [app_id=\"{APP_ID}\"] floating enable"
    )).await?;
    conn.run_command("mouse_warping none").await?;

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

    // Wait for window to appear (already floating at cursor via for_window rule)
    while let Some(msg) = events.next().await {
        let Event::Window(ev) = msg? else { continue };

        if ev.change == WindowChange::New && ev.container.app_id.as_deref() == Some(APP_ID) {
            break;
        }
    }

    let sel = format!("[app_id=\"{APP_ID}\"]");

    // Step 1: resize (may re-center the window)
    conn.run_command(format!("{sel} resize set {w} {h}")).await?;

    // Step 2: snap back to cursor (cursor hasn't moved — warping is off)
    conn.run_command(format!("{sel} move position mouse")).await?;

    // Step 3: read the cursor-based X, then override only Y
    let tree = conn.get_tree().await?;
    let win_x = find_node_by_app_id(&tree, APP_ID)
        .map(|n| n.rect.x)
        .unwrap_or(out.x + (out.width - w) / 2);
    let final_x = (win_x + xshift).clamp(out.x, out.x + out.width - w);

    conn.run_command(format!("{sel} move absolute position {final_x} {final_y}")).await?;
    conn.run_command(format!("{sel} focus")).await?;

    // Step 4: warp cursor into the window
    conn.run_command(format!("seat seat0 cursor set {} {}", final_x + w / 2, final_y + h / 2)).await?;

    focus_change_watcher(&mut conn).await?;

    // Restore original mouse warping
    conn.run_command(format!("mouse_warping {}", original_mouse_warping))
        .await?;

    Ok(())
}
