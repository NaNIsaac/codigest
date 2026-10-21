//! `scan`: bundle the whole project, or only the given paths, as one block.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use clap::Args;
use regex::Regex;

use crate::cache::{ContentCache, get_file_blob_hash};
use crate::common::{ProjectContext, confirm, stdin_is_terminal};
use crate::format::{self, Format};
use crate::output::{Output, OutputArgs};
use crate::par;
use crate::pystr::{filesize, thousands};
use crate::style::{bold, red, yellow};
use crate::{processor, prompts, pypath, structure, tags};

#[derive(Args, Debug)]
pub struct ScanArgs {
    /// Files or directories to share; FILE:40-60 or FILE:42 shares only those lines
    #[arg(value_name = "targets")]
    pub targets: Vec<String>,
    #[command(flatten)]
    pub out: OutputArgs,
    /// Ignore config filters
    #[arg(short, long)]
    pub all: bool,
    /// Add specific instruction context
    #[arg(short, long, default_value = "")]
    pub message: String,
    /// Add line numbers to code blocks
    #[arg(short = 'l', long = "lines")]
    pub line_numbers: bool,
    /// Skip confirmation prompt
    #[arg(short, long)]
    pub yes: bool,
    /// Recursively resolve imports
    #[arg(short, long)]
    pub resolve: bool,
    /// Output format
    #[arg(long, value_enum, default_value_t = Format::Xml)]
    pub format: Format,
}

const TOKEN_THRESHOLD: usize = 30_000;
const FILE_COUNT_THRESHOLD: usize = 100;

type Span = (usize, Option<usize>);

/// `src/app.py:40-60` or `src/app.py:42`; the last colon, so `C:\x.py:3` still works.
static RANGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?P<path>.+):(?P<lo>\d+)(?:-(?P<hi>\d+))?$").expect("valid pattern")
});

#[derive(Debug)]
struct Targets {
    paths: Vec<PathBuf>,
    /// Line ranges per file, keyed by the folded path.
    ranges: HashMap<String, Vec<Span>>,
}

fn parse_targets(raw: &[String]) -> Result<Targets, String> {
    let mut targets = Targets {
        paths: Vec::new(),
        ranges: HashMap::new(),
    };
    for item in raw {
        let mut path = PathBuf::from(item);
        let mut span = None;
        // Range syntax wins: on NTFS `a.py:12` names an alternate data stream of a.py,
        // which the path checks below would not reject.
        if let Some(m) = RANGE.captures(item)
            && Path::new(&m["path"]).is_file()
        {
            path = PathBuf::from(&m["path"]);
            let number = |s: &str| s.parse().unwrap_or(usize::MAX);
            span = Some((number(&m["lo"]), m.name("hi").map(|h| number(h.as_str()))));
        }
        if !path.exists() {
            let hint = if item.chars().skip(2).any(|c| c == ':') {
                " (a line range is FILE:START-END or FILE:LINE)"
            } else {
                ""
            };
            return Err(format!("{item} does not exist{hint}"));
        }
        let path = pypath::resolve(&path);
        if let Some(span) = span {
            targets
                .ranges
                .entry(pypath::norm(&path))
                .or_default()
                .push(span);
        }
        if !targets.paths.iter().any(|p| pypath::same(p, &path)) {
            targets.paths.push(path);
        }
    }
    Ok(targets)
}

