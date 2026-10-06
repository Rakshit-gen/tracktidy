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
    /// The raw XML inside the point's <extensions>, where watches keep heart
    /// rate, cadence and temperature. Kept as is so cleaning a file doesn't
    /// throw that data away.
    pub extensions: Option<String>,
}

/// A stretch of recording without a break. Devices start a new segment when
/// you pause and resume, or when they lose the signal for a while.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Segment {
    pub points: Vec<Point>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Track {
    pub name: Option<String>,
    pub segments: Vec<Segment>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Gpx {
    /// The name in the file's metadata, if it has one.
    pub name: Option<String>,
    /// The app or device that wrote the file, from the creator attribute.
    pub creator: Option<String>,
    /// xmlns:prefix declarations from the root, which the kept extensions need
    /// (Garmin's heart rate lives under gpxtpx:, for example).
    pub namespaces: Vec<(String, String)>,
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
    Ok(Point {
        pos,
        ele: None,
        time: None,
        extensions: None,
    })
}

pub fn read_gpx(text: &str) -> Result<Gpx> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut gpx = Gpx::default();
    let mut point: Option<Point> = None;
    // Names of the open elements, so a <name> can tell whether it belongs to
    // the file's metadata, a track, or something we don't keep, like a waypoint.
    let mut open: Vec<Vec<u8>> = Vec::new();
    loop {
        let event = reader.read_event().context("the GPX isn't valid XML")?;
        match &event {
            Event::Start(e) if e.local_name().as_ref() == b"name" => {
                let raw = reader.read_text(e.name())?;
                let raw = raw.decode()?;
                let name = quick_xml::escape::unescape(raw.trim())
                    .map(|n| n.into_owned())
                    .unwrap_or_else(|_| raw.trim().to_string());
                match open.last().map(Vec::as_slice) {
                    Some(b"metadata") => gpx.name = Some(name),
                    Some(b"trk") => {
                        if let Some(track) = gpx.tracks.last_mut() {
                            track.name = Some(name);
                        }
                    }
                    _ => {}
                }
                continue;
            }
            Event::Start(e) if e.local_name().as_ref() == b"gpx" => {
                for attr in e.attributes() {
                    let attr = attr?;
                    let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
                    let value = attr.normalized_value(XmlVersion::Explicit1_0)?.into_owned();
                    if key == "creator" {
                        gpx.creator = Some(value);
                    } else if key.starts_with("xmlns:") {
                        gpx.namespaces.push((key, value));
                    }
                }
            }
            _ => {}
        }
        match &event {
            Event::Start(e) => open.push(e.local_name().as_ref().to_vec()),
            Event::End(_) => {
                open.pop();
            }
            _ => {}
        }
        match event {
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == b"trk" => gpx.tracks.push(Track::default()),
            Event::Start(e) if e.local_name().as_ref() == b"trkseg" => {
                let track = gpx.tracks.last_mut().context("a track segment sits outside any track")?;
                track.segments.push(Segment::default());
            }
            Event::Start(e) if e.local_name().as_ref() == b"trkpt" => point = Some(start_point(&e)?),
            Event::Empty(e) if e.local_name().as_ref() == b"trkpt" => push(&mut gpx, start_point(&e)?)?,
            Event::Start(e) if point.is_some() && e.local_name().as_ref() == b"ele" => {
                let text = reader.read_text(e.name())?;
                // read_text swallowed the closing tag, so close it here too.
                open.pop();
                let p = point.as_mut().unwrap();
                // A blank or broken elevation is common in phone exports; treat it as missing.
                p.ele = text.decode()?.trim().parse().ok();
            }
            Event::Start(e) if point.is_some() && e.local_name().as_ref() == b"time" => {
                let text = reader.read_text(e.name())?;
                open.pop();
                let raw = text.decode()?;
                let p = point.as_mut().unwrap();
                p.time = Some(parse_time(&raw).with_context(|| format!("\"{}\" isn't a time", raw.trim()))?);
            }
            Event::Start(e) if point.is_some() && e.local_name().as_ref() == b"extensions" => {
                let text = reader.read_text(e.name())?;
                open.pop();
                let raw = text.decode()?.trim().to_string();
                if !raw.is_empty() {
                    point.as_mut().unwrap().extensions = Some(raw);
                }
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

/// Writes a GPX 1.1 file. Waypoints, routes and other metadata in the original
/// aren't carried over; tracks, names, elevation, times and point extensions are.
pub fn write_gpx(gpx: &Gpx) -> String {
    use quick_xml::escape::escape;
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out += "<gpx version=\"1.1\" creator=\"";
    out += &escape(gpx.creator.as_deref().unwrap_or("tracktidy"));
    out += "\" xmlns=\"http://www.topografix.com/GPX/1/1\"";
    for (key, value) in &gpx.namespaces {
        out += &format!(" {key}=\"{}\"", escape(value.as_str()));
    }
    out += ">\n";
    if let Some(name) = &gpx.name {
        out += &format!("  <metadata>\n    <name>{}</name>\n  </metadata>\n", escape(name.as_str()));
    }
    for track in &gpx.tracks {
        out += "  <trk>\n";
        if let Some(name) = &track.name {
            out += &format!("    <name>{}</name>\n", escape(name.as_str()));
        }
        for segment in &track.segments {
            out += "    <trkseg>\n";
            for p in &segment.points {
                out += &format!("      <trkpt lat=\"{:.7}\" lon=\"{:.7}\">", p.pos.lat, p.pos.lon);
                if let Some(ele) = p.ele {
                    out += &format!("<ele>{ele:.1}</ele>");
                }
                if let Some(t) = p.time {
                    out += &format!("<time>{}</time>", crate::time::format_time(t));
                }
                if let Some(ext) = &p.extensions {
                    out += &format!("<extensions>{ext}</extensions>");
                }
                out += "</trkpt>\n";
            }
            out += "    </trkseg>\n";
        }
        out += "  </trk>\n";
    }
    out += "</gpx>\n";
    out
}
