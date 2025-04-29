use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::Deserialize;
use std::{env, process::Command};

/// dropdown — toggle a Kitty overlay “dropdown” window
#[derive(Parser)]
#[command(name = "dropdown")]
struct Cli {
    #[command(subcommand)]
    cmd: CommandType,
}

#[derive(Subcommand)]
enum CommandType {
    /// Toggle the dropdown (open if closed, close if open)
    Toggle,
    /// Force-open the dropdown
    Open,
    /// Force-close the dropdown
    Close,
}

#[derive(Deserialize)]
struct OsWindow {
    tabs: Vec<Tab>,
}
#[derive(Deserialize)]
struct Tab {
    windows: Vec<Win>,
}
#[derive(Deserialize)]
struct Win {
    id: i64,
    title: String,
}

fn main() -> Result<()> {
    let Cli { cmd } = Cli::parse();

    // Ensure KITTY_LISTEN_ON is set by your Kitty config (e.g. in kitty.conf: listen_on unix:/tmp/kitty-rc.sock)
    // Then you don't need to pass --to; kitty @ commands will pick up the env variable.

    match cmd {
        CommandType::Toggle => {
            if dropdown_exists()? {
                close_dropdown()?;
            } else {
                open_dropdown()?;
            }
        }
        CommandType::Open => open_dropdown()?,
        CommandType::Close => close_dropdown()?,
    }

    Ok(())
}

/// Check if a Kitty window titled "dropdown" exists
fn dropdown_exists() -> Result<bool> {
    let out = Command::new("kitty")
        .args(["@", "ls"]) // uses KITTY_LISTEN_ON env
        .output()
        .context("listing kitty windows")?;

    if out.stdout.is_empty() {
        return Ok(false);
    }

    let wins: Vec<OsWindow> =
        serde_json::from_slice(&out.stdout).context("parsing kitty ls JSON")?;

    Ok(wins
        .iter()
        .flat_map(|o| &o.tabs)
        .flat_map(|t| &t.windows)
        .any(|w| w.title == "dropdown"))
}

/// Launch a Kitty overlay titled "dropdown"
fn open_dropdown() -> Result<()> {
    Command::new("kitty")
        .args([
            "@",
            "launch",
            "--type",
            "overlay-main",
            "--title",
            "dropdown",
            // position via horizontal split (top)
            "--location",
            "hsplit",
            // shrink height to ~30% (negative bias removes from bottom)
            "--bias",
            "-70",
            // keep focus on original window
            "--keep-focus",
            // allow remote control within overlay
            "--allow-remote-control",
            // set working directory
            "--cwd",
            &env::var("HOME").context("reading HOME env")?,
            // command to run inside overlay
            "--",
            "bluetuith",
        ])
        .status()
        .context("launching dropdown overlay")?;
    Ok(())
}

/// Close the Kitty overlay window titled "dropdown"
fn close_dropdown() -> Result<()> {
    let out = Command::new("kitty")
        .args(["@", "ls"]) // uses KITTY_LISTEN_ON env
        .output()
        .context("listing kitty windows for close")?;

    if out.stdout.is_empty() {
        return Ok(());
    }

    let wins: Vec<OsWindow> =
        serde_json::from_slice(&out.stdout).context("parsing kitty ls JSON for close")?;

    for os in wins {
        for tab in os.tabs {
            for w in tab.windows {
                if w.title == "dropdown" {
                    Command::new("kitty")
                        .args(["@", "close-window", "--match", &format!("id:{}", w.id)])
                        .status()
                        .context("closing dropdown window")?;
                }
            }
        }
    }
    Ok(())
}
