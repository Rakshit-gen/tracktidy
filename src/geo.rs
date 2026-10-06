//! Distances on the earth's surface.

/// Mean radius of the earth in metres, the value most GPS software uses.
const EARTH_RADIUS_M: f64 = 6_371_008.8;

/// A position in decimal degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LatLon {
    pub lat: f64,
    pub lon: f64,
}

impl LatLon {
    /// Great-circle distance in metres (haversine). Plenty accurate for the
    /// few metres between GPS fixes and for whole runs or rides.
    pub fn distance_to(self, other: LatLon) -> f64 {
        let (p1, p2) = (self.lat.to_radians(), other.lat.to_radians());
        let dp = p2 - p1;
        let dl = (other.lon - self.lon).to_radians();
        let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
        2.0 * EARTH_RADIUS_M * a.sqrt().asin()
    }
}

/// The distance unit people read their splits and pace in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Km,
    Mile,
}

impl Unit {
    pub fn metres(self) -> f64 {
        match self {
            Unit::Km => 1000.0,
            Unit::Mile => 1609.344,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Unit::Km => "km",
            Unit::Mile => "mi",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_degree_of_latitude_is_about_111_km() {
        let d = LatLon { lat: 0.0, lon: 0.0 }.distance_to(LatLon { lat: 1.0, lon: 0.0 });
        assert!((d - 111_195.0).abs() < 5.0, "{d}");
    }

    #[test]
    fn longitude_shrinks_towards_the_poles() {
        let at = |lat: f64| LatLon { lat, lon: 0.0 }.distance_to(LatLon { lat, lon: 1.0 });
        assert!((at(60.0) - at(0.0) / 2.0).abs() < 50.0);
    }

    #[test]
    fn short_hops_are_exact_enough() {
        // London to Paris is about 343.5 km.
        let london = LatLon {
            lat: 51.5074,
            lon: -0.1278,
        };
        let paris = LatLon {
            lat: 48.8566,
            lon: 2.3522,
        };
        assert!((london.distance_to(paris) / 1000.0 - 343.5).abs() < 1.0);
        assert_eq!(london.distance_to(london), 0.0);
    }
}
