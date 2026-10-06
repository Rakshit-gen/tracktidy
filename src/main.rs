use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};

use tracktidy::clean;
use tracktidy::geo::Unit;
use tracktidy::splits::splits;
use tracktidy::gpx::{Gpx, read_gpx, write_gpx};
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
        #[arg(long, value_enum, default_value_t = Sport::Run)]
        sport: Sport,
    },
    /// Remove GPS spikes, stops and the private ends of a track
    Clean {
        file: PathBuf,
        /// Where to write the result [default: next to the input, as NAME.clean.gpx]
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = Sport::Run)]
        sport: Sport,
        /// Fastest believable speed in km/h; faster jumps are dropped as spikes [default: set by --sport]
        #[arg(long)]
        max_speed: Option<f64>,
        /// Cut this many metres off the start and end of the track
        #[arg(long, default_value_t = 0.0)]
        trim: f64,
        /// Leave stops as they are instead of collapsing them
        #[arg(long)]
        keep_pauses: bool,
    },
    /// Time and pace for each kilometre or mile
    Splits {
        file: PathBuf,
        /// Split by mile instead of kilometre
        #[arg(long)]
        miles: bool,
    },
}

/// What the track was, which decides what counts as moving and how fast is
/// too fast to be real.
#[derive(Clone, Copy, ValueEnum)]
enum Sport {
    Walk,
    Run,
    Ride,
}

impl Sport {
    fn settings(self) -> Settings {
        let min_moving_speed = match self {
            Sport::Walk => 0.3,
            Sport::Run => 0.5,
            Sport::Ride => 1.0,
        };
        Settings { min_moving_speed, ..Settings::default() }
    }

    /// km/h. Generous on purpose: a sprint finish or a fast descent is real.
    fn max_speed(self) -> f64 {
        match self {
            Sport::Walk => 15.0,
            Sport::Run => 30.0,
            Sport::Ride => 90.0,
        }
    }
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Stats { file, sport } => {
            let gpx = load(&file)?;
            print!("{}", describe(&stats(&gpx, &sport.settings())));
        }
        Command::Clean { file, output, sport, max_speed, trim, keep_pauses } => {
            let mut gpx = load(&file)?;
            let settings = sport.settings();
            let max_speed = max_speed.unwrap_or(sport.max_speed());
            let before = stats(&gpx, &settings);
            let mut report = Vec::new();
            let mut note = |n: usize, what: &str| {
                if n > 0 {
                    report.push(format!("{n} {what}"));
                }
            };
            note(clean::drop_time_glitches(&mut gpx), "with repeated or backwards times");
            note(clean::drop_spikes(&mut gpx, max_speed / 3.6), "GPS spikes");
            if !keep_pauses {
                note(clean::collapse_pauses(&mut gpx, 5.0, 20.0), "from stops");
            }
            if trim > 0.0 {
                note(clean::trim_ends(&mut gpx, trim), "near the start and end");
            }
            let after = stats(&gpx, &settings);
            if report.is_empty() {
                eprintln!("Nothing to clean.");
            } else {
                eprintln!("Removed {} of {} points:", before.points - after.points, before.points);
                for line in &report {
                    eprintln!("  {line}");
                }
                eprintln!("Distance {:.2} km, was {:.2} km.", after.distance_m / 1000.0, before.distance_m / 1000.0);
            }
            let output = output.unwrap_or_else(|| file.with_extension("clean.gpx"));
            std::fs::write(&output, write_gpx(&gpx))
                .with_context(|| format!("couldn't write {}", output.display()))?;
            eprintln!("Wrote {}", output.display());
        }
        Command::Splits { file, miles } => {
            let unit = if miles { Unit::Mile } else { Unit::Km };
            let gpx = load(&file)?;
            let list = splits(&gpx, unit.metres());
            if list.is_empty() {
                anyhow::bail!("{} has no timed points, so there is nothing to split", file.display());
            }
            let mut covered = 0.0;
            for s in &list {
                covered += s.distance_m;
                let pace = s.time_s / (s.distance_m / unit.metres());
                println!(
                    "{:>6.2} {}  {:>8}  {:>6} /{}",
                    covered / unit.metres(),
                    unit.label(),
                    clock(s.time_s),
                    clock(pace),
                    unit.label()
                );
            }
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
