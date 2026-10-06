use std::path::PathBuf;
use std::process::{Command, Output};

const SAMPLE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/sample-run.gpx");

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tracktidy"))
        .args(args)
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn scratch(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name)
}

#[test]
fn stats_on_the_sample() {
    let out = run(&["stats", SAMPLE]);
    assert!(out.status.success());
    let s = text(&out.stdout);
    assert!(s.contains("Distance   6.03 km"), "{s}");
    assert!(s.contains("Points     556"), "{s}");
}

#[test]
fn clean_removes_the_spike_and_the_stop() {
    let path = scratch("clean.gpx");
    let out = run(&["clean", SAMPLE, "-o", path.to_str().unwrap()]);
    assert!(out.status.success());
    let report = text(&out.stderr);
    assert!(report.contains("1 GPS spikes"), "{report}");
    assert!(report.contains("26 from stops"), "{report}");
    let stats = text(&run(&["stats", path.to_str().unwrap()]).stdout);
    assert!(stats.contains("Top speed  15.9 km/h"), "{stats}");
}

#[test]
fn clean_keeps_heart_rate() {
    let path = scratch("hr.gpx");
    assert!(
        run(&["clean", SAMPLE, "-o", path.to_str().unwrap()])
            .status
            .success()
    );
    let gpx = std::fs::read_to_string(path).unwrap();
    assert!(gpx.contains("xmlns:gpxtpx="));
    assert!(gpx.contains("<gpxtpx:hr>"));
}

#[test]
fn splits_in_miles() {
    let out = run(&["splits", SAMPLE, "--miles"]);
    assert!(out.status.success());
    let s = text(&out.stdout);
    assert_eq!(s.lines().count(), 4, "{s}");
    assert!(s.lines().next().unwrap().contains("1.00 mi"), "{s}");
}

#[test]
fn a_bad_home_is_refused() {
    let out = run(&["clean", SAMPLE, "--home", "91,0"]);
    assert!(!out.status.success());
    assert!(text(&out.stderr).contains("off the map"));
}

#[test]
fn a_missing_file_says_so() {
    let out = run(&["stats", "no-such-file.gpx"]);
    assert!(!out.status.success());
    assert!(text(&out.stderr).contains("couldn't read no-such-file.gpx"));
}

#[test]
fn stats_as_json() {
    let s = text(&run(&["stats", SAMPLE, "--json"]).stdout);
    assert!(
        s.starts_with("{\"points\":556,\"distance_m\":6027.1,"),
        "{s}"
    );
}

#[test]
fn cleaning_away_everything_writes_nothing() {
    let path = scratch("empty.gpx");
    let out = run(&[
        "clean",
        SAMPLE,
        "--trim",
        "5000",
        "-o",
        path.to_str().unwrap(),
    ]);
    assert!(!out.status.success());
    assert!(text(&out.stderr).contains("removed every point"));
    assert!(!path.exists());
}
