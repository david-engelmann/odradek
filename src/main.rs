//! Command line entry point, the odradek binary.
//!
//! Subcommands
//!   scan     print the scan of one recorded cluster snapshot
//!   plan     print the quarantine plan for one proven zombie
//!            in one recorded cluster snapshot, or the estate
//!            plan over every proven zombie in the snapshot
//!            when no entry is named
//!   observe  print the observation report for one plan over
//!            a later recorded snapshot
//!
//! The argument parsing is clap, adopted under the
//! maintainer's tool choice ruling. Before clap sees the
//! arguments, a scan walks them the way the Python parser's
//! own scan walked them. It fires help where that scan fired
//! it, and where the invocation is clean it runs the command
//! itself on the slots the separator distribution assigns,
//! because the Python parser's separator reading, distributing
//! the values of each segment over the slots that remain when
//! the segment is reached, has no clap counterpart. A segment
//! is a run of values ended by each flag the parser consumes.
//! Within a segment the first separator token is discounted,
//! the separator joins the share of the value just before it,
//! and each share gives up its first separator token, and a
//! share that does not end holding exactly one token holds a
//! list, which surfaces in the uncaught family at the moment
//! that slot is read, a plan's entry list surfacing only after
//! the fixtures load because the load stands between the
//! snapshot parse and the plan build in the command's own
//! read order. Where the scan cannot assign the invocation it
//! hands the arguments to clap, as they stand when the scan
//! was blocked and gathered otherwise, since clap refuses the
//! same invocation in its own voice. Help text and malformed
//! invocation error text are clap's own forms. The documents
//! the tool emits and the one error line discipline for its
//! own refusals are unchanged. Odradek reads recorded
//! snapshots and prints documents and writes no file, and it
//! never connects to a live cluster.

use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::process::ExitCode;

use anyhow::Error as AnyError;
use clap::{Args, CommandFactory, Parser, Subcommand};
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

use odradek::errors::Error;
use odradek::judgment::{FixturePlayer, JudgmentSource};
use odradek::observe::build_observation;
use odradek::paths::py_path_folded;
use odradek::plan::{build_estate_plan, build_plan};
use odradek::pytext::{PyStr, is_decimal};
use odradek::scan::{classify, format_scan};
use odradek::snapshot::parse_snapshot;

#[derive(Parser)]
#[command(
    name = "odradek",
    about = "Hunter agents stalk a compute cluster for zombie workloads, idle services, forgotten namespaces, and load balancers serving nobody. Nothing is deleted on suspicion; every target is proven dead first.",
    disable_version_flag = true,
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    #[command(name = "scan", infer_long_args = true)]
    Scan(ScanArgs),
    #[command(name = "plan", infer_long_args = true)]
    Plan(PlanArgs),
    #[command(name = "observe", infer_long_args = true)]
    Observe(ObserveArgs),
}

#[derive(Args)]
struct ScanArgs {
    snapshot: OsString,
}

#[derive(Args)]
struct PlanArgs {
    snapshot: OsString,
    entry_id: Option<OsString>,
    #[arg(long = "fixtures")]
    fixtures: Option<OsString>,
}

#[derive(Args)]
struct ObserveArgs {
    earlier: OsString,
    later: OsString,
    entry_id: OsString,
}

/// The subcommands the command line declares.
#[derive(Clone, Copy, PartialEq)]
enum Cmd {
    Scan,
    Plan,
    Observe,
}

impl Cmd {
    fn name(self) -> &'static str {
        match self {
            Cmd::Scan => "scan",
            Cmd::Plan => "plan",
            Cmd::Observe => "observe",
        }
    }

    fn slots(self) -> usize {
        match self {
            Cmd::Scan => 1,
            Cmd::Plan => 2,
            Cmd::Observe => 3,
        }
    }
}

/// One subcommand of the command line, with the long flags the
/// derive model declares for it.
///
/// The flag tables of the scan come from clap's own model of
/// the derive structs, read through the command factory, so
/// the derive model stays the one owner of the flag set and
/// this layer restates none of it.
struct SubcommandSpec {
    name: String,
    cmd: Cmd,
    value_flags: Vec<String>,
    longs: Vec<String>,
}

fn subcommand_specs() -> Vec<SubcommandSpec> {
    Cli::command()
        .get_subcommands()
        .map(|sub| {
            let mut value_flags: Vec<String> = Vec::new();
            let mut longs: Vec<String> = Vec::new();
            for arg in sub.get_arguments() {
                let Some(long) = arg.get_long() else {
                    continue;
                };
                let name = format!("--{long}");
                if !longs.contains(&name) {
                    longs.push(name.clone());
                }
                if matches!(arg.get_action(), clap::ArgAction::Set) {
                    value_flags.push(name);
                }
            }
            // The factory's model of a subcommand does not
            // surface its generated help flag, which every
            // subcommand parser nevertheless carries, so the
            // table names it outright, as the root table does.
            if !longs.iter().any(|long| long == "--help") {
                longs.push("--help".to_string());
            }
            let cmd = match sub.get_name() {
                "scan" => Cmd::Scan,
                "plan" => Cmd::Plan,
                "observe" => Cmd::Observe,
                _ => unreachable!("the derive model declares three subcommands"),
            };
            SubcommandSpec {
                name: sub.get_name().to_string(),
                cmd,
                value_flags,
                longs,
            }
        })
        .collect()
}

fn root_longs() -> Vec<String> {
    // The root table is the one flag the root parser carries,
    // named outright. The unbuilt command the factory hands
    // back surfaces no root argument at all, and the built
    // command surfaces the generated help argument alone, as
    // the probe of Round 9 recorded, so the table holds the
    // help long because the root declares no other long. The
    // set stands guarded by the pin of ZH-50 in the test
    // module below. The subcommand tables are derived from
    // the factory, which does surface theirs.
    vec!["--help".to_string()]
}

/// The code points of one argument, decoded the way the
/// oracle's argument decoding decodes them, so the scan's
/// classification reads the same string the Python parser
/// read.
fn points_of(token: &OsStr) -> Vec<u32> {
    PyStr::from_surrogateescape(token.as_bytes()).0
}

/// Whether a body of code points reads as a negative number
/// under the oracle's pattern, one run of decimal digits, or
/// a run of digits, a point, and another run. The digit set
/// is the full Unicode decimal set, as it is in a Python text
/// pattern.
fn is_negative_number(points: &[u32]) -> bool {
    let Some(body) = points.strip_prefix(&[0x2D]) else {
        return false;
    };
    if !body.is_empty() && body.iter().all(|point| is_decimal(*point)) {
        return true;
    }
    match body.iter().position(|point| *point == 0x2E) {
        Some(dot) => {
            let (whole, fraction) = (&body[..dot], &body[dot + 1..]);
            whole.iter().all(|point| is_decimal(*point))
                && !fraction.is_empty()
                && fraction.iter().all(|point| is_decimal(*point))
        }
        None => false,
    }
}

/// Whether the negative number pattern matches the token,
/// where the pattern's end anchor also matches just before
/// one trailing newline, as a Python text pattern's does.
fn negative_number_matches(points: &[u32]) -> bool {
    is_negative_number(points) || points.strip_suffix(&[0x0A]).is_some_and(is_negative_number)
}

