# tracktidy

Tidies up a GPX track from a watch or phone before you upload or share it. It drops the GPS fixes that jump
across the river, collapses the minute you stood at a crossing into one stop, and can cut the first and last few
hundred metres so the map doesn't end at your front door. Heart rate, cadence and other device data in the file
are kept.

It also prints the numbers you'd want to check: distance, moving time, pace, climbing, top speed and splits.

## Install

```sh
cargo install --path .
```

Needs Rust 1.85 or newer. No network, no account.

## Use

The repo has a made-up 5 km run around Hyde Park in `examples/sample-run.gpx`, with a GPS spike, a 90 second
wait and noisy fixes, the way real files come out. Before cleaning:

```
$ tracktidy stats examples/sample-run.gpx
Distance   6.03 km
Elapsed    27:45
Moving     27:15
Pace       4:31 /km moving
Climb      25 m up, 26 m down
Top speed  217.8 km/h over 10 s
Points     556
```

One bad fix put a 217 km/h sprint and an extra kilometre in there. Cleaning, and trimming 200 m off each end:

```
$ tracktidy clean examples/sample-run.gpx --trim 200 -o run.gpx
Removed 72 of 556 points:
  1 GPS spikes
  26 from stops
  45 near the start and end
Distance 4.85 km, was 6.03 km.
Wrote run.gpx

$ tracktidy stats run.gpx
Distance   4.85 km
Elapsed    25:30
Moving     24:00
Pace       4:57 /km moving
Climb      10 m up, 22 m down
Top speed  15.9 km/h over 10 s
Points     484

$ tracktidy splits run.gpx
  1.00 km      4:55    4:55 /km
  2.00 km      4:57    4:57 /km
  3.00 km      6:22    6:22 /km
  4.00 km      4:56    4:56 /km
  4.85 km      4:19    5:03 /km
```

Split times are clock time, so the stop in the third kilometre shows up there.

### Options

`clean`:

- `--sport walk|run|ride` sets what counts as too fast to be real (15, 30 or 90 km/h) and the speed below which
  you count as stopped. Run is the default.
- `--max-speed` overrides the spike limit, in km/h.
- `--trim 200` cuts 200 m off both ends. Only the opening and closing stretches go; passing the start again
  halfway round a loop is left alone.
- `--home 51.5074,-0.1278` removes every point within 200 m of that spot, wherever it falls. `--home-radius`
  changes the distance.
- `--keep-pauses` leaves stops alone.
- `-o` picks the output file. Without it the result goes next to the input as `NAME.clean.gpx`. The input is
  never changed.

`stats` takes `--miles` and `--json` (metres and seconds). `splits` takes `--miles`.

## What it does to a track

In order:

1. Points whose time repeats or goes backwards are dropped.
2. A point is a spike if reaching it from the last good point would take more than the speed limit. It is
   dropped and the next point is checked against the same good point, so a run of bad fixes goes too. A first
   point that is too far from each of the next three, while those three agree, is the bad one and goes instead.
3. A stop is any stretch that stays within 5 m of where it began for 20 seconds or more. It keeps its first and
   last point, so the time is still there but the wandering is gone.
4. `--trim` and `--home`, if given.

Moving time counts the gaps between fixes where you went faster than the sport's threshold, and ignores gaps
longer than a minute. Climbing ignores changes under 3 m, since GPS elevation wobbles that much standing still.
Top speed is the best 10 second stretch, not the best single gap between two fixes.

Routes (`<rte>`) are read as tracks. Waypoints aren't kept in the output.

## Limits

- A spike is judged only by speed. A wrong fix that is close enough to be possible stays in.
- Stops are found with fixed numbers (5 m, 20 s). A slow walk through a crowd can look like a stop.
- Files without times (planned routes) can be trimmed and summed up, but spikes and stops can't be found.

## Regenerate the sample

```sh
cargo run --example make_sample > examples/sample-run.gpx
```

It is seeded, so the file comes out the same.
