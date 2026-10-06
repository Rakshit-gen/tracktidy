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

/// Removes points within `radius` metres of where the track starts and of
/// where it ends, so a shared track doesn't show which door you left from.
/// Only the opening and closing stretches are cut: passing the start again
/// in the middle of a loop is left alone. Returns how many points went.
pub fn trim_ends(gpx: &mut Gpx, radius: f64) -> usize {
    let all: Vec<&Point> = gpx.tracks.iter().flat_map(|t| &t.segments).flat_map(|s| &s.points).collect();
    let (Some(first), Some(last)) = (all.first().map(|p| p.pos), all.last().map(|p| p.pos)) else {
        return 0;
    };
    let total = all.len();
    let head = all.iter().take_while(|p| p.pos.distance_to(first) < radius).count();
    let tail = all.iter().rev().take_while(|p| p.pos.distance_to(last) < radius).count();
    let cut = |i: usize| i < head || i >= total.saturating_sub(tail);
    let mut i = 0;
    let mut dropped = 0;
    for seg in gpx.tracks.iter_mut().flat_map(|t| &mut t.segments) {
        seg.points.retain(|_| {
            let gone = cut(i);
            i += 1;
            dropped += gone as usize;
            !gone
        });
    }
    dropped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::tests::track;

    fn norths(gpx: &Gpx) -> Vec<f64> {
        gpx.tracks[0].segments[0].points.iter().map(|p| ((p.pos.lat - 51.5) * 111_195.0).round()).collect()
    }

    #[test]
    fn drops_a_lone_spike() {
        let mut gpx = track(&[(0.0, 0.0, 0.0), (3.0, 1.0, 0.0), (400.0, 2.0, 0.0), (9.0, 3.0, 0.0), (12.0, 4.0, 0.0)]);
        assert_eq!(drop_spikes(&mut gpx, 10.0), 1);
        assert_eq!(norths(&gpx), [0.0, 3.0, 9.0, 12.0]);
    }

    #[test]
    fn drops_a_run_of_bad_fixes() {
        let mut gpx = track(&[
            (0.0, 0.0, 0.0),
            (300.0, 1.0, 0.0),
            (305.0, 2.0, 0.0),
            (9.0, 3.0, 0.0),
            (12.0, 4.0, 0.0),
        ]);
        assert_eq!(drop_spikes(&mut gpx, 10.0), 2);
        assert_eq!(norths(&gpx), [0.0, 9.0, 12.0]);
    }

    #[test]
    fn keeps_fast_but_possible_movement() {
        // A cyclist at 15 m/s is fine with a ride limit.
        let mut gpx = track(&[(0.0, 0.0, 0.0), (15.0, 1.0, 0.0), (30.0, 2.0, 0.0)]);
        assert_eq!(drop_spikes(&mut gpx, 25.0), 0);
    }
}
