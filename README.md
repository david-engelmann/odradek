# odradek

Hunter agents stalk a compute cluster for zombie workloads, idle services, forgotten namespaces, and load balancers serving nobody. Nothing is deleted on suspicion; every target is proven dead first.

Odradek is a command line tool that works from recorded cluster snapshots and nothing else. It reads one snapshot file it is handed, classifies every entry by three mechanical proof rules over its recorded signal, owner, and in-flight evidence, and prints a scan listing every suspect and proven zombie with the evidence behind its classification. It also prints a reversible quarantine plan for one proven zombie or an estate plan covering every proven zombie in a snapshot, and an observation report that reads the outcome of a plan watch from a later snapshot. It executes nothing against any cluster, and the first action any plan describes is a reversible quarantine.

[![CI](https://github.com/david-engelmann/odradek/actions/workflows/ci.yml/badge.svg)](https://github.com/david-engelmann/odradek/actions/workflows/ci.yml) [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

```console
$ ./target/debug/odradek scan examples/snapshot.txt
snapshot 100000
counts live 2 suspect 3 proven 2 settled 1
zombie stale-1
  evidence for signal age 3600 seconds reaches the proof threshold of 360 seconds
  evidence for owner done-run is recorded finished
  evidence for holds cache with no in-flight marker
zombie stale-2
  evidence for signal age 1000 seconds reaches the proof threshold of 180 seconds
  evidence for owner ghost-7 is absent from the snapshot
  evidence for the entry holds nothing and carries no in-flight marker
suspect busy-1
  evidence against signal age 150 seconds is under the proof threshold of 180 seconds
  evidence against owner boss-run is recorded running
  evidence against holds cache, in-flight including the in-flight marker
suspect half-1
  evidence for signal age 5000 seconds reaches the proof threshold of 900 seconds
  missing no owner reference is recorded for this entry
  evidence for holds reports with no in-flight marker
suspect quiet-1
  missing no signal is recorded for this entry
  missing no owner reference is recorded for this entry
  evidence for the entry holds nothing and carries no in-flight marker
```

## Install

Odradek is a Rust crate that builds from source with cargo.

```sh
git clone https://github.com/david-engelmann/odradek.git
cd odradek
cargo build
```

The binary lands at target/debug/odradek, and the test suite runs with `cargo test`. The build needs a Unix environment such as Linux, macOS, or WSL on Windows.

## Quickstart

The smallest complete run prints a quarantine plan for one proven zombie in the committed example snapshot.

```console
$ ./target/debug/odradek plan examples/snapshot.txt stale-1
quarantine plan for stale-1
classification proven zombie
proof
  evidence for signal age 3600 seconds reaches the proof threshold of 360 seconds
  evidence for owner done-run is recorded finished
  evidence for holds cache with no in-flight marker
step 1 reversible, detach stale-1 from scheduler-1 so no new work reaches it
step 2 reversible, fence the recorded holds, cache, so nothing else claims them while the entry is watched
step 3 reversible, hold the quarantine for the observation window and watch for any signal from stale-1
observation window 1200 seconds
escalation, if stale-1 shows no signal for the whole observation window and no owner comes forward, a kill proposal may be drafted in a later round, and this plan never escalates itself
executes nothing, this plan is a document and the tool performs no part of it
```

## Usage

### scan

`odradek scan <snapshot>` classifies every entry in the snapshot and prints the snapshot line, the counts line, and one block for every suspect and every proven zombie, as the block at the top of this page shows. Each block carries exactly three evidence lines, one for each proof rule, naming the recorded fact the classification rests on. Entries classified live or settled are counted and never listed, and a snapshot with no suspects and no proven zombies prints the snapshot line and the counts line alone.

### plan

`odradek plan <snapshot> <entry>` prints a quarantine plan for one proven zombie, as the quickstart shows. Every step in the plan is marked reversible, and the plan states its observation window and its escalation condition and closes by stating that the tool executes nothing. Asked for an entry the snapshot does not carry, the plan refuses with one error line naming the absence, and asked for an entry classified live, suspect, or settled, it refuses with one error line naming the entry and its classification. With no entry named, `odradek plan <snapshot>` prints an estate plan holding one plan block for every proven zombie in the snapshot, in the order the scan lists them, and a snapshot with no proven zombies prints the estate framing alone.

With `--fixtures <directory>` the plan also asks the offline kill call about the proven zombie it names, reading recorded judgment fixtures from the directory it is handed. A trusted answer records its verdict on the plan and can advance the plan for operator approval, and it changes nothing else in the document, and advancing a plan performs no part of it and approves nothing by itself.

```console
$ ./target/debug/odradek plan examples/snapshot.txt stale-1 --fixtures fixtures
quarantine plan for stale-1
classification proven zombie
proof
  evidence for signal age 3600 seconds reaches the proof threshold of 360 seconds
  evidence for owner done-run is recorded finished
  evidence for holds cache with no in-flight marker
step 1 reversible, detach stale-1 from scheduler-1 so no new work reaches it
step 2 reversible, fence the recorded holds, cache, so nothing else claims them while the entry is watched
step 3 reversible, hold the quarantine for the observation window and watch for any signal from stale-1
observation window 1200 seconds
escalation, if stale-1 shows no signal for the whole observation window and no owner comes forward, a kill proposal may be drafted in a later round, and this plan never escalates itself
kill call verdict quarantine with confidence 0.9
verdict advances this plan, the plan is ready for an operator to approve, and the approval itself stays an operator decision the tool never takes
executes nothing, this plan is a document and the tool performs no part of it
```

### observe

`odradek observe <earlier> <later> <entry>` reads the outcome of the watch a plan set, from the snapshot the plan was written from and a second snapshot recorded later. Inside the observation window the report gives the outcome watch still running and no verdict, and once the elapsed seconds reach the window it gives exactly one verdict, quarantine held or quarantine broken, read from the later snapshot's recorded signal and owner facts alone.

```console
$ ./target/debug/odradek observe examples/snapshot.txt examples/later-snapshot.txt stale-1
observation report for stale-1
planned from snapshot 100000
observed at snapshot 101500
elapsed 1500 seconds
observation window 1200 seconds
outcome quarantine held
  evidence for the later snapshot records last signal 96400 for the entry, the same last signal the earlier snapshot recorded
  evidence for owner done-run is recorded finished
executes nothing, this report is a document and the tool performs no part of it
```

Every command reads only the files it is handed and writes nothing to any file. Documents print to the standard output and error lines to the standard error. A file that cannot be read or an input that breaks the recorded format refuses with one error line and exit status 2, and a malformed invocation exits with status 2. A line that cannot be printed because its stream is already gone answers with status 120 instead, and a document of 8192 bytes or more that cannot finish printing answers with status 1, and the reader ceilings and the digit limit answer with status 1, as ZH-28 and ZH-30 state.

## Status

The tool is complete at the surface its contract describes, with no stubbed or held back part, and the full CI gate runs on every change. Round 9 landed in PR 21, guarding the root help long set and every command's help short with derived pins and executing the two discrimination mutations Round 6 could not run. Closed rounds 9 of 100.

## License

MIT. See [LICENSE](LICENSE).
