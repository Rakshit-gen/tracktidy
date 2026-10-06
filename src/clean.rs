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
