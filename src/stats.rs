//! Distance, time and climbing for a track.

use crate::gpx::{Gpx, Point};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stats {
    pub points: usize,
    /// Metres, summed inside segments only. The gap between two segments is
    /// usually a pause or lost signal, not ground you covered.
    pub distance_m: f64,
    /// First timestamp to last, in seconds. None when the file has no times,
    /// which is normal for a planned route.
    pub elapsed_s: Option<f64>,
    /// Time spent actually moving. Stops at lights, photo breaks and the
    /// minute you stood around before pressing start don't count.
    pub moving_s: Option<f64>,
}

/// Thresholds that depend on what kind of activity the track is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// Below this speed, in metres per second, you're standing still. GPS
    /// drifts a little even when you don't move, so this can't be zero.
    pub min_moving_speed: f64,
    /// A gap between fixes longer than this, in seconds, is a pause (the
    /// watch was stopped or lost signal) and isn't counted as moving.
    pub max_gap_s: f64,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            min_moving_speed: 0.5,
            max_gap_s: 60.0,
        }
    }
}

/// Every segment of every track, as point slices.
pub fn segments(gpx: &Gpx) -> impl Iterator<Item = &[Point]> {
    gpx.tracks.iter().flat_map(|t| t.segments.iter().map(|s| s.points.as_slice()))
}

pub fn stats(gpx: &Gpx, settings: &Settings) -> Stats {
    let mut s = Stats::default();
    let mut moving = 0.0;
    let mut timed = false;
    let mut first_time = None;
    let mut last_time = None;
    for seg in segments(gpx) {
        s.points += seg.len();
        for pair in seg.windows(2) {
            let d = pair[0].pos.distance_to(pair[1].pos);
            s.distance_m += d;
            if let (Some(t0), Some(t1)) = (pair[0].time, pair[1].time) {
                timed = true;
                let dt = t1 - t0;
                if dt > 0.0 && dt <= settings.max_gap_s && d / dt >= settings.min_moving_speed {
                    moving += dt;
                }
            }
        }
        for p in seg {
            if let Some(t) = p.time {
                first_time = Some(first_time.map_or(t, |f: f64| f.min(t)));
                last_time = Some(last_time.map_or(t, |l: f64| l.max(t)));
            }
        }
    }
    s.elapsed_s = first_time.zip(last_time).map(|(f, l)| l - f);
    s.moving_s = timed.then_some(moving);
    s
}
