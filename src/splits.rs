//! Time for each kilometre of a track.

use crate::gpx::Gpx;
use crate::stats::segments;

#[derive(Clone, Debug, PartialEq)]
pub struct Split {
    /// Length of this split in metres; only the last one can be short.
    pub distance_m: f64,
    /// Clock time spent covering it, stops included.
    pub time_s: f64,
}

/// Cuts the track into pieces `every` metres long and times each one. The
/// moment a split boundary is crossed is found by interpolating between the
/// two fixes either side of it, so a split isn't off by however long the
/// watch waits between fixes. Distance doesn't count across a gap between
/// segments, but the clock keeps running. Points without times are skipped.
pub fn splits(gpx: &Gpx, every: f64) -> Vec<Split> {
    let mut out = Vec::new();
    let mut done = 0.0; // metres covered so far
    let mut mark_d = 0.0; // where the current split started
    let mut mark_t: Option<f64> = None;
    let mut last_t = None;
    for seg in segments(gpx) {
        let timed: Vec<_> = seg.iter().filter_map(|p| Some((p.pos, p.time?))).collect();
        for pair in timed.windows(2) {
            let ((a, ta), (b, tb)) = (pair[0], pair[1]);
            mark_t.get_or_insert(ta);
            let step = a.distance_to(b);
            while step > 0.0 && done + step >= mark_d + every {
                let frac = (mark_d + every - done) / step;
                let t = ta + frac * (tb - ta);
                out.push(Split { distance_m: every, time_s: t - mark_t.unwrap() });
                mark_d += every;
                mark_t = Some(t);
            }
            done += step;
            last_t = Some(tb);
        }
    }
    if let (Some(start), Some(end)) = (mark_t, last_t) {
        if done - mark_d > 1.0 {
            out.push(Split { distance_m: done - mark_d, time_s: end - start });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo::Unit;
    use crate::stats::tests::track;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 0.5
    }

    #[test]
    fn interpolates_the_boundary() {
        // 4 m/s for 2500 m, one fix every 100 m.
        let pts: Vec<_> = (0..=25).map(|i| (i as f64 * 100.0, i as f64 * 25.0, 0.0)).collect();
        let s = splits(&track(&pts), Unit::Km.metres());
        assert_eq!(s.len(), 3);
        assert!(close(s[0].time_s, 250.0) && close(s[1].time_s, 250.0), "{s:?}");
        assert!(close(s[2].distance_m, 500.0) && close(s[2].time_s, 125.0), "{s:?}");
    }

    #[test]
    fn one_long_step_can_cross_several_boundaries() {
        let s = splits(&track(&[(0.0, 0.0, 0.0), (2000.0, 400.0, 0.0)]), 1000.0);
        assert_eq!(s.len(), 2);
        assert!(close(s[0].time_s, 200.0) && close(s[1].time_s, 200.0), "{s:?}");
    }

    #[test]
    fn miles() {
        let pts: Vec<_> = (0..=20).map(|i| (i as f64 * 100.0, i as f64 * 30.0, 0.0)).collect();
        let s = splits(&track(&pts), Unit::Mile.metres());
        assert_eq!(s.len(), 2);
        assert!(close(s[0].time_s, 482.8), "{s:?}");
    }

    #[test]
    fn untimed_tracks_have_no_splits() {
        let mut gpx = track(&[(0.0, 0.0, 0.0), (2000.0, 1.0, 0.0)]);
        gpx.tracks[0].segments[0].points.iter_mut().for_each(|p| p.time = None);
        assert!(splits(&gpx, 1000.0).is_empty());
    }
}
