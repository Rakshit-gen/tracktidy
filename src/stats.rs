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
}

/// Every segment of every track, as point slices.
pub fn segments(gpx: &Gpx) -> impl Iterator<Item = &[Point]> {
    gpx.tracks.iter().flat_map(|t| t.segments.iter().map(|s| s.points.as_slice()))
}

pub fn stats(gpx: &Gpx) -> Stats {
    let mut s = Stats::default();
    let mut first_time = None;
    let mut last_time = None;
    for seg in segments(gpx) {
        s.points += seg.len();
        for pair in seg.windows(2) {
            s.distance_m += pair[0].pos.distance_to(pair[1].pos);
        }
        for p in seg {
            if let Some(t) = p.time {
                first_time = Some(first_time.map_or(t, |f: f64| f.min(t)));
                last_time = Some(last_time.map_or(t, |l: f64| l.max(t)));
            }
        }
    }
    s.elapsed_s = first_time.zip(last_time).map(|(f, l)| l - f);
    s
}
