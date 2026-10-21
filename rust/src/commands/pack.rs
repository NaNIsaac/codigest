//! `pack`: one block with what a model needs for the task at hand: the last failed command
//! run in this project, the files its output points at, your uncommitted changes, and the
//! local modules those files import. Files are added in that order until the token budget
//! runs out; whatever is left out is listed, so the reader knows what it is not seeing.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use clap::Args;
use regex::Regex;

use crate::common::ProjectContext;
use crate::output::{Output, OutputArgs};
use crate::pystr::thousands;
use crate::redact::redact_diff;
use crate::resolver::Resolver;
use crate::shadow::ContextAnchor;
use crate::style::{bold, cyan, dim, green, red, yellow};
use crate::tags::{escape_attr, escape_value};
use crate::term::{self, Entry};
use crate::tokenizer::estimate_tokens;
use crate::{callees, git_ops, processor, pypath};

pub const DEFAULT_BUDGET: usize = 30_000;
// A file that is only named in the failing output gets this many lines around each reference.
const EXCERPT_CONTEXT: usize = 10;
const EXCERPT_MIN_LINES: usize = 60;
const FAILURE_MAX_AGE_SECS: i64 = 3600;
// Lines of a long failing output kept from the start (first failures) and the end (summary).
pub const OUTPUT_HEAD: usize = 80;
pub const OUTPUT_TAIL: usize = 40;

pub const SOURCES: [&str; 4] = ["failure", "changes", "tests", "imports"];

// `File "x.py", line 7` (Python) or `x.py:7` / `x.ts(7,3)` (pytest, tsc, gcc, rustc, eslint, ...)
static LOCATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"File "(?P<py>[^"]+)", line (?P<py_line>\d+)|(?P<path>[\w./\\~-]+\.\w+)[:(](?P<line>\d+)"#)
        .expect("valid pattern")
});
static TEST_FILE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^test_.*\.py$|^.*_test\.py$|\.(test|spec)\.[cm]?[jt]sx?$").expect("valid pattern")
});
static HUNK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^@@ -\d+(?:,\d+)? \+(?P<start>\d+)(?:,(?P<count>\d+))? @@").expect("valid pattern")
});

#[derive(Args, Debug)]
pub struct PackArgs {
    /// Files or directories to include first
    #[arg(value_name = "targets")]
    pub targets: Vec<PathBuf>,
    /// Add specific instruction context
    #[arg(short, long, default_value = "")]
    pub message: String,
    /// Token budget (estimated)
    #[arg(short, long, default_value_t = DEFAULT_BUDGET as u64, value_parser = clap::value_parser!(u64).range(1000..))]
    pub budget: u64,
    /// Automatic sources to add: failure,changes,tests,imports (all, or none when you name files)
    #[arg(long = "with", value_name = "SOURCES")]
    pub with_: Option<String>,
    #[command(flatten)]
    pub out: OutputArgs,
}

pub fn parse_sources(text: &str) -> Result<HashSet<String>, String> {
    let chosen: HashSet<String> = text
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    let mut unknown: Vec<&String> = chosen
        .iter()
        .filter(|s| !SOURCES.contains(&s.as_str()) && s.as_str() != "none")
        .collect();
    if !unknown.is_empty() {
        unknown.sort();
        let names: Vec<&str> = unknown.iter().map(|s| s.as_str()).collect();
        return Err(format!(
            "unknown source {}; choose from {}",
            names.join(", "),
            SOURCES.join(", ")
        ));
    }
    Ok(chosen.into_iter().filter(|s| s != "none").collect())
}

fn started(entry: &Entry) -> Option<i64> {
    parse_iso(&entry.started_at)
}

