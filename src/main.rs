mod niri;
mod sway;

use clap::{Parser, ValueEnum};
use std::str::FromStr;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub static APP_ID: &str = "test";

#[derive(Debug, Clone, ValueEnum)]
pub enum Terminal {
    Kitty,
    Alacritty,
    Rio,
}

#[derive(Debug, Clone)]
pub enum Size {
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
pub struct Cli {
    /// terminal to use
    #[arg(short = 't', long = "terminal", value_enum)]
    pub terminal: Terminal,

    /// window width (pixels or fraction)
    #[arg(short = 'W', long = "width")]
    pub width: Option<Size>,

    /// window height (pixels or fraction)
    #[arg(short = 'H', long = "height")]
    pub height: Option<Size>,

    #[arg(short = 'y', long = "yshift")]
    pub yshift: Option<Size>,

    #[arg(short = 'x', long = "xshift")]
    pub xshift: Option<Size>,

    #[arg(short = 'c', long = "center")]
    pub center: bool,

    /// subcommand to run + its arguments
    #[arg(last = true)]
    pub command: Vec<String>,
}

pub fn resolve(opt: &Option<Size>, screen: i32, def_frac: f32) -> i32 {
    match opt {
        Some(Size::Px(px)) => *px as i32,
        Some(Size::Fr(fr)) => (screen as f32 * fr).round() as i32,
        None => (screen as f32 * def_frac).round() as i32,
    }
}

enum Backend {
    Sway,
    Niri,
}

fn detect_backend() -> Result<Backend> {
    if std::env::var_os("NIRI_SOCKET").is_some() {
        Ok(Backend::Niri)
    } else if std::env::var_os("SWAYSOCK").is_some() {
        Ok(Backend::Sway)
    } else {
        Err("neither $NIRI_SOCKET nor $SWAYSOCK is set -- are you running Sway or niri?".into())
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match detect_backend()? {
        Backend::Sway => futures_lite::future::block_on(sway::spawn_dropdown(&cli)),
        Backend::Niri => niri::spawn_dropdown(&cli),
    }
}
