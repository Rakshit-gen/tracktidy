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
    /// Metres climbed and descended. None when no point has an elevation.
    pub gain_m: Option<f64>,
    pub loss_m: Option<f64>,
    /// Fastest speed held over at least ten seconds, in metres per second.
    /// Point-to-point speed jumps around too much to be worth reporting.
    pub max_speed: Option<f64>,
}

/// Seconds a speed has to be held to count as the top speed.
const TOP_SPEED_WINDOW_S: f64 = 10.0;

/// The fastest average speed over any stretch of at least `window` seconds,
/// using two pointers over the segment's running distance.
fn top_speed(seg: &[Point], window: f64) -> Option<f64> {
    let mut along = Vec::with_capacity(seg.len());
    let mut total = 0.0;
    for (i, p) in seg.iter().enumerate() {
        if i > 0 {
            total += seg[i - 1].pos.distance_to(p.pos);
        }
        along.push(total);
    }
    let mut best: Option<f64> = None;
    let mut j = 0;
    for i in 0..seg.len() {
        let Some(ti) = seg[i].time else { continue };
        j = j.max(i + 1);
        while j < seg.len() && seg[j].time.is_some_and(|tj| tj - ti < window) {
            j += 1;
        }
        let Some(tj) = seg.get(j).and_then(|p| p.time) else { break };
        let speed = (along[j] - along[i]) / (tj - ti);
        best = Some(best.map_or(speed, |b| b.max(speed)));
    }
    best
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
    /// Elevation has to move this many metres from the last counted level
    /// before it's counted as climbing or descending. GPS elevation wobbles by
    /// a metre or two from fix to fix, and adding up every wobble can double
    /// the climb on a flat run.
    pub climb_threshold_m: f64,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            min_moving_speed: 0.5,
            max_gap_s: 60.0,
            climb_threshold_m: 3.0,
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
    let mut level: Option<f64> = None;
    let (mut gain, mut loss) = (0.0, 0.0);
    let mut first_time = None;
    let mut last_time = None;
    for seg in segments(gpx) {
        s.points += seg.len();
        if let Some(top) = top_speed(seg, TOP_SPEED_WINDOW_S) {
            s.max_speed = Some(s.max_speed.map_or(top, |m| m.max(top)));
        }
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
            if let Some(ele) = p.ele {
                match level {
                    None => level = Some(ele),
                    Some(l) if ele - l >= settings.climb_threshold_m => {
                        gain += ele - l;
                        level = Some(ele);
                    }
                    Some(l) if l - ele >= settings.climb_threshold_m => {
                        loss += l - ele;
                        level = Some(ele);
                    }
                    _ => {}
                }
            }
            if let Some(t) = p.time {
                first_time = Some(first_time.map_or(t, |f: f64| f.min(t)));
                last_time = Some(last_time.map_or(t, |l: f64| l.max(t)));
            }
        }
    }
    s.elapsed_s = first_time.zip(last_time).map(|(f, l)| l - f);
    s.moving_s = timed.then_some(moving);
    if level.is_some() {
        s.gain_m = Some(gain);
        s.loss_m = Some(loss);
    }
    s
}