/// Whether a token reads as a positional string rather than
/// as an option, the classification the Python parser makes
/// when it meets the token. A bare word and a lone dash are
/// positional. A dash token carrying a space reads as
/// positional whatever its prefix, the one exception being
/// the short help family, whose letter after the dash decides
/// ahead of the space rule. A dash token without a space is
/// positional only when it matches the negative number
/// pattern. Any long token without a space is an option.
fn reads_as_positional(points: &[u32]) -> bool {
    if points == [0x2D, 0x2D] {
        return false;
    }
    if points.first() != Some(&0x2D) || points == [0x2D] {
        return true;
    }
    if points.get(1) == Some(&0x68) {
        return false;
    }
    if points.contains(&0x20) {
        return true;
    }
    if points.starts_with(&[0x2D, 0x2D]) {
        return false;
    }
    negative_number_matches(points)
}

/// How one token reads as a help request.
enum HelpRead {
    /// Not a help token.
    No,
    /// Help fires here.
    Fire,
    /// A help token carrying an explicit argument, which the
    /// Python parser refuses in its own voice, and nothing
    /// later can fire help. The token the stream handed to
    /// clap carries instead, in the form clap refuses there.
    Refuse(OsString),
}

/// The ASCII text of a run of code points, when every point
/// is ASCII, for prefix tests against the ASCII flag names.
fn ascii_text(points: &[u32]) -> Option<String> {
    points
        .iter()
        .map(|point| char::from_u32(*point).filter(|ch| ch.is_ascii()))
        .collect()
}

fn help_read(points: &[u32], token: &OsStr, longs: &[String]) -> HelpRead {
    if points.starts_with(&[0x2D, 0x2D]) {
        let name_points = match points.iter().position(|point| *point == 0x3D) {
            Some(at) => &points[..at],
            None => points,
        };
        if name_points.len() > 2 {
            let Some(name) = ascii_text(name_points) else {
                return HelpRead::No;
            };
            let matches: Vec<&String> = longs
                .iter()
                .filter(|long| long.starts_with(&name))
                .collect();
            if matches.len() == 1 && matches[0] == "--help" {
                if points.contains(&0x3D) {
                    return HelpRead::Refuse(token.to_os_string());
                }
                return HelpRead::Fire;
            }
        }
        return HelpRead::No;
    }
    if points.first() == Some(&0x2D) && points.get(1) == Some(&0x68) {
        return match points.get(2) {
            None => HelpRead::Fire,
            Some(0x3D) => {
                let mut replacement = OsString::from("--help=");
                replacement.push(OsStr::from_bytes(token.as_bytes().get(3..).unwrap_or(&[])));
                HelpRead::Refuse(replacement)
            }
            Some(0x2D) => {
                let mut replacement = OsString::from("--help=");
                replacement.push(OsStr::from_bytes(token.as_bytes().get(2..).unwrap_or(&[])));
                HelpRead::Refuse(replacement)
            }
            Some(_) => HelpRead::Fire,
        };
    }
    HelpRead::No
}

/// How a token reads against the subcommand's one value flag.
enum FlagRead {
    /// Not the value flag.
    No,
    /// The flag with a joined value, the bytes after its first
    /// equals sign.
    Joined(OsString),
    /// The flag standing alone, expecting its value as the
    /// next token.
    Standing,
}

fn flag_read(points: &[u32], token: &OsStr, spec: &SubcommandSpec) -> FlagRead {
    if !points.starts_with(&[0x2D, 0x2D]) || spec.value_flags.is_empty() {
        return FlagRead::No;
    }
    let stem_points = match points.iter().position(|point| *point == 0x3D) {
        Some(at) => &points[..at],
        None => points,
    };
    if stem_points.len() <= 2 {
        return FlagRead::No;
    }
    let Some(stem) = ascii_text(stem_points) else {
        return FlagRead::No;
    };
    let matched: Vec<&String> = spec
        .value_flags
        .iter()
        .filter(|flag| flag.starts_with(&stem))
        .collect();
    if matched.len() != 1 {
        return FlagRead::No;
    }
    match token.as_bytes().iter().position(|byte| *byte == b'=') {
        Some(at) => FlagRead::Joined(OsStr::from_bytes(&token.as_bytes()[at + 1..]).to_os_string()),
        None => FlagRead::Standing,
    }
}

/// One token inside a segment of values.
#[derive(Clone)]
enum SegTok {
    Value(OsString),
    Sep,
}

/// One positional slot after the separator distribution.
enum Slot {
    One(OsString),
    Listed,
}

/// A command the distribution assigned, ready to run.
struct Assigned {
    cmd: Cmd,
    slots: Vec<Slot>,
    fixtures: Option<OsString>,
}

/// The outcome of the argument scan.
enum Scan {
    /// Print the help of the named subcommand parser, or of
    /// the top level parser when the name is absent, at
    /// status 0.
    FireHelp(Option<Cmd>),
    /// Run the command the distribution assigned.
    Assigned(Assigned),
    /// Hand these arguments to clap.
    Run(Vec<OsString>),
}

/// Distribute the segments over the command's slots, in
/// order, each segment filling the next slots that remain
/// when it is reached.
///
/// Within a segment the first separator token is discounted.
/// The first separator joins the share of the value just
/// before it, or the first share when it leads, and every
/// other token, a later separator among them, is a value for
/// the next share in order. Each share then gives up its first
/// separator token, and a share that does not end holding
/// exactly one token holds a list.
fn distribute(segments: &[Vec<SegTok>], action_count: usize) -> Option<Vec<Slot>> {
    // A trailing separator that closes a segment holding no
    // positional, once any flag has been consumed, is a usage
    // refusal in the oracle, not a completed invocation: the
    // separator ends the argument string with an empty final
    // group, and the parser's positional match fails on it.
    // Adjudicated against the real parser for this command
    // set before the port was written (ZH-27).
    if segments.len() > 1
        && let Some(last) = segments.iter().rev().find(|segment| !segment.is_empty())
        && last.len() == 1
        && matches!(last[0], SegTok::Sep)
    {
        return None;
    }
    let mut slots: Vec<Slot> = Vec::new();
    for segment in segments {
        if segment.is_empty() {
            continue;
        }
        let has_separator = segment.iter().any(|token| matches!(token, SegTok::Sep));
        let arguments = segment.len() - usize::from(has_separator);
        if arguments == 0 {
            continue;
        }
        if slots.len() + arguments > action_count {
            return None;
        }
        let first_separator = segment
            .iter()
            .position(|token| matches!(token, SegTok::Sep));
        let mut shares: Vec<Vec<&SegTok>> = vec![Vec::new(); arguments];
        let mut seen = 0usize;
        for (index, token) in segment.iter().enumerate() {
            if Some(index) == first_separator {
                shares[seen.saturating_sub(1)].push(token);
            } else {
                shares[seen].push(token);
                seen += 1;
            }
        }
        for share in &mut shares {
            if let Some(at) = share.iter().position(|token| matches!(token, SegTok::Sep)) {
                share.remove(at);
            }
            match share.len() {
                1 => match share[0] {
                    SegTok::Value(value) => slots.push(Slot::One(value.clone())),
                    SegTok::Sep => slots.push(Slot::One(OsString::from("--"))),
                },
                _ => slots.push(Slot::Listed),
            }
        }
    }
    if slots.len() != action_count {
        return None;
    }
    Some(slots)
}