pub fn run(args: ScanArgs) -> anyhow::Result<i32> {
    let out = Output::new(&args.out, true);
    let targets = match parse_targets(&args.targets) {
        Ok(targets) => targets,
        Err(why) => {
            eprintln!("Error: Invalid value for '[targets]...': {why}");
            return Ok(2);
        }
    };
    let mut ctx = ProjectContext::new(&targets.paths)?;
    let root = ctx.root_path.clone();
    let engine = prompts::get_engine(&root);

    let files = ctx.get_target_files(&targets.paths, args.all, args.resolve)?;

    // A range of a big file is small: count what will be shared, not the whole file.
    let mut total_size: u64 = 0;
    for file in &files {
        total_size += match targets.ranges.get(&pypath::norm(file)) {
            Some(spans) => {
                let mut chars = 0;
                for &(lo, hi) in spans {
                    chars += processor::read_line_range(file, lo, hi)?.0.chars().count();
                }
                chars as u64
            }
            None => std::fs::metadata(file)?.len(),
        };
    }
    let total_files = files.len();
    let est_tokens = (total_size / 4) as usize;

    if !ctx.withheld.is_empty() {
        let names: Vec<String> = ctx
            .withheld
            .iter()
            .filter_map(|p| pypath::rel_posix(p, &root))
            .collect();
        out.log(&format!(
            "{} {}",
            yellow("Withheld (may contain secrets):"),
            names.join(", ")
        ));
    }

    let is_large = est_tokens > TOKEN_THRESHOLD || total_files > FILE_COUNT_THRESHOLD;
    if is_large && !args.yes {
        out.log(&format!(
            "{} {total_files} files, {}, about {} tokens (asks above {} tokens or {FILE_COUNT_THRESHOLD} files; -y skips this)",
            yellow("Large:"),
            filesize(total_size),
            thousands(est_tokens as u64),
            thousands(TOKEN_THRESHOLD as u64),
        ));
        let hint = "Share less by naming paths (cdg scan src/), or cdg digest for signatures only.";
        if !stdin_is_terminal() {
            // Nobody can answer the question: say how to go on instead of a bare "Aborted!".
            out.log(&format!("Stopped. {hint} -y takes it all."));
            return Ok(1);
        }
        match confirm("Continue anyway?") {
            Some(true) => {}
            Some(false) => {
                out.log(&format!("Nothing shared. {hint}"));
                return Ok(0);
            }
            None => {
                eprintln!("Aborted!");
                return Ok(1);
            }
        }
    }

    let tree = structure::generate_ascii_tree(&files, &root, None);
    let line_numbers = args.line_numbers;
    if args.format != Format::Xml {
        // The block cache holds XML blocks, so other formats read the files afresh.
        let read = par::map(&files, |file| -> anyhow::Result<Vec<Shared>> {
            let path = pypath::rel_posix(file, &root)
                .unwrap_or_else(|| format!("[EXTERNAL]/{}", pypath::name(file)));
            match targets.ranges.get(&pypath::norm(file)) {
                Some(spans) => spans
                    .iter()
                    .map(|&(lo, hi)| {
                        let (content, lines) = processor::read_line_range(file, lo, hi)?;
                        Ok(Shared {
                            path: path.clone(),
                            lines,
                            content,
                        })
                    })
                    .collect(),
                None => Ok(vec![Shared {
                    content: processor::read_file_content(file, line_numbers),
                    path,
                    lines: String::new(),
                }]),
            }
        });
        let mut shared = Vec::new();
        for result in read {
            shared.extend(result?);
        }
        let project = pypath::name(&root);
        let snapshot = match args.format {
            Format::Markdown => markdown(&project, &args.message, &tree, &shared),
            _ => serde_json::to_string_pretty(&serde_json::json!({
                "project": project,
                "instruction": args.message,
                "structure": tree,
                "files": shared,
            }))?,
        };
        out.deliver(&snapshot, "snapshot", |out| {
            let tokens = snapshot.chars().count().div_ceil(4);
            out.log(&format!(
                "{} {total_files} files, ~{} tokens",
                bold("Snapshot"),
                thousands(tokens as u64)
            ));
        })?;
        return Ok(0);
    }
    let mut cache = ContentCache::new(&root);
    let persists = cache.persists();
    // Cache lookups in order, the reading of the misses in parallel, then the blocks back
    // in file order.
    let rels: Vec<(PathBuf, String, Option<String>)> = files
        .iter()
        .map(|file| {
            let rel = pypath::rel_posix(file, &root)
                .unwrap_or_else(|| format!("[EXTERNAL]/{}", pypath::name(file)));
            let ranged = targets.ranges.contains_key(&pypath::norm(file));
            let hit = (!ranged)
                .then(|| cache.get_file_block(&rel, None, line_numbers, Some(file)))
                .flatten();
            (file.clone(), rel, hit)
        })
        .collect();
    let read = par::map(
        &rels,
        |(file, rel, hit)| -> anyhow::Result<Vec<(String, Option<String>)>> {
            if let Some(spans) = targets.ranges.get(&pypath::norm(file)) {
                return spans
                    .iter()
                    .map(|&(lo, hi)| {
                        let (content, shown) = processor::read_line_range(file, lo, hi)?;
                        Ok((tags::file(rel, &content, "", &shown), None))
                    })
                    .collect();
            }
            if let Some(block) = hit {
                return Ok(vec![(block.clone(), None)]);
            }
            let blob_hash = if persists {
                get_file_blob_hash(file)
            } else {
                None
            };
            let content = processor::read_file_content(file, line_numbers);
            Ok(vec![(tags::file(rel, &content, "", ""), blob_hash)])
        },
    );
    let mut blocks = Vec::new();
    for ((file, rel, hit), result) in rels.into_iter().zip(read) {
        let ranged = targets.ranges.contains_key(&pypath::norm(&file));
        for (block, blob_hash) in result? {
            if !ranged && hit.is_none() {
                cache.set_file_block(
                    &rel,
                    blob_hash.as_deref(),
                    line_numbers,
                    &block,
                    Some(&file),
                );
            }
            blocks.push(block);
        }
    }
    cache.save();

    let project = pypath::name(&root);
    let source_code = blocks.join("\n\n");
    let snapshot = match engine.render(
        "snapshot",
        &[
            ("project_name", &project),
            ("tree_structure", &tree),
            ("source_code", &source_code),
            ("instruction", &args.message),
        ],
    ) {
        Ok(text) => text,
        Err(err) => {
            out.log(&format!(
                "{} {err}",
                red("[Error] Template Rendering Failed:")
            ));
            return Ok(1);
        }
    };

    out.deliver(&snapshot, "snapshot", |out| {
        let tokens = snapshot.chars().count().div_ceil(4);
        out.log(&format!(
            "{} {total_files} files, ~{} tokens",
            bold("Snapshot"),
            thousands(tokens as u64)
        ));
    })?;
    Ok(0)
}

