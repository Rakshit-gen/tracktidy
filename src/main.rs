use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use tracktidy::gpx::{Gpx, read_gpx};
use tracktidy::stats::{Settings, Stats, stats};

#[derive(Parser)]
#[command(version, about = "Clean up GPX tracks before you share them")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Distance, time, climbing and speed for a track
    Stats {
        file: PathBuf,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Stats { file } => {
            let gpx = load(&file)?;
            print!("{}", describe(&stats(&gpx, &Settings::default())));
        }
    }
    Ok(())
}

fn load(path: &Path) -> Result<Gpx> {
    let text = std::fs::read_to_string(path).with_context(|| format!("couldn't read {}", path.display()))?;
    read_gpx(&text).with_context(|| format!("couldn't read {} as GPX", path.display()))
}

/// 3725 seconds as "1:02:05", 125 as "2:05".
fn clock(seconds: f64) -> String {
    let s = seconds.round() as u64;
    let (h, m, s) = (s / 3600, s % 3600 / 60, s % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

fn describe(s: &Stats) -> String {
    let mut out = format!("Distance   {:.2} km\n", s.distance_m / 1000.0);
    if let Some(elapsed) = s.elapsed_s {
        out += &format!("Elapsed    {}\n", clock(elapsed));
    }
    if let Some(moving) = s.moving_s {
        out += &format!("Moving     {}\n", clock(moving));
        if moving > 0.0 {
            let pace = moving / (s.distance_m / 1000.0);
            out += &format!("Pace       {} /km moving\n", clock(pace));
        }
    }
    if let (Some(gain), Some(loss)) = (s.gain_m, s.loss_m) {
        out += &format!("Climb      {gain:.0} m up, {loss:.0} m down\n");
    }
    if let Some(top) = s.max_speed {
        out += &format!("Top speed  {:.1} km/h over 10 s\n", top * 3.6);
    }
    out += &format!("Points     {}\n", s.points);
    out
}