/// Walk the arguments the way the Python parser's scan walked
/// them, as the module doc describes.
fn scan(tokens: &[OsString]) -> Scan {
    let specs = subcommand_specs();
    let root = root_longs();
    let mut raw: Vec<OsString> = Vec::new();
    let mut out: Vec<OsString> = Vec::new();
    let mut segments: Vec<Vec<SegTok>> = vec![Vec::new()];
    let mut current: Option<usize> = None;
    let mut pending = false;
    let mut fixtures_slot: Option<usize> = None;
    let mut fixtures_value: Option<OsString> = None;
    let mut blocked = false;
    let mut separated = false;
    // Whether nothing but the subcommand word and the consumed
    // fixtures flag has been gathered. An unknown optional in
    // the gathered stream means the Python parser refused the
    // invocation as unrecognized, so the direct run the
    // distribution would otherwise take must not happen.
    let mut clean = true;
    for token in tokens {
        if separated {
            raw.push(token.clone());
            if token.as_bytes() == b"--" {
                segments
                    .last_mut()
                    .expect("a segment always stands")
                    .push(SegTok::Sep);
            } else {
                segments
                    .last_mut()
                    .expect("a segment always stands")
                    .push(SegTok::Value(token.clone()));
            }
            continue;
        }
        if blocked {
            raw.push(token.clone());
            continue;
        }
        let points = points_of(token);
        if pending {
            raw.push(token.clone());
            if reads_as_positional(&points) {
                pending = false;
                fixtures_value = Some(token.clone());
                if let Some(slot) = fixtures_slot {
                    let mut joined = out[slot].clone();
                    joined.push("=");
                    joined.push(token);
                    out[slot] = joined;
                }
            } else {
                blocked = true;
            }
            continue;
        }
        if points == [0x2D, 0x2D] {
            raw.push(token.clone());
            if current.is_some() {
                segments
                    .last_mut()
                    .expect("a segment always stands")
                    .push(SegTok::Sep);
                separated = true;
            } else {
                blocked = true;
            }
            continue;
        }
        let longs: &[String] = match current {
            Some(index) => &specs[index].longs,
            None => &root,
        };
        match help_read(&points, token, longs) {
            HelpRead::Fire => {
                return Scan::FireHelp(current.map(|index| specs[index].cmd));
            }
            HelpRead::Refuse(replacement) => {
                blocked = true;
                raw.push(replacement);
                continue;
            }
            HelpRead::No => {}
        }
        raw.push(token.clone());
        if current.is_none() {
            if let Some(index) = specs
                .iter()
                .position(|spec| spec.name.as_bytes() == token.as_bytes())
            {
                current = Some(index);
                out.push(token.clone());
                continue;
            }
            if reads_as_positional(&points) {
                blocked = true;
            } else {
                clean = false;
                out.push(token.clone());
            }
            continue;
        }
        let spec = &specs[current.expect("a subcommand is selected")];
        match flag_read(&points, token, spec) {
            FlagRead::Joined(value) => {
                fixtures_value = Some(value);
                if let Some(old) = fixtures_slot.take() {
                    out.remove(old);
                }
                out.push(token.clone());
                fixtures_slot = Some(out.len() - 1);
                segments.push(Vec::new());
                continue;
            }
            FlagRead::Standing => {
                if let Some(old) = fixtures_slot.take() {
                    out.remove(old);
                }
                out.push(token.clone());
                fixtures_slot = Some(out.len() - 1);
                pending = true;
                segments.push(Vec::new());
                continue;
            }
            FlagRead::No => {}
        }
        if reads_as_positional(&points) {
            segments
                .last_mut()
                .expect("a segment always stands")
                .push(SegTok::Value(token.clone()));
        } else {
            clean = false;
            out.push(token.clone());
        }
    }
    if blocked || pending {
        return Scan::Run(raw);
    }
    let Some(index) = current else {
        return Scan::Run(out);
    };
    let cmd = specs[index].cmd;
    let assigned = if clean {
        match cmd {
            // The plan takes two shares in its single form
            // and one share in its estate form, and the two
            // share reading is tried first so a complete
            // single invocation is never read as an estate.
            Cmd::Plan => distribute(&segments, 2).or_else(|| distribute(&segments, 1)),
            _ => distribute(&segments, cmd.slots()),
        }
    } else {
        None
    };
    match assigned {
        Some(slots) => Scan::Assigned(Assigned {
            cmd,
            slots,
            fixtures: fixtures_value,
        }),
        None => {
            if segments.iter().any(|segment| !segment.is_empty()) {
                out.push(OsString::from("--"));
                for segment in &segments {
                    for token in segment {
                        match token {
                            SegTok::Value(value) => out.push(value.clone()),
                            SegTok::Sep => out.push(OsString::from("--")),
                        }
                    }
                }
            }
            Scan::Run(out)
        }
    }
}

/// The size of the Python tool's standard output buffer. A
/// print whose failure surfaces while the output still fits
/// the buffer is the failed flush family, and one that
/// surfaces at or past it is the uncaught family (ZH-28).
const STDOUT_BUFFER_BYTES: usize = 8192;

/// The port's own line for a print that failed inside the
/// buffer, standing where the Python tool's interpreter
/// printed its exception ignored noise.
const FAILED_PRINT_LINE: &str = "odradek could not print its document";

/// The port's own line for a print that failed at or past the
/// buffer, standing where the Python tool printed a traceback.
const UNCAUGHT_PRINT_LINE: &str = "odradek: the document print failed";

/// The port's own line for a list standing in a slot a command
/// reads as one path, standing where the Python tool printed
/// a traceback for its type error.
const LIST_PATH_LINE: &str =
    "odradek: an argument arrived as a list where a single path was expected";

/// The port's own line for a list standing in the entry slot,
/// standing where the Python tool printed a traceback for its
/// unhashable type error at the entry lookup.
const LIST_ENTRY_LINE: &str =
    "odradek: an argument arrived as a list where a single entry identifier was expected";

/// A print failure discovered while the output still fit the
/// standard output buffer.
#[derive(Debug)]
struct FailedFlush(String);

impl std::fmt::Display for FailedFlush {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for FailedFlush {}

/// A print failure discovered at or past the buffer.
#[derive(Debug)]
struct UncaughtPrint(String);

impl std::fmt::Display for UncaughtPrint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for UncaughtPrint {}

/// The platform error text of a failed read, without the
/// suffix the Rust error rendering adds, so the refusal line
/// carries the same words the Python tool's handler reads off
/// its own error.
fn strerror(error: &std::io::Error) -> String {
    let text = error.to_string();
    match error.raw_os_error() {
        Some(code) => text
            .strip_suffix(&format!(" (os error {code})"))
            .map_or(text.clone(), str::to_string),
        None => text,
    }
}

/// Print one document to standard output through one checked
/// write, as the Python tool prints through its buffered
/// write and its exit flush.
fn print_out(text: &str) -> anyhow::Result<()> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    lock.write_all(text.as_bytes())
        .and_then(|()| lock.flush())
        .map_err(|_| {
            if text.len() < STDOUT_BUFFER_BYTES {
                AnyError::new(FailedFlush(FAILED_PRINT_LINE.to_string()))
            } else {
                AnyError::new(UncaughtPrint(UNCAUGHT_PRINT_LINE.to_string()))
            }
        })
}

/// Write one error line to the error stream through one
/// checked write, and answer with the given status, or with
/// status 120 when the write itself fails.
fn answer_error_line(text: &str, status: u8) -> ExitCode {
    let stderr = std::io::stderr();
    let mut lock = stderr.lock();
    match lock.write_all(text.as_bytes()) {
        Ok(()) => ExitCode::from(status),
        Err(_) => ExitCode::from(120),
    }
}

/// Answer a failed document print.
fn answer_print_failure(failure: AnyError) -> ExitCode {
    if let Some(failed) = failure.downcast_ref::<FailedFlush>() {
        answer_error_line(&format!("{}\n", failed.0), 120)
    } else if let Some(uncaught) = failure.downcast_ref::<UncaughtPrint>() {
        answer_error_line(&format!("{}\n", uncaught.0), 1)
    } else {
        answer_error_line(&format!("error: {failure}\n"), 2)
    }
}

