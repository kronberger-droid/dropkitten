use niri_ipc::socket::Socket;
use niri_ipc::{
    Action, Event, FloatingPosition, PresetSize, RelativeTo, Request, Response, SpawnRule,
};

use crate::{resolve, Cli, Size, Terminal, APP_ID, Result};

fn send(sock: &mut Socket, req: Request) -> Result<Response> {
    sock.send(req)?.map_err(|e| e.into())
}

fn to_preset(size: &Option<Size>, default_frac: f64) -> PresetSize {
    match size {
        Some(Size::Px(px)) => PresetSize::Fixed(*px as i32),
        Some(Size::Fr(fr)) => PresetSize::Proportion(*fr as f64),
        None => PresetSize::Proportion(default_frac),
    }
}


fn build_spawn_command(cli: &Cli) -> Vec<String> {
    let cmd_args = &cli.command;

    match cli.terminal {
        Terminal::Kitty => {
            let mut args = vec![
                "kitty".into(),
                "--class".into(),
                APP_ID.into(),
                "--".into(),
            ];
            args.extend(cmd_args.iter().cloned());
            args
        }
        Terminal::Alacritty => {
            let mut args = vec!["alacritty".into(), "--class".into(), APP_ID.into()];
            if !cmd_args.is_empty() {
                args.push("-e".into());
                args.extend(cmd_args.iter().cloned());
            }
            args
        }
        Terminal::Rio => {
            let mut args = vec!["rio".into(), "--class".into(), APP_ID.into()];
            args.extend(cmd_args.iter().cloned());
            args
        }
    }
}

pub fn spawn_dropdown(cli: &Cli) -> Result<()> {
    // Open event stream socket BEFORE spawning so we don't miss the open event
    let mut ev_sock = Socket::connect()?;
    ev_sock.send(Request::EventStream)?.map_err(|e| format!("niri EventStream error: {e}"))?;
    let mut next_event = ev_sock.read_events();

    // Command socket for sending actions
    let mut cmd = Socket::connect()?;

    // Query output dimensions (needed for position offsets with pixel values)
    let output = match send(&mut cmd, Request::FocusedOutput)? {
        Response::FocusedOutput(Some(out)) => out,
        _ => return Err("no focused output".into()),
    };
    let logical = output.logical.ok_or("output has no logical info")?;
    let out_w = logical.width as i32;
    let out_h = logical.height as i32;

    // Build a SpawnRule so niri applies floating, size, and position
    // atomically when the window first opens — no flicker.
    let w = resolve(&cli.width, out_w, 0.30);
    let h = resolve(&cli.height, out_h, 0.40);
    let yshift = resolve(&cli.yshift, out_h, if cli.center { 0.0 } else { 0.1 });

    // Horizontal: cursor position, clamped so the window stays on-screen.
    let cursor_x = match send(&mut cmd, Request::CursorPosition)? {
        Response::CursorPosition(Some(pos)) => pos.x,
        _ => (out_w / 2) as f64, // fallback: center
    };
    let xshift = resolve(&cli.xshift, out_w, 0.0);
    let x = (cursor_x as i32 - w / 2 + xshift).clamp(0, out_w - w);

    let y = if cli.center {
        (out_h - h) / 2 + yshift
    } else {
        yshift
    };

    let position = FloatingPosition {
        x: x as f64,
        y: y as f64,
        relative_to: RelativeTo::TopLeft,
    };

    let rule = SpawnRule {
        open_floating: Some(true),
        open_focused: Some(true),
        default_column_width: Some(Some(to_preset(&cli.width, 0.30))),
        default_window_height: Some(Some(to_preset(&cli.height, 0.40))),
        default_floating_position: Some(position),
        ..SpawnRule::default()
    };

    send(&mut cmd, Request::Action(Action::Spawn {
        rule: Some(rule),
        rule_str: None,
        command: build_spawn_command(cli),
    }))?;

    // Wait for window to appear.  Capture is_focused from the Window struct
    // because niri does NOT guarantee event ordering — WindowFocusChanged for
    // our window may arrive BEFORE WindowOpenedOrChanged, meaning the first
    // loop would silently consume it.
    let (window_id, initially_focused) = loop {
        let event = next_event()?;
        if let Event::WindowOpenedOrChanged { window } = event {
            if window.app_id.as_deref() == Some(APP_ID) {
                break (window.id, window.is_focused);
            }
        }
    };

    // Watch for focus loss.  Use `initially_focused` from the window-open event
    // so we don't depend on catching a WindowFocusChanged that may have already
    // been consumed by the first loop.
    let mut confirmed = initially_focused;
    loop {
        let event = next_event()?;
        if let Event::WindowFocusChanged { id } = event {
            if id == Some(window_id) {
                confirmed = true;
            } else if confirmed {
                send(&mut cmd, Request::Action(Action::CloseWindow {
                    id: Some(window_id),
                }))?;
                break;
            }
        }
    }

    Ok(())
}
