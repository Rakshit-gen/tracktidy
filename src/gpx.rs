//! Reading and writing GPX files.

use anyhow::{Context, Result, bail};
use quick_xml::{Reader, XmlVersion};
use quick_xml::events::{BytesStart, Event};

use crate::geo::LatLon;
use crate::time::{Seconds, parse_time};

/// One GPS fix.
#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    pub pos: LatLon,
    pub ele: Option<f64>,
    pub time: Option<Seconds>,
}

/// A stretch of recording without a break. Devices start a new segment when
/// you pause and resume, or when they lose the signal for a while.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Segment {
    pub points: Vec<Point>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Track {
    pub segments: Vec<Segment>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Gpx {
    pub tracks: Vec<Track>,
}

fn coord(start: &BytesStart, key: &[u8]) -> Result<f64> {
    let attr = start
        .try_get_attribute(key)?
        .with_context(|| format!("a track point has no {} attribute", String::from_utf8_lossy(key)))?;
    let value = attr.normalized_value(XmlVersion::Explicit1_0)?;
    value.trim().parse().with_context(|| format!("\"{value}\" isn't a coordinate"))
}

fn start_point(start: &BytesStart) -> Result<Point> {
    let pos = LatLon {
        lat: coord(start, b"lat")?,
        lon: coord(start, b"lon")?,
    };
    if !(-90.0..=90.0).contains(&pos.lat) || !(-180.0..=180.0).contains(&pos.lon) {
        bail!("a track point is off the map: {}, {}", pos.lat, pos.lon);
    }
    Ok(Point { pos, ele: None, time: None })
}

pub fn read_gpx(text: &str) -> Result<Gpx> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut gpx = Gpx::default();
    let mut point: Option<Point> = None;
    loop {
        match reader.read_event().context("the GPX isn't valid XML")? {
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == b"trk" => gpx.tracks.push(Track::default()),
            Event::Start(e) if e.local_name().as_ref() == b"trkseg" => {
                let track = gpx.tracks.last_mut().context("a track segment sits outside any track")?;
                track.segments.push(Segment::default());
            }
            Event::Start(e) if e.local_name().as_ref() == b"trkpt" => point = Some(start_point(&e)?),
            Event::Empty(e) if e.local_name().as_ref() == b"trkpt" => push(&mut gpx, start_point(&e)?)?,
            Event::Start(e) if point.is_some() && e.local_name().as_ref() == b"ele" => {
                let text = reader.read_text(e.name())?;
                let p = point.as_mut().unwrap();
                // A blank or broken elevation is common in phone exports; treat it as missing.
                p.ele = text.decode()?.trim().parse().ok();
            }
            Event::Start(e) if point.is_some() && e.local_name().as_ref() == b"time" => {
                let text = reader.read_text(e.name())?;
                let raw = text.decode()?;
                let p = point.as_mut().unwrap();
                p.time = Some(parse_time(&raw).with_context(|| format!("\"{}\" isn't a time", raw.trim()))?);
            }
            Event::End(e) if e.local_name().as_ref() == b"trkpt" => {
                if let Some(p) = point.take() {
                    push(&mut gpx, p)?;
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(gpx)
}

fn push(gpx: &mut Gpx, p: Point) -> Result<()> {
    let segment = gpx
        .tracks
        .last_mut()
        .and_then(|t| t.segments.last_mut())
        .context("a track point sits outside any track segment")?;
    segment.points.push(p);
    Ok(())
}
