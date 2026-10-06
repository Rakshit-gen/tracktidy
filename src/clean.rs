//! Fixing up a track: GPS spikes, private start and end points, pauses.

use crate::geo::LatLon;
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

/// Removes every point within `radius` metres of `home`, wherever it falls
/// in the track. Use this when a ride starts somewhere else but passes the
/// house on the way back. Returns how many points went.
pub fn hide_zone(gpx: &mut Gpx, home: LatLon, radius: f64) -> usize {
    let mut dropped = 0;
    for seg in gpx.tracks.iter_mut().flat_map(|t| &mut t.segments) {
        let before = seg.points.len();
        seg.points.retain(|p| p.pos.distance_to(home) >= radius);
        dropped += before - seg.points.len();
    }
    dropped
}

/// Collapses stops. While you wait at a light the watch keeps logging, and
/// the fixes wander a few metres around where you stand, which adds false
/// distance. Any run of points that stays within `radius` metres of its
/// first point for at least `min_s` seconds is cut down to its first and
/// last point. Returns how many points went.
pub fn collapse_pauses(gpx: &mut Gpx, radius: f64, min_s: f64) -> usize {
    let mut dropped = 0;
    for seg in gpx.tracks.iter_mut().flat_map(|t| &mut t.segments) {
        let pts = std::mem::take(&mut seg.points);
        let mut kept = Vec::with_capacity(pts.len());
        let mut i = 0;
        while i < pts.len() {
            let anchor = pts[i].pos;
            let mut j = i + 1;
            while j < pts.len() && pts[j].pos.distance_to(anchor) < radius {
                j += 1;
            }
            // pts[i..j] all sit near the anchor.
            let long = match (pts[i].time, pts[j - 1].time) {
                (Some(a), Some(b)) => b - a >= min_s,
                _ => false,
            };
            if long && j - i > 2 {
                dropped += j - i - 2;
                kept.push(pts[i].clone());
                kept.push(pts[j - 1].clone());
                i = j;
            } else {
                kept.push(pts[i].clone());
                i += 1;
            }
        }
        seg.points = kept;
    }
    dropped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::tests::{north, track};

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

    fn line(metres: &[f64]) -> Gpx {
        let pts: Vec<(f64, f64, f64)> = metres.iter().enumerate().map(|(i, &m)| (m, i as f64, 0.0)).collect();
        track(&pts)
    }

    #[test]
    fn trims_both_ends() {
        let mut gpx = line(&[0.0, 50.0, 150.0, 300.0, 450.0, 550.0, 600.0]);
        assert_eq!(trim_ends(&mut gpx, 100.0), 4);
        assert_eq!(norths(&gpx), [150.0, 300.0, 450.0]);
    }

    #[test]
    fn trimming_a_loop_keeps_the_middle() {
        // Out 400 m and back: the far end of the loop is well clear, but the
        // whole way out and back is near the start, so only the ends go.
        let mut gpx = line(&[0.0, 50.0, 200.0, 400.0, 200.0, 50.0, 0.0]);
        assert_eq!(trim_ends(&mut gpx, 100.0), 4);
        assert_eq!(norths(&gpx), [200.0, 400.0, 200.0]);
    }

    #[test]
    fn trimming_everything_leaves_nothing() {
        let mut gpx = line(&[0.0, 10.0, 20.0]);
        assert_eq!(trim_ends(&mut gpx, 100.0), 3);
        assert!(norths(&gpx).is_empty());
    }

    #[test]
    fn hides_a_zone_in_the_middle() {
        let mut gpx = line(&[0.0, 200.0, 480.0, 500.0, 520.0, 800.0]);
        assert_eq!(hide_zone(&mut gpx, north(500.0), 50.0), 3);
        assert_eq!(norths(&gpx), [0.0, 200.0, 800.0]);
    }

    #[test]
    fn collapses_a_long_stop() {
        let mut gpx = track(&[
            (0.0, 0.0, 0.0),
            (10.0, 3.0, 0.0),
            (12.0, 20.0, 0.0),
            (9.0, 40.0, 0.0),
            (11.0, 60.0, 0.0),
            (40.0, 70.0, 0.0),
        ]);
        assert_eq!(collapse_pauses(&mut gpx, 5.0, 30.0), 2);
        assert_eq!(norths(&gpx), [0.0, 10.0, 11.0, 40.0]);
    }

    #[test]
    fn leaves_a_short_stop() {
        let mut gpx = track(&[(0.0, 0.0, 0.0), (1.0, 5.0, 0.0), (2.0, 10.0, 0.0), (30.0, 20.0, 0.0)]);
        assert_eq!(collapse_pauses(&mut gpx, 5.0, 30.0), 0);
    }
}