/// Print a document and answer for its print.
fn print_document(text: &str) -> ExitCode {
    match print_out(text) {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => answer_print_failure(failure),
    }
}

/// Answer a library error on the terms the Python command
/// answered it. The refused family prints its line after the
/// error prefix at status 2, and the uncaught family prints
/// the port's own line at status 1.
fn answer_library_error(error: Error) -> ExitCode {
    if error.is_uncaught() {
        answer_error_line(&format!("{}\n", error.message()), 1)
    } else {
        answer_error_line(&format!("error: {}\n", error.message()), 2)
    }
}

/// Answer a clap error the way the command line answers it.
///
/// An error clap prints to standard error, a malformed
/// invocation, is printed there and answered with its own
/// exit code, and it goes through the same handled error
/// stream write as the tool's own lines, so an error stream
/// that cannot take the text answers with status 120. An
/// error clap prints to standard output is the help text, and
/// help output takes the failed write discipline of every
/// document the tool prints, because the Python tool's help
/// goes to the same buffered standard output its documents
/// do.
fn answer_clap_error(error: clap::Error) -> ExitCode {
    if error.use_stderr() {
        let code = error.exit_code();
        let text = error.render().to_string();
        let text = if text.ends_with('\n') {
            text
        } else {
            format!("{text}\n")
        };
        return answer_error_line(&text, if code == 0 { 2 } else { code as u8 });
    }
    let text = error.render().to_string();
    print_document(&text)
}

/// Read one slot's snapshot as text, answering a failure on
/// the spot. The read opens the folded form of the path as
/// written, as the oracle's path object folds before it
/// opens. The cannot read line and the not readable as UTF-8
/// line name the path as written, as does the source every
/// later refusal carries (ZH-29).
fn read_slot(slot: &Slot) -> Result<(PyStr, PyStr), ExitCode> {
    let raw = match slot {
        Slot::Listed => {
            return Err(answer_error_line(&format!("{LIST_PATH_LINE}\n"), 1));
        }
        Slot::One(raw) => raw,
    };
    // The fold drops empty and single dot components and
    // reads an empty path as the single dot, so a file
    // spelled with a trailing slash or a trailing dot
    // component opens as the file itself, where a raw open
    // fails with not a directory.
    let folded_bytes = py_path_folded(raw.as_bytes());
    let content = match std::fs::read(OsStr::from_bytes(&folded_bytes)) {
        Ok(content) => content,
        Err(error) => {
            let written = PyStr::from_surrogateescape(raw.as_bytes());
            return Err(answer_error_line(
                &format!(
                    "error: the snapshot at {written} cannot be read, {}\n",
                    strerror(&error)
                ),
                2,
            ));
        }
    };
    let source = PyStr::from_surrogateescape(raw.as_bytes());
    match String::from_utf8(content) {
        Ok(text) => Ok((PyStr::from_str(&text), source)),
        Err(_) => Err(answer_error_line(
            &format!("error: the snapshot at {source} is not readable as UTF-8 text\n"),
            2,
        )),
    }
}

/// The entry identifier a slot holds, answering a list on the
/// spot with the uncaught family's line.
fn entry_slot(slot: &Slot) -> Result<PyStr, ExitCode> {
    match slot {
        Slot::Listed => Err(answer_error_line(&format!("{LIST_ENTRY_LINE}\n"), 1)),
        Slot::One(raw) => Ok(PyStr::from_surrogateescape(raw.as_bytes())),
    }
}

/// Run one assigned command on its slots, in the read order
/// the Python command reads: the snapshot reads and parses in
/// slot order, then the plan's fixtures load, then the build.
fn run_assigned(assigned: &Assigned) -> ExitCode {
    match assigned.cmd {
        Cmd::Scan => {
            let (text, source) = match read_slot(&assigned.slots[0]) {
                Ok(pair) => pair,
                Err(code) => return code,
            };
            match parse_snapshot(&text, &source) {
                Ok(snapshot) => match classify(&snapshot) {
                    Ok(verdicts) => print_document(&format_scan(&snapshot, &verdicts)),
                    Err(error) => answer_library_error(error),
                },
                Err(error) => answer_library_error(error),
            }
        }
        Cmd::Plan => {
            let (text, source) = match read_slot(&assigned.slots[0]) {
                Ok(pair) => pair,
                Err(code) => return code,
            };
            let snapshot = match parse_snapshot(&text, &source) {
                Ok(snapshot) => snapshot,
                Err(error) => return answer_library_error(error),
            };
            let verdicts = match classify(&snapshot) {
                Ok(verdicts) => verdicts,
                Err(error) => return answer_library_error(error),
            };
            let player = match &assigned.fixtures {
                None => None,
                Some(directory) => match FixturePlayer::load(directory.as_bytes()) {
                    Ok(player) => Some(player),
                    Err(error) => return answer_library_error(error),
                },
            };
            let source_ref = player.as_ref().map(|held| held as &dyn JudgmentSource);
            if assigned.slots.len() == 1 {
                // The estate form names no entry, so no entry
                // slot is read and no entry level refusal
                // stands. The snapshot read, the parse, the
                // classification, and the fixtures load above
                // are the whole refusal order ahead of it.
                return match build_estate_plan(&snapshot, &verdicts, source_ref) {
                    Ok(document) => print_document(&document),
                    Err(error) => answer_library_error(error),
                };
            }
            let entry_id = match entry_slot(&assigned.slots[1]) {
                Ok(id) => id,
                Err(code) => return code,
            };
            match build_plan(&snapshot, &verdicts, &entry_id, source_ref) {
                Ok(document) => print_document(&document),
                Err(error) => answer_library_error(error),
            }
        }
        Cmd::Observe => {
            let (earlier_text, earlier_source) = match read_slot(&assigned.slots[0]) {
                Ok(pair) => pair,
                Err(code) => return code,
            };
            let earlier = match parse_snapshot(&earlier_text, &earlier_source) {
                Ok(snapshot) => snapshot,
                Err(error) => return answer_library_error(error),
            };
            let (later_text, later_source) = match read_slot(&assigned.slots[1]) {
                Ok(pair) => pair,
                Err(code) => return code,
            };
            let later = match parse_snapshot(&later_text, &later_source) {
                Ok(snapshot) => snapshot,
                Err(error) => return answer_library_error(error),
            };
            let entry_id = match entry_slot(&assigned.slots[2]) {
                Ok(id) => id,
                Err(code) => return code,
            };
            let verdicts = match classify(&earlier) {
                Ok(verdicts) => verdicts,
                Err(error) => return answer_library_error(error),
            };
            match build_observation(&earlier, &later, &verdicts, &entry_id) {
                Ok(document) => print_document(&document),
                Err(error) => answer_library_error(error),
            }
        }
    }
}

fn run() -> ExitCode {
    let mut args = std::env::args_os();
    let program = args.next().unwrap_or_else(|| OsString::from("odradek"));
    let rest: Vec<OsString> = args.collect();
    match scan(&rest) {
        Scan::FireHelp(cmd) => {
            let mut argv = vec![program];
            if let Some(cmd) = cmd {
                argv.push(OsString::from(cmd.name()));
            }
            argv.push(OsString::from("--help"));
            match Cli::try_parse_from(argv) {
                Ok(_) => ExitCode::from(0),
                Err(error) => answer_clap_error(error),
            }
        }
        Scan::Assigned(assigned) => run_assigned(&assigned),
        Scan::Run(tokens) => {
            match Cli::try_parse_from(std::iter::once(program).chain(tokens)) {
                Ok(_) => {
                    // The scan could not assign the invocation,
                    // so the Python parser refused it. Clap's
                    // acceptance of the gathered stream is
                    // answered with a usage refusal, since the
                    // functional outcome is a refusal either way.
                    let usage = Cli::command().render_usage().to_string();
                    answer_error_line(
                        &format!("error: the invocation does not parse\n{usage}\n"),
                        2,
                    )
                }
                Err(error) => answer_clap_error(error),
            }
        }
    }
}

