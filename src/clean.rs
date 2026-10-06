//! Fixing up a track: GPS spikes, private start and end points, pauses.

use crate::gpx::{Gpx, Point};

/// Removes fixes that would mean moving faster than `max_speed` metres per
/// second from the last good fix. These are the points that jump across a
/// river or onto the next street when the signal bounces off buildings.
/// Each point is checked against the last point that was kept, so a single
/// bad fix is dropped and the track carries on from where it really was.
/// Points without a time can't be judged and are kept. Returns how many
/// points were dropped.
pub fn drop_spikes(gpx: &mut Gpx, max_speed: f64) -> usize {
    let mut dropped = 0;
    for track in &mut gpx.tracks {
        for seg in &mut track.segments {
            let mut kept: Vec<Point> = Vec::with_capacity(seg.points.len());
            for p in seg.points.drain(..) {
                let bad = kept.last().is_some_and(|last: &Point| match (last.time, p.time) {
                    (Some(t0), Some(t1)) if t1 > t0 => last.pos.distance_to(p.pos) / (t1 - t0) > max_speed,
                    _ => false,
                });
                if bad {
                    dropped += 1;
                } else {
                    kept.push(p);
                }
            }
            seg.points = kept;
        }
    }
    dropped
}
