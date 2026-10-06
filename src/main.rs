use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};

use tracktidy::clean;
use tracktidy::geo::{LatLon, Unit};
use tracktidy::gpx::{Gpx, read_gpx, write_gpx};
use tracktidy::splits::splits;
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
        /// Show distance and pace in miles
        #[arg(long)]
        miles: bool,
        /// Print the numbers as JSON, in metres and seconds
        #[arg(long)]
        json: bool,
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
        /// Remove every point near this spot, given as "lat,lon"
        #[arg(long, value_parser = parse_latlon, allow_hyphen_values = true)]
        home: Option<LatLon>,
        /// How far around --home to clear, in metres
        #[arg(long, default_value_t = 200.0)]
        home_radius: f64,
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
        Settings {
            min_moving_speed,
            ..Settings::default()
        }
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
        Command::Stats {
            file,
            sport,
            miles,
            json,
        } => {
            let gpx = load(&file)?;
            let s = stats(&gpx, &sport.settings());
            if json {
                println!("{}", to_json(&s));
            } else {
                print!(
                    "{}",
                    describe(&s, if miles { Unit::Mile } else { Unit::Km })
                );
            }
        }
        Command::Clean {
            file,
            output,
            sport,
            max_speed,
            trim,
            home,
            home_radius,
            keep_pauses,
        } => {
            let mut gpx = load(&file)?;
            let settings = sport.settings();
            let max_speed = max_speed.unwrap_or(sport.max_speed());
            let before = stats(&gpx, &settings);
            if before.elapsed_s.is_none() {
                eprintln!(
                    "This track has no times (a planned route?), so spikes and stops can't be told apart."
                );
            }
            let mut report = Vec::new();
            let mut note = |n: usize, what: &str| {
                if n > 0 {
                    report.push(format!("{n} {what}"));
                }
            };
            note(
                clean::drop_time_glitches(&mut gpx),
                "with repeated or backwards times",
            );
            note(clean::drop_spikes(&mut gpx, max_speed / 3.6), "GPS spikes");
            if !keep_pauses {
                note(clean::collapse_pauses(&mut gpx, 5.0, 20.0), "from stops");
            }
            if trim > 0.0 {
                note(clean::trim_ends(&mut gpx, trim), "near the start and end");
            }
            if let Some(home) = home {
                note(clean::hide_zone(&mut gpx, home, home_radius), "near home");
            }
            clean::prune_empty(&mut gpx);
            if gpx.tracks.is_empty() {
                anyhow::bail!(
                    "cleaning removed every point, so nothing was written; try a smaller --trim or --home-radius"
                );
            }
            let after = stats(&gpx, &settings);
            if report.is_empty() {
                eprintln!("Nothing to clean.");
            } else {
                eprintln!(
                    "Removed {} of {} points:",
                    before.points - after.points,
                    before.points
                );
                for line in &report {
                    eprintln!("  {line}");
                }
                eprintln!(
                    "Distance {:.2} km, was {:.2} km.",
                    after.distance_m / 1000.0,
                    before.distance_m / 1000.0
                );
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
                anyhow::bail!(
                    "{} has no timed points, so there is nothing to split",
                    file.display()
                );
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

/// "51.5074,-0.1278" with optional spaces after the comma.
fn parse_latlon(s: &str) -> Result<LatLon, String> {
    let bad =
        || format!("expected \"lat,lon\" in decimal degrees, like 51.5074,-0.1278, not \"{s}\"");
    let (lat, lon) = s.split_once(',').ok_or_else(bad)?;
    let lat: f64 = lat.trim().parse().map_err(|_| bad())?;
    let lon: f64 = lon.trim().parse().map_err(|_| bad())?;
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return Err(format!("{lat},{lon} is off the map; latitude comes first"));
    }
    Ok(LatLon { lat, lon })
}

fn load(path: &Path) -> Result<Gpx> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("couldn't read {}", path.display()))?;
    read_gpx(&text).with_context(|| format!("couldn't read {} as GPX", path.display()))
}

/// 3725 seconds as "1:02:05", 125 as "2:05".
fn clock(seconds: f64) -> String {
    let s = seconds.round() as u64;
    let (h, m, s) = (s / 3600, s % 3600 / 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

fn to_json(s: &Stats) -> String {
    let num = |v: Option<f64>| v.map_or("null".to_string(), |v| format!("{v:.1}"));
    format!(
        "{{\"points\":{},\"distance_m\":{:.1},\"elapsed_s\":{},\"moving_s\":{},\"gain_m\":{},\"loss_m\":{},\"max_speed_mps\":{}}}",
        s.points,
        s.distance_m,
        num(s.elapsed_s),
        num(s.moving_s),
        num(s.gain_m),
        num(s.loss_m),
        num(s.max_speed)
    )
}

fn describe(s: &Stats, unit: Unit) -> String {
    let (per, label) = (unit.metres(), unit.label());
    let mut out = format!("Distance   {:.2} {label}\n", s.distance_m / per);
    if let Some(elapsed) = s.elapsed_s {
        out += &format!("Elapsed    {}\n", clock(elapsed));
    }
    if let Some(moving) = s.moving_s {
        out += &format!("Moving     {}\n", clock(moving));
        if moving > 0.0 {
            let pace = moving / (s.distance_m / per);
            out += &format!("Pace       {} /{label} moving\n", clock(pace));
        }
    }
    if let (Some(gain), Some(loss)) = (s.gain_m, s.loss_m) {
        let (f, h) = if unit == Unit::Mile {
            (3.28084, "ft")
        } else {
            (1.0, "m")
        };
        out += &format!(
            "Climb      {:.0} {h} up, {:.0} {h} down\n",
            gain * f,
            loss * f
        );
    }
    if let Some(top) = s.max_speed {
        let (speed, speed_label) = match unit {
            Unit::Km => (top * 3.6, "km/h"),
            Unit::Mile => (top * 3600.0 / per, "mph"),
        };
        out += &format!("Top speed  {speed:.1} {speed_label} over 10 s\n");
    }
    out += &format!("Points     {}\n", s.points);
    out
}