fn main() -> ExitCode {
    // The Python reader reaches documents nested thousands of
    // containers deep before its own ceiling refuses them, and
    // the port's reader and value walks recurse once per
    // container on the deepest path, as the Python tool's do.
    // The binary therefore runs on a worker thread with a
    // stack sized for that depth, so a document at the ceiling
    // is refused or carried on the tool's own terms and never
    // on the stack's.
    match std::thread::Builder::new()
        .stack_size(512 * 1024 * 1024)
        .spawn(run)
    {
        Ok(handle) => handle.join().unwrap_or(ExitCode::from(2)),
        Err(_) => run(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Derive the root long set from the built command in
    /// the manner subcommand_specs derives the long set of
    /// a subcommand, collecting the long of every argument
    /// the root command carries and completing the set with
    /// the help long where it stands absent.
    fn derived_root_longs() -> Vec<String> {
        let mut command = Cli::command();
        command.build();
        let mut longs: Vec<String> = Vec::new();
        for arg in command.get_arguments() {
            let Some(long) = arg.get_long() else {
                continue;
            };
            let name = format!("--{long}");
            if !longs.contains(&name) {
                longs.push(name);
            }
        }
        if !longs.iter().any(|long| long == "--help") {
            longs.push("--help".to_string());
        }
        longs
    }

    /// ZH-50. The hand set the scan reads help tokens
    /// against on the root, the table root_longs returns,
    /// holds the help long alone, and it holds it because
    /// the root command declares no other long. This pin
    /// derives the root long set from the built command and
    /// asserts the derived set is the one help long, so a
    /// long added to the root command, declared as an
    /// argument or raised by an attribute, fails here in
    /// the open instead of leaving the hand set standing
    /// behind the parser in silence.
    #[test]
    fn root_long_set_is_the_one_help_long() {
        assert_eq!(derived_root_longs(), vec!["--help".to_string()]);
        assert_eq!(root_longs(), derived_root_longs());
    }

    /// Derive the short of the help argument of every
    /// command in the tree, walking the built command
    /// recursively, each command first and then its
    /// subcommands in their own order, finding each
    /// argument by its help long in the manner
    /// derived_root_longs collects the longs of the root
    /// command, and standing absent in a command's place
    /// where that command carries no argument with the
    /// help long. ZH-54 gives the walk this recursive
    /// form, where the derivation of ZH-51 as first
    /// written walked one level deep.
    fn derived_help_shorts() -> Vec<Option<char>> {
        fn help_short(command: &clap::Command) -> Option<char> {
            for arg in command.get_arguments() {
                if arg.get_long() == Some("help") {
                    return arg.get_short();
                }
            }
            None
        }
        fn collect(command: &clap::Command, shorts: &mut Vec<Option<char>>) {
            shorts.push(help_short(command));
            for sub in command.get_subcommands() {
                collect(sub, shorts);
            }
        }
        let mut command = Cli::command();
        command.build();
        let mut shorts = Vec::new();
        collect(&command, &mut shorts);
        shorts
    }

    /// ZH-51. The two reads that test the second point of
    /// a token against the code point 0x68, the short help
    /// test of reads_as_positional and the short help
    /// branch of help_read, test that point in every
    /// context the scan walks, on the root and behind a
    /// subcommand alike, and they rest on the help
    /// argument of every command in the tree carrying
    /// the short h. This pin derives the help short of
    /// every command from the built command and asserts
    /// each one is the letter h, so a help short
    /// customized to another letter on any command, or a
    /// command left with no help argument at all, fails
    /// here in the open instead of leaving the two reads
    /// standing behind the parser in silence.
    #[test]
    fn every_help_short_is_the_letter_h() {
        let shorts = derived_help_shorts();
        assert_eq!(shorts.len(), 4);
        assert!(shorts.iter().all(|short| *short == Some('h')));
    }

    /// The tokens of one argument stream, from their text
    /// forms.
    fn stream(parts: &[&str]) -> Vec<OsString> {
        parts.iter().map(OsString::from).collect()
    }

    /// What one slot of an assigned answer is expected to
    /// hold, one token or a list.
    enum Want {
        One(&'static str),
        Listed,
    }

    /// Assert the walk fires the help of the named command,
    /// or of the top level when the name is absent.
    fn assert_fire_help(parts: &[&str], want: Option<Cmd>) {
        match scan(&stream(parts)) {
            Scan::FireHelp(cmd) => assert!(cmd == want, "the walk fired the wrong help"),
            Scan::Assigned(_) => panic!("expected help to fire, the walk assigned the stream"),
            Scan::Run(_) => panic!("expected help to fire, the walk handed the stream over"),
        }
    }

    /// Assert the walk hands the stream over, token for
    /// token.
    fn assert_run(parts: &[&str], want: &[&str]) {
        match scan(&stream(parts)) {
            Scan::Run(tokens) => assert_eq!(tokens, stream(want)),
            Scan::FireHelp(_) => panic!("expected a hand over, help fired"),
            Scan::Assigned(_) => panic!("expected a hand over, the walk assigned the stream"),
        }
    }

    /// Assert the walk assigns the stream to the named
    /// command, with the slots and the fixtures value the
    /// census derives.
    fn assert_assigned(
        parts: &[&str],
        want_cmd: Cmd,
        want_slots: &[Want],
        want_fixtures: Option<&str>,
    ) {
        match scan(&stream(parts)) {
            Scan::Assigned(assigned) => {
                assert!(
                    assigned.cmd == want_cmd,
                    "the walk assigned the wrong command"
                );
                assert_eq!(assigned.slots.len(), want_slots.len(), "slot count");
                for (slot, want) in assigned.slots.iter().zip(want_slots.iter()) {
                    match (slot, want) {
                        (Slot::One(value), Want::One(text)) => {
                            assert!(value.as_os_str() == OsStr::new(text), "slot value");
                        }
                        (Slot::Listed, Want::Listed) => {}
                        (Slot::One(_), Want::Listed) => {
                            panic!("expected a list in this slot, the walk assigned one token");
                        }
                        (Slot::Listed, Want::One(_)) => {
                            panic!("expected one token in this slot, the walk assigned a list");
                        }
                    }
                }
                assert!(
                    assigned.fixtures.as_deref() == want_fixtures.map(OsStr::new),
                    "fixtures value"
                );
            }
            Scan::FireHelp(_) => panic!("expected an assignment, help fired"),
            Scan::Run(_) => panic!("expected an assignment, the walk handed the stream over"),
        }
    }

    /// ZH-63, family 1. The long arm of the help read fires
    /// on the exact help long and on its unique abbreviation,
    /// against the root table and each subcommand table.

    #[test]
    fn scan_help_long_exact_fires_at_the_root() {
        assert_fire_help(&["--help"], None);
    }

    #[test]
    fn scan_help_long_exact_fires_behind_observe() {
        assert_fire_help(&["observe", "--help"], Some(Cmd::Observe));
    }

    #[test]
    fn scan_help_long_abbreviation_fires_at_the_root() {
        assert_fire_help(&["--h"], None);
    }

    #[test]
    fn scan_help_long_abbreviation_fires_behind_scan() {
        assert_fire_help(&["scan", "--he"], Some(Cmd::Scan));
    }

    #[test]
    fn scan_help_long_abbreviation_fires_behind_plan() {
        assert_fire_help(&["plan", "--h"], Some(Cmd::Plan));
    }

    /// ZH-63, family 1. A help long carrying an explicit
    /// value refuses, and the token itself stands in the
    /// handed over stream. The trailing help token separates
    /// the refusal from a mere no read, since the refusal's
    /// block keeps it from firing while a no read lets it
    /// fire (the red team's Round 12 Minor 1, adopted in the
    /// owner fix pass).

    #[test]
    fn scan_help_long_with_a_value_refuses_at_the_root() {
        assert_run(&["--help=x", "-h"], &["--help=x", "-h"]);
    }

    #[test]
    fn scan_help_long_abbreviation_with_a_value_refuses_behind_scan() {
        assert_run(&["scan", "--h=x", "-h"], &["scan", "--h=x", "-h"]);
    }

    /// ZH-63, family 1. A name longer than the help long
    /// matches nothing, the match test being the long
    /// starting with the name, and the token is gathered as
    /// an unknown option.

    #[test]
    fn scan_help_longer_name_reads_no_help() {
        assert_run(&["scan", "--helpful"], &["scan", "--helpful"]);
    }

    /// ZH-63, family 1. A name of two points never reaches
    /// the match, the arm requiring more than two points.

    #[test]
    fn scan_help_name_of_two_points_reads_no_help() {
        assert_run(&["scan", "--=x"], &["scan", "--=x"]);
    }

    /// ZH-63, family 1. A name carrying a non-ASCII point
    /// reads no help, the name test reading ASCII alone.

    #[test]
    fn scan_help_non_ascii_name_reads_no_help() {
        assert_run(&["scan", "--hé"], &["scan", "--hé"]);
    }

    /// ZH-63, family 1. A name matching no long in the table
    /// reads no help and is gathered as an unknown option.

    #[test]
    fn scan_help_name_matching_no_long_reads_no_help() {
        assert_run(&["observe", "--bogus"], &["observe", "--bogus"]);
    }

    /// ZH-63, family 2. The short arm of the help read fires
    /// on the bare short and on any continuation but the two
    /// refusing ones.

    #[test]
    fn scan_help_short_fires_at_the_root() {
        assert_fire_help(&["-h"], None);
    }

    #[test]
    fn scan_help_short_fires_behind_observe() {
        assert_fire_help(&["observe", "-h"], Some(Cmd::Observe));
    }

    #[test]
    fn scan_help_short_bundle_fires_behind_scan() {
        assert_fire_help(&["scan", "-hx"], Some(Cmd::Scan));
    }

    #[test]
    fn scan_help_short_bundle_fires_behind_plan() {
        assert_fire_help(&["plan", "-hh"], Some(Cmd::Plan));
    }

    /// ZH-63, family 2. A space as the third point fires,
    /// the help read running ahead of the space rule of the
    /// classification read.

    #[test]
    fn scan_help_short_with_a_space_fires_behind_scan() {
        assert_fire_help(&["scan", "-h x"], Some(Cmd::Scan));
    }

    /// ZH-63, family 2. An equals continuation refuses, and
    /// the replacement --help= plus the rest of the token
    /// stands in the handed over stream.

    #[test]
    fn scan_help_short_with_a_value_refuses_at_the_root() {
        assert_run(&["-h=x"], &["--help=x"]);
    }

    #[test]
    fn scan_help_short_with_an_empty_value_refuses_behind_scan() {
        assert_run(&["scan", "-h="], &["scan", "--help="]);
    }

    /// ZH-63, family 2. A dash continuation refuses, and the
    /// replacement carries the token from its second point,
    /// so the dash itself stands inside the refused form.

    #[test]
    fn scan_help_short_with_a_dash_value_refuses_behind_plan() {
        assert_run(&["plan", "-h-x"], &["plan", "--help=-x"]);
    }

    #[test]
    fn scan_help_short_with_a_lone_dash_refuses_at_the_root() {
        assert_run(&["-h-"], &["--help=-"]);
    }

    /// ZH-63, family 2. The capital letter is not the short
    /// arm, and the token is gathered as an unknown option.

    #[test]
    fn scan_help_capital_reads_no_help() {
        assert_run(&["scan", "-H"], &["scan", "-H"]);
    }

    /// ZH-63, family 2. Help fires ahead of the values the
    /// walk has already collected, though they could never
    /// distribute.

    #[test]
    fn scan_help_fires_ahead_of_collected_values() {
        assert_fire_help(&["scan", "a", "b", "-h"], Some(Cmd::Scan));
    }

    /// ZH-63, family 2. Help fires ahead of the assignment
    /// though a fixtures value stands consumed.

    #[test]
    fn scan_help_fires_ahead_of_a_consumed_flag() {
        assert_fire_help(&["plan", "snap", "--fixtures=x", "-h"], Some(Cmd::Plan));
    }

    /// ZH-63, family 3. The classification read on the bare
    /// shapes: a bare word and the lone dash are positional.

    #[test]
    fn scan_read_lone_dash_is_a_value() {
        assert_assigned(&["scan", "-"], Cmd::Scan, &[Want::One("-")], None);
    }

    /// ZH-63, family 3. The negative number pattern: a run
    /// of digits, or a run, a point, and a run, the whole
    /// part allowed empty and the fraction required.

    #[test]
    fn scan_read_negative_integer_is_a_value() {
        assert_assigned(&["scan", "-2"], Cmd::Scan, &[Want::One("-2")], None);
    }

    #[test]
    fn scan_read_negative_fraction_is_a_value() {
        assert_assigned(&["scan", "-2.5"], Cmd::Scan, &[Want::One("-2.5")], None);
    }

    #[test]
    fn scan_read_negative_with_an_empty_whole_part_is_a_value() {
        assert_assigned(&["scan", "-.5"], Cmd::Scan, &[Want::One("-.5")], None);
    }

    #[test]
    fn scan_read_negative_with_an_empty_fraction_is_an_option() {
        assert_run(&["scan", "-2."], &["scan", "-2."]);
    }

    /// ZH-63, family 3. The pattern's end anchor admits one
    /// trailing newline, as a Python text pattern's does,
    /// and no more than one.

    #[test]
    fn scan_read_negative_with_one_trailing_newline_is_a_value() {
        assert_assigned(&["scan", "-2\n"], Cmd::Scan, &[Want::One("-2\n")], None);
    }

    #[test]
    fn scan_read_negative_with_two_trailing_newlines_is_an_option() {
        assert_run(&["scan", "-2\n\n"], &["scan", "-2\n\n"]);
    }

    /// ZH-63, family 3. The digit set of the pattern is the
    /// full Unicode decimal set.

    #[test]
    fn scan_read_negative_in_unicode_digits_is_a_value() {
        assert_assigned(&["scan", "-١٢"], Cmd::Scan, &[Want::One("-١٢")], None);
    }

    /// ZH-63, family 3. The long prefix decides ahead of the
    /// negative pattern, and a second point in the fraction
    /// breaks the pattern.

    #[test]
    fn scan_read_double_dash_digits_are_an_option() {
        assert_run(&["scan", "--2"], &["scan", "--2"]);
    }

    #[test]
    fn scan_read_negative_with_a_second_point_is_an_option() {
        assert_run(&["scan", "-2.5.5"], &["scan", "-2.5.5"]);
    }

    /// ZH-63, family 3. A dash token carrying a space reads
    /// as positional whatever its prefix, the short help
    /// family standing as the one exception and decided by
    /// the help read first.

    #[test]
    fn scan_read_dash_token_with_a_space_is_a_value() {
        assert_assigned(&["scan", "- x"], Cmd::Scan, &[Want::One("- x")], None);
    }

    #[test]
    fn scan_read_flag_token_with_a_space_is_a_value() {
        assert_assigned(
            &["plan", "--fixtures x"],
            Cmd::Plan,
            &[Want::One("--fixtures x")],
            None,
        );
    }

    /// ZH-63, family 3. A token with no first point at all
    /// reads as positional.

    #[test]
    fn scan_read_empty_token_is_a_value() {
        assert_assigned(&["scan", ""], Cmd::Scan, &[Want::One("")], None);
    }

    /// ZH-63, family 4. The fixtures flag in its standing
    /// and joined forms, exact and abbreviated, takes its
    /// value, and the plan assigns the estate form on the
    /// one share that stands.

    #[test]
    fn scan_flag_standing_takes_the_next_token() {
        assert_assigned(
            &["plan", "snap", "--fixtures", "dir"],
            Cmd::Plan,
            &[Want::One("snap")],
            Some("dir"),
        );
    }

    #[test]
    fn scan_flag_joined_carries_its_value() {
        assert_assigned(
            &["plan", "snap", "--fixtures=dir"],
            Cmd::Plan,
            &[Want::One("snap")],
            Some("dir"),
        );
    }

    #[test]
    fn scan_flag_standing_abbreviation_takes_the_next_token() {
        assert_assigned(
            &["plan", "snap", "--fi", "dir"],
            Cmd::Plan,
            &[Want::One("snap")],
            Some("dir"),
        );
    }

    #[test]
    fn scan_flag_joined_abbreviation_carries_its_value() {
        assert_assigned(
            &["plan", "snap", "--f=dir"],
            Cmd::Plan,
            &[Want::One("snap")],
            Some("dir"),
        );
    }

    /// ZH-63, family 4. The empty joined value is a value,
    /// and a joined value runs from the first equals sign.

    #[test]
    fn scan_flag_joined_empty_value_is_a_value() {
        assert_assigned(
            &["plan", "snap", "--fixtures="],
            Cmd::Plan,
            &[Want::One("snap")],
            Some(""),
        );
    }

    #[test]
    fn scan_flag_joined_value_runs_from_the_first_equals() {
        assert_assigned(
            &["plan", "snap", "--fixtures=a=b"],
            Cmd::Plan,
            &[Want::One("snap")],
            Some("a=b"),
        );
    }

    /// ZH-63, family 4. The pending arm takes the next token
    /// when the classification read calls it positional, a
    /// negative number among them.

    #[test]
    fn scan_flag_value_may_be_a_negative_number() {
        assert_assigned(
            &["plan", "snap", "--fixtures", "-2.5"],
            Cmd::Plan,
            &[Want::One("snap")],
            Some("-2.5"),
        );
    }

    /// ZH-63, family 4. The flag standing at the end of the
    /// stream leaves the walk pending, and the raw stream
    /// hands over as written.

    #[test]
    fn scan_flag_standing_at_the_end_hands_over() {
        assert_run(
            &["plan", "snap", "--fixtures"],
            &["plan", "snap", "--fixtures"],
        );
    }

    /// ZH-63, family 4. Any token the classification read
    /// does not call positional blocks the pending arm, an
    /// option, a help token of either spelling, the
    /// separator, and the flag itself among them, and help
    /// does not fire behind a pending flag.

    #[test]
    fn scan_flag_pending_blocks_on_an_option() {
        assert_run(
            &["plan", "snap", "--fixtures", "--bogus"],
            &["plan", "snap", "--fixtures", "--bogus"],
        );
    }

    #[test]
    fn scan_flag_pending_blocks_on_the_short_help_token() {
        assert_run(
            &["plan", "snap", "--fixtures", "-h"],
            &["plan", "snap", "--fixtures", "-h"],
        );
    }

    #[test]
    fn scan_flag_pending_blocks_on_the_long_help_token() {
        assert_run(
            &["plan", "snap", "--fixtures", "--help"],
            &["plan", "snap", "--fixtures", "--help"],
        );
    }

    #[test]
    fn scan_flag_pending_blocks_on_the_separator() {
        assert_run(
            &["plan", "snap", "--fixtures", "--"],
            &["plan", "snap", "--fixtures", "--"],
        );
    }

    #[test]
    fn scan_flag_pending_blocks_on_the_flag_itself() {
        assert_run(
            &["plan", "snap", "--fixtures", "--fixtures"],
            &["plan", "snap", "--fixtures", "--fixtures"],
        );
    }

    /// ZH-63, family 4. Where the flag repeats, the last
    /// value wins.

    #[test]
    fn scan_flag_last_value_wins() {
        assert_assigned(
            &["plan", "snap", "--fixtures", "a", "--fixtures", "b"],
            Cmd::Plan,
            &[Want::One("snap")],
            Some("b"),
        );
    }

    /// ZH-63, family 4. A repeated flag removes its first
    /// token from the gathered stream, even from mid stream,
    /// and the removal stands visible where the gathered
    /// stream hands over.

    #[test]
    fn scan_flag_repeat_removes_the_first_token_from_the_gathered_stream() {
        assert_run(
            &[
                "plan",
                "--fixtures",
                "a",
                "--bogus",
                "--fixtures",
                "b",
                "snap",
            ],
            &["plan", "--bogus", "--fixtures=b", "--", "snap"],
        );
    }

    /// ZH-63, family 4. A stem carrying a non ASCII point
    /// reads as no flag, and the token is gathered as an
    /// unknown option, the branch the census names with its
    /// ground (the red team's Round 12 Minor 3, adopted in
    /// the owner fix pass).

    #[test]
    fn scan_flag_non_ascii_stem_reads_no_flag() {
        assert_run(
            &["plan", "snap", "--fixturés=x"],
            &["plan", "--fixturés=x", "--", "snap"],
        );
    }

    /// ZH-63, family 4. A subcommand carrying no value flag
    /// reads the flag token as an unknown option.

    #[test]
    fn scan_flag_table_of_scan_is_empty() {
        assert_run(
            &["scan", "--fixtures", "dir"],
            &["scan", "--fixtures", "--", "dir"],
        );
    }

    /// ZH-63, family 4. A consumed flag ends its segment,
    /// and the shares on either side of it fill the slots in
    /// order, the single form of the plan.

    #[test]
    fn scan_flag_ends_the_segment_and_the_shares_fill_across() {
        assert_assigned(
            &["plan", "a", "--fixtures=x", "b"],
            Cmd::Plan,
            &[Want::One("a"), Want::One("b")],
            Some("x"),
        );
    }

    #[test]
    fn scan_flag_standing_ahead_of_both_shares() {
        assert_assigned(
            &["plan", "--fixtures", "dir", "snap", "entry"],
            Cmd::Plan,
            &[Want::One("snap"), Want::One("entry")],
            Some("dir"),
        );
    }

    /// ZH-63, family 5. Recognition at the root is byte
    /// equality against the subcommand names, and a word
    /// that does not name one blocks the walk there.

    #[test]
    fn scan_root_names_do_not_abbreviate() {
        assert_run(&["sca", "snap"], &["sca", "snap"]);
    }

    #[test]
    fn scan_root_name_in_another_case_does_not_select() {
        assert_run(&["Scan", "snap"], &["Scan", "snap"]);
    }

    #[test]
    fn scan_root_name_with_a_trailing_space_does_not_select() {
        assert_run(&["scan "], &["scan "]);
    }

    /// ZH-63, family 5. With no subcommand selected and
    /// nothing gathered, the walk hands over the empty
    /// stream.

    #[test]
    fn scan_root_empty_stream_hands_over_empty() {
        assert_run(&[], &[]);
    }

    /// ZH-63, family 5. A root option the walk cannot assign
    /// is gathered, and with no subcommand the gathered
    /// stream hands over.

    #[test]
    fn scan_root_option_is_gathered() {
        assert_run(&["--bogus"], &["--bogus"]);
    }

    #[test]
    fn scan_root_unknown_short_is_gathered() {
        assert_run(&["-x"], &["-x"]);
    }

    /// ZH-63, family 5. Help fires at the root though an
    /// unknown option stands gathered ahead of it.

    #[test]
    fn scan_root_help_fires_behind_a_gathered_option() {
        assert_fire_help(&["--bogus", "-h"], None);
    }

    /// ZH-63, family 5. The separator at the root blocks,
    /// no subcommand standing selected to receive it, and a
    /// blocked walk hands over the raw stream as written.

    #[test]
    fn scan_root_separator_blocks() {
        assert_run(&["--"], &["--"]);
    }

    #[test]
    fn scan_root_blocked_stream_is_the_raw_stream() {
        assert_run(&["--bogus", "--"], &["--bogus", "--"]);
    }

    #[test]
    fn scan_root_positional_blocks_the_whole_stream() {
        assert_run(&["junk", "scan", "snap"], &["junk", "scan", "snap"]);
    }

    /// ZH-63, family 5. A subcommand selected but
    /// undistributed hands over the gathered stream alone,
    /// no separator joining it, every segment standing
    /// empty.

    #[test]
    fn scan_root_subcommand_alone_hands_over_gathered() {
        assert_run(&["scan"], &["scan"]);
    }

    /// ZH-63, family 5. A subcommand word behind a
    /// subcommand is a plain value like any other word.

    #[test]
    fn scan_root_subcommand_word_behind_a_subcommand_is_a_value() {
        assert_assigned(&["scan", "plan"], Cmd::Scan, &[Want::One("plan")], None);
    }

    /// ZH-63, family 6. Within a segment the first separator
    /// is discounted into the share of the value just before
    /// it, or the first share when it leads, and each share
    /// gives up its first separator.

    #[test]
    fn scan_separator_leading_is_discounted() {
        assert_assigned(
            &["scan", "--", "snap"],
            Cmd::Scan,
            &[Want::One("snap")],
            None,
        );
    }

    #[test]
    fn scan_separator_trailing_joins_the_share() {
        assert_assigned(
            &["scan", "snap", "--"],
            Cmd::Scan,
            &[Want::One("snap")],
            None,
        );
    }

    #[test]
    fn scan_separator_joins_the_share_before_it() {
        assert_assigned(
            &["observe", "a", "--", "b", "c"],
            Cmd::Observe,
            &[Want::One("a"), Want::One("b"), Want::One("c")],
            None,
        );
    }

    /// ZH-63, family 6. Of two separators in one segment the
    /// first is discounted and given up, and the second
    /// stands as the share's one token.

    #[test]
    fn scan_separator_second_of_two_stands_as_the_value() {
        assert_assigned(&["scan", "--", "--"], Cmd::Scan, &[Want::One("--")], None);
    }

    #[test]
    fn scan_separator_leading_pair_stands_as_the_value() {
        assert_assigned(
            &["observe", "--", "--", "a", "b"],
            Cmd::Observe,
            &[Want::One("--"), Want::One("a"), Want::One("b")],
            None,
        );
    }

    /// ZH-63, family 6. A share that does not end holding
    /// exactly one token holds a list, here a lone later
    /// separator whose share gives it up and ends empty.

    #[test]
    fn scan_separator_lone_later_separator_holds_a_list() {
        assert_assigned(
            &["observe", "a", "--", "--", "b"],
            Cmd::Observe,
            &[Want::One("a"), Want::Listed, Want::One("b")],
            None,
        );
    }

    #[test]
    fn scan_separator_list_in_the_entry_share() {
        assert_assigned(
            &["plan", "--", "a", "--"],
            Cmd::Plan,
            &[Want::One("a"), Want::Listed],
            None,
        );
    }

    /// ZH-63, family 6. A trailing separator that closes a
    /// segment holding no positional, once a flag has been
    /// consumed, refuses the distribution in both of the
    /// plan's readings, and the handed over stream flattens
    /// the segments behind one separator.

    #[test]
    fn scan_separator_trailing_after_a_joined_flag_is_vetoed() {
        assert_run(
            &["plan", "snap", "--fixtures=x", "--"],
            &["plan", "--fixtures=x", "--", "snap", "--"],
        );
    }

    #[test]
    fn scan_separator_trailing_after_a_standing_flag_is_vetoed() {
        assert_run(
            &["plan", "snap", "--fixtures", "dir", "--"],
            &["plan", "--fixtures=dir", "--", "snap", "--"],
        );
    }

    /// ZH-63, family 6. The veto tests the last non empty
    /// segment holding exactly one separator, so a second
    /// separator standing in that segment escapes it and
    /// the distribution assigns, the second separator
    /// standing as the estate slot's token (the red team's
    /// Round 12 Minor 2, adopted in the owner fix pass).

    #[test]
    fn scan_separator_second_in_the_last_segment_escapes_the_veto() {
        assert_assigned(
            &["plan", "snap", "--fixtures=x", "--", "--"],
            Cmd::Plan,
            &[Want::One("snap"), Want::One("--")],
            Some("x"),
        );
    }

    /// ZH-63, family 6. The separator leading a segment that
    /// holds a value is discounted into that value's share,
    /// and the veto does not stand.

    #[test]
    fn scan_separator_leading_the_post_flag_segment_is_discounted() {
        assert_assigned(
            &["plan", "a", "--fixtures=x", "--", "b"],
            Cmd::Plan,
            &[Want::One("a"), Want::One("b")],
            Some("x"),
        );
    }

    #[test]
    fn scan_separator_in_a_value_segment_vetoes_nothing() {
        assert_assigned(
            &["plan", "--fixtures", "dir", "snap", "--"],
            Cmd::Plan,
            &[Want::One("snap")],
            Some("dir"),
        );
    }

    /// ZH-63, family 6. Where the distribution fails on the
    /// share count, the handed over stream appends one
    /// separator and the flattened segments to the gathered
    /// stream.

    #[test]
    fn scan_distribution_too_few_shares_hands_over() {
        assert_run(&["observe", "a", "b"], &["observe", "--", "a", "b"]);
    }

    #[test]
    fn scan_distribution_too_many_shares_hands_over() {
        assert_run(&["scan", "a", "b"], &["scan", "--", "a", "b"]);
    }

    /// ZH-63, family 6. In the separated state every later
    /// token is a value, a help token and a flag token among
    /// them, never a flag.

    #[test]
    fn scan_separator_help_after_it_is_a_value() {
        assert_run(
            &["scan", "snap", "--", "-h"],
            &["scan", "--", "snap", "--", "-h"],
        );
    }

    #[test]
    fn scan_separator_flag_after_it_is_a_value() {
        assert_run(
            &["scan", "snap", "--", "--fixtures"],
            &["scan", "--", "snap", "--", "--fixtures"],
        );
    }

    /// ZH-63, family 6. The blocked arm runs ahead of the
    /// separator token, so a walk blocked by a refused help
    /// token records no separator and hands the raw stream
    /// over.

    #[test]
    fn scan_separator_blocked_arm_runs_ahead_of_it() {
        assert_run(&["scan", "--help=x", "--"], &["scan", "--help=x", "--"]);
    }
}