/// Seconds since the epoch of an ISO 8601 timestamp with an offset, as Python writes them.
fn parse_iso(text: &str) -> Option<i64> {
    let (date, rest) = text.split_once('T')?;
    let mut d = date.split('-').map(|p| p.parse::<i64>());
    let (y, m, day) = (d.next()?.ok()?, d.next()?.ok()?, d.next()?.ok()?);
    let (time, offset) = match rest.find(['+', '-', 'Z']) {
        Some(i) => rest.split_at(i),
        None => (rest, ""),
    };
    let mut t = time.split(':');
    let h: i64 = t.next()?.parse().ok()?;
    let mi: i64 = t.next()?.parse().ok()?;
    let s: f64 = t.next().unwrap_or("0").parse().ok()?;
    // Python always writes the offset; a bare time is read as UTC.
    let offset_secs = match offset {
        "" => 0,
        "Z" => 0,
        o => {
            let sign = if o.starts_with('-') { -1 } else { 1 };
            let mut parts = o[1..].split(':');
            let oh: i64 = parts.next()?.parse().ok()?;
            let om: i64 = parts.next().unwrap_or("0").parse().ok()?;
            sign * (oh * 3600 + om * 60)
        }
    };
    // Days from civil (Howard Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3600 + mi * 60 + s as i64 - offset_secs)
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// Latest failed command run under `root` within the last hour, unless the same command
/// has passed since. Older failures are rarely what someone running `pack` is looking at.
pub fn pick_failure(entries: Vec<Entry>, root: &Path) -> Option<Entry> {
    let cutoff = now_secs() - FAILURE_MAX_AGE_SECS;
    let mut passed: HashSet<String> = HashSet::new();
    for entry in term::select(entries, false, Some(root)).into_iter().rev() {
        if started(&entry).is_none_or(|t| t < cutoff) {
            return None;
        }
        match entry.exit_code {
            Some(0) => {
                passed.insert(entry.command.clone());
            }
            Some(_) if !passed.contains(&entry.command) => return Some(entry),
            _ => {}
        }
    }
    None
}

/// Project files named in command output, with the line numbers given for each, in order.
pub fn referenced_files(
    output: &str,
    bases: &[PathBuf],
    known: &HashSet<String>,
) -> Vec<(PathBuf, Vec<usize>)> {
    let mut found: Vec<(PathBuf, Vec<usize>)> = Vec::new();
    for m in LOCATION.captures_iter(output) {
        let raw = m
            .name("py")
            .or_else(|| m.name("path"))
            .map_or("", |g| g.as_str());
        let line: usize = m
            .name("py_line")
            .or_else(|| m.name("line"))
            .map_or(0, |g| g.as_str().parse().unwrap_or(0));
        let raw = if let Some(rest) = raw.strip_prefix('~') {
            crate::paths::home().join(rest.trim_start_matches(['/', '\\']))
        } else {
            PathBuf::from(raw)
        };
        for base in bases {
            let path = pypath::resolve(&base.join(&raw));
            if known.contains(&pypath::norm(&path)) {
                match found.iter_mut().find(|(p, _)| pypath::same(p, &path)) {
                    Some((_, lines)) => {
                        if !lines.contains(&line) {
                            lines.push(line);
                        }
                    }
                    None => found.push((path, vec![line])),
                }
                break;
            }
        }
    }
    found
}

/// The lines around each reference and every span in `whole` in full, numbered, with `...`
/// between gaps, plus the ranges shown ("3-23,40-60"). None when the file is short or the
/// excerpt would be most of it anyway.
pub fn excerpt(path: &Path, lines: &[usize], whole: &[(usize, usize)]) -> Option<(String, String)> {
    let text = std::fs::read_to_string(path).ok()?;
    let rows = crate::pystr::splitlines(&text);
    let n = rows.len();
    let mut wanted: Vec<(usize, usize)> = lines
        .iter()
        .map(|&l| {
            (
                l.saturating_sub(EXCERPT_CONTEXT).max(1),
                (l + EXCERPT_CONTEXT).min(n),
            )
        })
        .collect();
    wanted.extend(whole.iter().map(|&(lo, hi)| (lo.max(1), hi.min(n))));
    wanted.sort();
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for (lo, hi) in wanted {
        match spans.last_mut() {
            Some(last) if lo <= last.1 + 1 => last.1 = last.1.max(hi),
            _ => spans.push((lo, hi)),
        }
    }
    let covered: isize = spans
        .iter()
        .map(|&(lo, hi)| hi as isize - lo as isize + 1)
        .sum();
    if n <= EXCERPT_MIN_LINES || covered as f64 > n as f64 * 0.7 {
        return None;
    }
    let width = n.to_string().len();
    let parts: Vec<String> = spans
        .iter()
        .map(|&(lo, hi)| {
            (lo..=hi)
                .map(|i| format!("{i:>width$}: {}", rows[i - 1]))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect();
    let shown: Vec<String> = spans.iter().map(|(lo, hi)| format!("{lo}-{hi}")).collect();
    Some((parts.join("\n...\n"), shown.join(",")))
}

pub fn changed_paths(diff: &str, root: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    for line in diff.lines() {
        if line.starts_with("diff --git") {
            let last = line.split_whitespace().last().unwrap_or("");
            let p = pypath::resolve(&root.join(last.strip_prefix("b/").unwrap_or(last)));
            if p.is_file() && !paths.iter().any(|q| pypath::same(q, &p)) {
                paths.push(p);
            }
        }
    }
    paths
}

/// Per changed file, the line ranges its hunks cover in the new version.
pub fn hunk_spans(diff: &str, root: &Path) -> HashMap<String, Vec<(usize, usize)>> {
    let mut spans: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
    let mut current: Option<String> = None;
    for line in diff.lines() {
        if let Some(name) = line.strip_prefix("+++ ") {
            let name = name.trim();
            let name = name.strip_prefix("b/").unwrap_or(name);
            current =
                (name != "/dev/null").then(|| pypath::norm(&pypath::resolve(&root.join(name))));
        } else if let (Some(file), Some(m)) = (&current, HUNK.captures(line)) {
            let start: usize = m["start"].parse().unwrap_or(1);
            let count: usize = m
                .name("count")
                .map_or(1, |c| c.as_str().parse().unwrap_or(1));
            spans
                .entry(file.clone())
                .or_default()
                .push((start, start + count.max(1) - 1));
        }
    }
    spans
}

/// Long output keeps its head (the first failures) and its tail (the summary).
pub fn shorten_output(text: &str) -> String {
    let lines = crate::pystr::splitlines(text);
    if lines.len() <= OUTPUT_HEAD + OUTPUT_TAIL + 20 {
        return text.to_string();
    }
    let cut = lines.len() - OUTPUT_HEAD - OUTPUT_TAIL;
    let note = format!("[... {cut} lines left out; `cdg term -f` shows the whole output ...]");
    let mut out: Vec<&str> = lines[..OUTPUT_HEAD].to_vec();
    out.push(&note);
    out.extend(&lines[lines.len() - OUTPUT_TAIL..]);
    out.join("\n")
}

pub fn ago(iso: &str) -> String {
    let Some(t) = parse_iso(iso) else {
        return String::new();
    };
    let seconds = now_secs() - t;
    for (unit, size) in [("d", 86_400), ("h", 3600), ("m", 60)] {
        if seconds >= size {
            return format!("{}{unit} ago", seconds / size);
        }
    }
    "just now".into()
}

/// (diff, where it came from, changed files). A `cdg mark` baseline wins over Git.
pub fn working_changes(
    root: &Path,
    files: &[PathBuf],
) -> anyhow::Result<(String, String, Vec<PathBuf>)> {
    let anchor = ContextAnchor::new(root);
    if anchor.has_history() {
        let diff = anchor.get_changes(files)?;
        let changed = changed_paths(&diff, root);
        return Ok((
            redact_diff(&diff),
            format!("since cdg mark ({})", anchor.last_update_time()),
            changed,
        ));
    }
    if let Some((diff, untracked)) = git_ops::working_changes(root) {
        let mut changed = changed_paths(&diff, root);
        changed.extend(untracked);
        return Ok((
            redact_diff(&diff),
            "uncommitted changes (git)".into(),
            changed,
        ));
    }
    Ok((String::new(), String::new(), Vec::new()))
}

fn rel(path: &Path, root: &Path) -> String {
    pypath::rel_posix(path, root).unwrap_or_else(|| path.display().to_string())
}

pub fn run(args: PackArgs) -> anyhow::Result<i32> {
    let auto: HashSet<String> = match &args.with_ {
        Some(text) => match parse_sources(text) {
            Ok(set) => set,
            Err(why) => {
                eprintln!("Error: Invalid value for '--with': {why}");
                return Ok(2);
            }
        },
        None if args.targets.is_empty() => SOURCES.iter().map(|s| s.to_string()).collect(),
        None => HashSet::new(),
    };
    for target in &args.targets {
        if !target.exists() {
            eprintln!(
                "Error: Invalid value for '[TARGETS]...': Path '{}' does not exist.",
                target.display()
            );
            return Ok(2);
        }
    }
    let targets: Vec<PathBuf> = args.targets.iter().map(|t| pypath::resolve(t)).collect();
    let out = Output::new(&args.out, true);

    let mut ctx = ProjectContext::new(&targets)?;
    let root = ctx.root_path.clone();
    let known_list = ctx.get_target_files(&[], false, false)?;
    let known: HashSet<String> = known_list.iter().map(|p| pypath::norm(p)).collect();

    // (path, why it is here), most important first
    let mut wanted: Vec<(PathBuf, String)> = Vec::new();
    let want = |wanted: &mut Vec<(PathBuf, String)>, path: PathBuf, why: String| {
        if !wanted.iter().any(|(p, _)| pypath::same(p, &path)) {
            wanted.push((path, why));
        }
    };
    if !targets.is_empty() {
        for path in ctx.get_target_files(&targets, false, false)? {
            want(&mut wanted, path, "requested".into());
        }
    }

    let failure = if auto.contains("failure") {
        pick_failure(term::load_entries(), &root)
    } else {
        None
    };
    let mut refs: Vec<(PathBuf, Vec<usize>)> = Vec::new();
    if let Some(failure) = &failure {
        let bases = if failure.cwd.is_empty() {
            vec![root.clone()]
        } else {
            vec![PathBuf::from(&failure.cwd), root.clone()]
        };
        refs = referenced_files(&failure.output, &bases, &known);
        for (path, lines) in &refs {
            let mut shown: Vec<String> = lines.iter().take(3).map(usize::to_string).collect();
            if lines.len() > 3 {
                shown.push("...".into());
            }
            let plural = if lines.len() > 1 { "s" } else { "" };
            want(
                &mut wanted,
                path.clone(),
                format!("line{plural} {} in failing output", shown.join(", ")),
            );
        }
    }
    let refs_of = |path: &Path| {
        refs.iter()
            .find(|(p, _)| pypath::same(p, path))
            .map(|(_, l)| l.clone())
    };

    let mut resolver = Resolver::new(&root);
    let mut imports = |path: &Path| -> Vec<PathBuf> {
        let mut deps: Vec<PathBuf> = resolver
            .imports_of(path)
            .into_iter()
            .filter(|d| known.contains(&pypath::norm(d)))
            .collect();
        deps.sort_by(|a, b| crate::paths::cmp_paths(a, b));
        deps
    };

    // The code the failing tests call: a traceback that ends in an assertion names only the test.
    let mut calls: Vec<(PathBuf, Vec<(usize, usize)>)> = Vec::new();
    for (test, lines) in refs.clone() {
        if pypath::suffix(&pypath::name(&test)) != ".py" {
            continue;
        }
        // Two levels, so `from pkg import f` reaches the module behind pkg/__init__.py.
        let mut modules = imports(&test);
        for module in modules.clone() {
            for m in imports(&module) {
                if !modules.iter().any(|x| pypath::same(x, &m)) {
                    modules.push(m);
                }
            }
        }
        modules.retain(|m| !pypath::same(m, &test));
        let test_rel = rel(&test, &root);
        for (path, _name, span) in callees::called_definitions(&test, &lines, &modules) {
            match calls.iter_mut().find(|(p, _)| pypath::same(p, &path)) {
                Some((_, spans)) => spans.push(span),
                None => calls.push((path.clone(), vec![span])),
            }
            want(
                &mut wanted,
                path,
                format!("called from failing test {test_rel}"),
            );
        }
    }

    let (diff, diff_source, changed_all) = if auto.contains("changes") {
        working_changes(&root, &pypath::sorted_unique(known_list.clone()))?
    } else {
        (String::new(), String::new(), Vec::new())
    };
    let changed: Vec<PathBuf> = changed_all
        .iter()
        .map(|p| pypath::resolve(p))
        .filter(|p| known.contains(&pypath::norm(p)))
        .collect();
    for path in &changed {
        want(&mut wanted, path.clone(), "changed".into());
    }
    let hunks = if diff.trim().is_empty() {
        HashMap::new()
    } else {
        hunk_spans(&diff, &root)
    };

    let edited: Vec<PathBuf> = wanted
        .iter()
        .filter(|(_, why)| why == "requested" || why == "changed")
        .map(|(p, _)| p.clone())
        .collect();
    if !edited.is_empty() && auto.contains("tests") {
        let mut tests: Vec<PathBuf> = known_list
            .iter()
            .filter(|p| TEST_FILE.is_match(&pypath::name(p)))
            .cloned()
            .collect();
        tests.sort_by(|a, b| crate::paths::cmp_paths(a, b));
        for test in tests {
            let hit = imports(&test)
                .into_iter()
                .find(|d| edited.iter().any(|e| pypath::same(e, d)));
            if let Some(hit) = hit {
                want(&mut wanted, test, format!("tests {}", rel(&hit, &root)));
            }
        }
    }

    if auto.contains("imports") {
        let looked_up: Vec<PathBuf> = wanted
            .iter()
            // Like Python's endswith(("failing output", "failing test")): a callee's reason ends
            // with the test's path, so callees do get their imports.
            .filter(|(_, why)| !why.ends_with("failing output") && !why.ends_with("failing test"))
            .map(|(p, _)| p.clone())
            .collect();
        for path in looked_up {
            for dep in imports(&path) {
                want(
                    &mut wanted,
                    dep,
                    format!("imported by {}", rel(&path, &root)),
                );
            }
        }
    }

    if failure.is_none() && diff.trim().is_empty() && wanted.is_empty() {
        out.log(&format!(
            "{} no failing command here and no changes.",
            yellow("Nothing to pack:")
        ));
        out.log(&format!(
            "  Capture one with {}, or name files: {}",
            cyan("cdg cap -- pytest"),
            cyan("cdg pack src/app.py")
        ));
        return Ok(1);
    }

    let mut head: Vec<String> = Vec::new();
    if !args.message.is_empty() {
        head.push(format!(
            "<user_instruction>\n{}\n</user_instruction>",
            escape_value(&args.message)
        ));
    }
    if let Some(failure) = &failure {
        let short = Entry {
            output: shorten_output(&failure.output),
            ..failure.clone()
        };
        head.push(term::render(&[short], false));
    }
    if !diff.trim().is_empty() {
        head.push(format!(
            "<changes source=\"{}\">\n{}\n</changes>",
            escape_attr(&diff_source),
            escape_value(crate::pystr::trim_end(&diff))
        ));
    }
    let mut used = estimate_tokens(&head.join("\n"));

    let mut blocks: Vec<String> = Vec::new();
    let mut included: Vec<(String, String)> = Vec::new();
    let mut omitted: Vec<(String, String)> = Vec::new();
    // Credential files never get here (file selection withholds them); name the ones that
    // were asked for or changed, so it is clear they were left out on purpose.
    let named_or_changed: Vec<PathBuf> = targets
        .iter()
        .cloned()
        .chain(changed_all.iter().map(|p| pypath::resolve(p)))
        .collect();
    for path in &ctx.withheld {
        let resolved = pypath::resolve(path);
        let named = named_or_changed.iter().any(|p| pypath::same(p, &resolved));
        let inside = targets
            .iter()
            .any(|t| t.is_dir() && pypath::is_relative_to(path, t));
        if named || inside {
            omitted.push((rel(path, &root), "may contain secrets".into()));
        }
    }
    for (path, reason) in &wanted {
        let rel_path = rel(path, &root);
        // Files you asked for come whole. The rest come as excerpts where that saves much:
        // around the places the output points at, the definitions the failing tests call,
        // and the hunks of the diff above (which already shows the change itself).
        let mut part = None;
        if reason != "requested" {
            let mut whole: Vec<(usize, usize)> = calls
                .iter()
                .find(|(p, _)| pypath::same(p, path))
                .map(|(_, s)| s.clone())
                .unwrap_or_default();
            if changed.iter().any(|c| pypath::same(c, path)) {
                whole.extend(hunks.get(&pypath::norm(path)).cloned().unwrap_or_default());
            }
            let lines = refs_of(path);
            if lines.is_some() || !whole.is_empty() {
                part = excerpt(path, &lines.unwrap_or_default(), &whole);
            }
        }
        let (content, shown) =
            part.unwrap_or_else(|| (processor::read_file_content(path, true), String::new()));
        let lines_attr = if shown.is_empty() {
            String::new()
        } else {
            format!(" lines=\"{shown}\"")
        };
        let block = format!(
            "<file path=\"{}\" reason=\"{}\"{lines_attr}>\n{}\n</file>",
            escape_attr(&rel_path),
            escape_attr(reason),
            escape_value(&content)
        );
        let cost = estimate_tokens(&block);
        if used + cost > args.budget as usize {
            omitted.push((
                rel_path,
                format!("over budget (~{} tokens)", thousands(cost as u64)),
            ));
            continue;
        }
        used += cost;
        blocks.push(block);
        let brief = if reason.starts_with("lines ") {
            format!(
                "{} places in failing output",
                refs_of(path).map_or(0, |l| l.len())
            )
        } else {
            reason.clone()
        };
        included.push((
            rel_path,
            if shown.is_empty() {
                brief
            } else {
                format!("{brief} (excerpt)")
            },
        ));
    }

    if !omitted.is_empty() {
        let listing: Vec<String> = omitted
            .iter()
            .map(|(r, why)| format!("{r}: {why}"))
            .collect();
        blocks.push(format!(
            "<omitted>\n{}\n</omitted>",
            escape_value(&listing.join("\n"))
        ));
    }

    let mut all = head;
    all.extend(blocks);
    let pack = format!(
        "<context_pack project=\"{}\">\n{}\n</context_pack>",
        escape_attr(&pypath::name(&root)),
        all.join("\n")
    );

    let over = if used > args.budget as usize {
        format!(" {}", red("(fixed parts alone exceed the budget)"))
    } else {
        String::new()
    };
    out.log(&format!(
        "{}{over}",
        bold(&format!(
            "Packed ~{} / {} tokens",
            thousands(used as u64),
            thousands(args.budget)
        ))
    ));
    if auto.len() != SOURCES.len() {
        let on: Vec<&str> = SOURCES
            .iter()
            .copied()
            .filter(|s| auto.contains(*s))
            .collect();
        let on = if on.is_empty() {
            "none".to_string()
        } else {
            on.join(", ")
        };
        out.log(&format!(
            "  {}",
            dim(&format!(
                "auto     {on} (--with {} adds more)",
                SOURCES.join(",")
            ))
        ));
    }
    if let Some(failure) = &failure {
        let when = ago(&failure.started_at);
        let code = failure
            .exit_code
            .map_or("None".to_string(), |c| c.to_string());
        let when = if when.is_empty() {
            String::new()
        } else {
            format!(", {when}")
        };
        out.log(&format!(
            "  {}  {} {}",
            red("failed "),
            cyan(&failure.command),
            dim(&format!("(exit {code}{when})"))
        ));
    }
    if !diff.trim().is_empty() {
        out.log(&format!("  {}  {}", yellow("changes"), dim(&diff_source)));
    }
    let width = included
        .iter()
        .chain(&omitted)
        .map(|(r, _)| r.chars().count())
        .max()
        .unwrap_or(0);
    for (r, why) in &included {
        out.log(&format!("  {} {r:<width$}  {}", green("+"), dim(why)));
    }
    for (r, why) in &omitted {
        out.log(&format!("  {}", dim(&format!("- {r:<width$}  {why}"))));
    }

    out.deliver(&pack, "pack", |_| {})?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hunks_read_the_new_side() {
        let root = Path::new("/r");
        let diff = "diff --git a/x.py b/x.py\n--- a/x.py\n+++ b/x.py\n@@ -10,3 +12,4 @@\n a\n+b\n@@ -40 +43 @@\n-e\n+f\n";
        let spans = hunk_spans(diff, root);
        assert_eq!(spans.values().next().unwrap(), &vec![(12, 15), (43, 43)]);
    }

    #[test]
    fn long_output_keeps_head_and_tail() {
        let text: Vec<String> = (0..500).map(|i| format!("line {i}")).collect();
        let short = shorten_output(&text.join("\n"));
        let lines: Vec<&str> = short.lines().collect();
        assert_eq!(lines[0], "line 0");
        assert_eq!(*lines.last().unwrap(), "line 499");
        assert!(lines[OUTPUT_HEAD].contains("380 lines left out"));
    }

    #[test]
    fn sources_are_checked() {
        assert!(parse_sources("tests,none").unwrap().contains("tests"));
        assert!(parse_sources("bogus").is_err());
    }

    #[test]
    fn iso_timestamps_parse_with_offsets() {
        assert_eq!(parse_iso("1970-01-01T09:00:00+09:00"), Some(0));
        assert_eq!(
            parse_iso("2026-10-17T01:58:14+09:00").map(|t| t > 0),
            Some(true)
        );
    }
}
