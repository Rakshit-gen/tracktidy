//! Writes examples/sample-run.gpx: a made-up 5 km run around Hyde Park with
//! the usual problems in it, so the commands have something to work on.
//!
//!     cargo run --example make_sample > examples/sample-run.gpx
//!
//! It is seeded, so the file comes out the same every time.

use tracktidy::geo::LatLon;
use tracktidy::gpx::{Gpx, Point, Segment, Track, write_gpx};
use tracktidy::time::parse_time;

/// xorshift64, enough to wobble GPS fixes.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Roughly normal, mean 0, standard deviation 1.
    fn normal(&mut self) -> f64 {
        (0..12).map(|_| self.next()).sum::<f64>() - 6.0
    }
}

const CENTRE: LatLon = LatLon {
    lat: 51.5073,
    lon: -0.1657,
};
const M_PER_DEG_LAT: f64 = 111_195.0;

fn offset(east: f64, north: f64) -> LatLon {
    let m_per_deg_lon = M_PER_DEG_LAT * CENTRE.lat.to_radians().cos();
    LatLon {
        lat: CENTRE.lat + north / M_PER_DEG_LAT,
        lon: CENTRE.lon + east / m_per_deg_lon,
    }
}

fn main() {
    let mut rng = Rng(0x7ac7_71d1);
    let start = parse_time("2026-05-03T08:12:00Z").unwrap();
    // An ellipse about 5 km round, run at a touch over 3 m/s, with a fix
    // every 3 seconds.
    let (a, b) = (1000.0, 520.0);
    let mut points = Vec::new();
    let mut t = 0.0;
    let mut angle: f64 = 0.0;
    let mut paused = false;
    while angle < std::f64::consts::TAU {
        let speed = 3.1 + 0.2 * rng.normal();
        let (east, north) = (a * angle.cos() - a, b * angle.sin());
        let noise = 2.5;
        let mut pos = offset(east + noise * rng.normal(), north + noise * rng.normal());
        // One fix bounces off a building and lands 350 m away.
        if points.len() == 200 {
            pos = offset(east + 250.0, north + 250.0);
        }
        points.push(Point {
            pos,
            ele: Some(30.0 + 6.0 * (2.0 * angle).sin() + 0.4 * rng.normal()),
            time: Some(start + t),
            extensions: Some(format!(
                "<gpxtpx:TrackPointExtension><gpxtpx:hr>{}</gpxtpx:hr></gpxtpx:TrackPointExtension>",
                (148.0 + 6.0 * rng.normal()).round()
            )),
        });
        // Wait 90 seconds at the crossing halfway round.
        if !paused && angle > std::f64::consts::PI {
            paused = true;
            for _ in 0..30 {
                t += 3.0;
                let mut p = points.last().unwrap().clone();
                p.pos = offset(east + 1.5 * rng.normal(), north + 1.5 * rng.normal());
                p.time = Some(start + t);
                points.push(p);
            }
        }
        t += 3.0;
        // Step along the ellipse by about speed * 3 metres.
        let (dx, dy) = (a * angle.sin(), b * angle.cos());
        angle += speed * 3.0 / dx.hypot(dy);
    }
    let gpx = Gpx {
        name: Some("Sunday loop".into()),
        creator: Some("tracktidy make_sample".into()),
        namespaces: vec![(
            "xmlns:gpxtpx".into(),
            "http://www.garmin.com/xmlschemas/TrackPointExtension/v1".into(),
        )],
        tracks: vec![Track {
            name: Some("Sunday loop".into()),
            segments: vec![Segment { points }],
        }],
    };
    print!("{}", write_gpx(&gpx));
}
