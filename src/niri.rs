use niri_ipc::socket::Socket;
use niri_ipc::{Action, Event, PositionChange, Request, Response, SizeChange};

use crate::{resolve, Cli, Terminal, APP_ID, Result};

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
    let reply = ev_sock.send(Request::EventStream)?;
    reply.map_err(|e| format!("niri EventStream error: {e}"))?;
    let mut next_event = ev_sock.read_events();

    // Command socket for sending actions
    let mut cmd = Socket::connect()?;

    // Query focused output dimensions
    let reply = cmd.send(Request::FocusedOutput)?;
    let output = match reply.map_err(|e| format!("niri FocusedOutput error: {e}"))? {
        Response::FocusedOutput(Some(out)) => out,
        _ => return Err("no focused output".into()),
    };

    let logical = output.logical.ok_or("output has no logical info")?;
    let out_x = logical.x;
    let out_y = logical.y;
    let out_w = logical.width as i32;
    let out_h = logical.height as i32;

    let w = resolve(&cli.width, out_w, 0.30);
    let h = resolve(&cli.height, out_h, 0.40);
    let yshift = resolve(&cli.yshift, out_h, 0.1);
    let xshift = resolve(&cli.xshift, out_w, 0.0);

    // Spawn the terminal
    let spawn_cmd = build_spawn_command(cli);
    let reply = cmd.send(Request::Action(Action::Spawn {
        command: spawn_cmd,
    }))?;
    reply.map_err(|e| format!("niri Spawn error: {e}"))?;

    // Wait for window to appear
    let window_id = loop {
        let event = next_event()?;
        if let Event::WindowOpenedOrChanged { window } = event
            && window.app_id.as_deref() == Some(APP_ID)
        {
            break window.id;
        }
    };

    // Float the window
    let reply = cmd.send(Request::Action(Action::MoveWindowToFloating {
        id: Some(window_id),
    }))?;
    reply.map_err(|e| format!("niri MoveWindowToFloating error: {e}"))?;

    // Resize
    let reply = cmd.send(Request::Action(Action::SetWindowWidth {
        id: Some(window_id),
        change: SizeChange::SetFixed(w),
    }))?;
    reply.map_err(|e| format!("niri SetWindowWidth error: {e}"))?;

    let reply = cmd.send(Request::Action(Action::SetWindowHeight {
        id: Some(window_id),
        change: SizeChange::SetFixed(h),
    }))?;
    reply.map_err(|e| format!("niri SetWindowHeight error: {e}"))?;

    // Position
    if cli.center {
        let reply = cmd.send(Request::Action(Action::CenterWindow {
            id: Some(window_id),
        }))?;
        reply.map_err(|e| format!("niri CenterWindow error: {e}"))?;

        // Apply shifts on top of center if specified
        if xshift != 0 || yshift != 0 {
            let reply = cmd.send(Request::Action(Action::MoveFloatingWindow {
                id: Some(window_id),
                x: PositionChange::AdjustFixed(xshift as f64),
                y: PositionChange::AdjustFixed(yshift as f64),
            }))?;
            reply.map_err(|e| format!("niri MoveFloatingWindow error: {e}"))?;
        }
    } else {
        let final_x = out_x as f64 + xshift as f64;
        let final_y = out_y as f64 + yshift as f64;

        let reply = cmd.send(Request::Action(Action::MoveFloatingWindow {
            id: Some(window_id),
            x: PositionChange::SetFixed(final_x),
            y: PositionChange::SetFixed(final_y),
        }))?;
        reply.map_err(|e| format!("niri MoveFloatingWindow error: {e}"))?;
    }

    // Focus the window
    let reply = cmd.send(Request::Action(Action::FocusWindow { id: window_id }))?;
    reply.map_err(|e| format!("niri FocusWindow error: {e}"))?;

    // Watch for focus loss
    loop {
        let event = next_event()?;
        if let Event::WindowFocusChanged { id } = event
            && id != Some(window_id)
        {
            let reply = cmd.send(Request::Action(Action::CloseWindow {
                id: Some(window_id),
            }))?;
            reply.map_err(|e| format!("niri CloseWindow error: {e}"))?;
            break;
        }
    }

    Ok(())
}
