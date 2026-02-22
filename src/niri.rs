use niri_ipc::socket::Socket;
use niri_ipc::{Action, Event, PositionChange, Request, Response, SizeChange};

use crate::{resolve, Cli, Terminal, APP_ID, Result};

fn send(sock: &mut Socket, req: Request) -> Result<Response> {
    sock.send(req)?.map_err(|e| e.into())
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

    let needs_resize = cli.width.is_some() || cli.height.is_some();
    let needs_shift = cli.xshift.is_some() || cli.yshift.is_some();

    // Spawn the terminal
    send(&mut cmd, Request::Action(Action::Spawn {
        command: build_spawn_command(cli),
    }))?;

    // Wait for window to appear
    let window_id = loop {
        let event = next_event()?;
        if let Event::WindowOpenedOrChanged { window } = event
            && window.app_id.as_deref() == Some(APP_ID)
        {
            break window.id;
        }
    };

    // Float the window -- niri centers floating windows automatically
    send(&mut cmd, Request::Action(Action::MoveWindowToFloating {
        id: Some(window_id),
    }))?;

    // Only resize/reposition when explicitly requested
    if needs_resize || needs_shift {
        let output = match send(&mut cmd, Request::FocusedOutput)? {
            Response::FocusedOutput(Some(out)) => out,
            _ => return Err("no focused output".into()),
        };
        let logical = output.logical.ok_or("output has no logical info")?;
        let out_w = logical.width as i32;
        let out_h = logical.height as i32;

        if let Some(ref width) = cli.width {
            let w = resolve(&Some(width.clone()), out_w, 0.0);
            send(&mut cmd, Request::Action(Action::SetWindowWidth {
                id: Some(window_id),
                change: SizeChange::SetFixed(w),
            }))?;
        }
        if let Some(ref height) = cli.height {
            let h = resolve(&Some(height.clone()), out_h, 0.0);
            send(&mut cmd, Request::Action(Action::SetWindowHeight {
                id: Some(window_id),
                change: SizeChange::SetFixed(h),
            }))?;
        }

        if needs_shift {
            let xshift = resolve(&cli.xshift, out_w, 0.0);
            let yshift = resolve(&cli.yshift, out_h, 0.0);
            send(&mut cmd, Request::Action(Action::MoveFloatingWindow {
                id: Some(window_id),
                x: PositionChange::AdjustFixed(xshift as f64),
                y: PositionChange::AdjustFixed(yshift as f64),
            }))?;
        }
    }

    // Focus the window
    send(&mut cmd, Request::Action(Action::FocusWindow { id: window_id }))?;

    // Watch for focus loss
    loop {
        let event = next_event()?;
        if let Event::WindowFocusChanged { id } = event
            && id != Some(window_id)
        {
            send(&mut cmd, Request::Action(Action::CloseWindow {
                id: Some(window_id),
            }))?;
            break;
        }
    }

    Ok(())
}