/// One file, or one line range of it, as shared in Markdown or JSON.
#[derive(serde::Serialize)]
struct Shared {
    path: String,
    /// The range shown, `40-60`; empty for the whole file.
    #[serde(skip_serializing_if = "String::is_empty")]
    lines: String,
    content: String,
}

fn markdown(project: &str, instruction: &str, tree: &str, files: &[Shared]) -> String {
    let mut parts = vec![format!("# {project}")];
    if !instruction.is_empty() {
        parts.push(instruction.to_string());
    }
    parts.push(format!("## Structure\n\n{}", format::fenced(tree, "")));
    for file in files {
        let heading = if file.lines.is_empty() {
            format!("## {}", file.path)
        } else {
            format!("## {} (lines {})", file.path, file.lines)
        };
        parts.push(format!(
            "{heading}\n\n{}",
            format::fenced(&file.content, &file.path)
        ));
    }
    parts.join("\n\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_has_a_heading_and_a_fenced_block_per_file() {
        let files = [
            Shared {
                path: "a.py".into(),
                lines: String::new(),
                content: "x = 1\n".into(),
            },
            Shared {
                path: "b.md".into(),
                lines: "2-3".into(),
                content: "```\nq\n```".into(),
            },
        ];
        assert_eq!(
            markdown("proj", "fix it", "└── a.py", &files),
            "# proj\n\nfix it\n\n## Structure\n\n```\n└── a.py\n```\n\n\
             ## a.py\n\n```python\nx = 1\n```\n\n\
             ## b.md (lines 2-3)\n\n````markdown\n```\nq\n```\n````\n"
        );
    }

    #[test]
    fn a_range_needs_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.py");
        std::fs::write(&file, "x\ny\n").unwrap();
        let item = format!("{}:1-2", file.display());
        let parsed = parse_targets(&[item.clone(), format!("{}:2", file.display())]).unwrap();
        assert_eq!(parsed.paths.len(), 1);
        let spans = parsed.ranges.values().next().unwrap();
        assert_eq!(spans, &[(1, Some(2)), (2, None)]);

        let missing = dir.path().join("b.py");
        let err = parse_targets(&[format!("{}:3", missing.display())]).unwrap_err();
        assert!(err.contains("does not exist (a line range is FILE:START-END or FILE:LINE)"));
    }
}
