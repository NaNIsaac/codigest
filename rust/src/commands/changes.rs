//! `mark`, `diff` and `semdiff`: a baseline kept apart from Git, and what changed since.

use std::path::PathBuf;

use clap::Args;

use crate::common::ProjectContext;
use crate::outline::python::{Change, compare};
use crate::output::{Output, OutputArgs};
use crate::redact::redact_diff;
use crate::shadow::ContextAnchor;
use crate::style::{bold, cyan, dim, green, yellow};
use crate::tokenizer::estimate_tokens;
use crate::{prompts, pypath, tags};

fn directory(target: &std::path::Path, what: &str) -> Option<PathBuf> {
    if target.is_dir() {
        Some(pypath::resolve(target))
    } else {
        eprintln!(
            "Error: Invalid value for '[{what}]': Directory '{}' does not exist.",
            target.display()
        );
        None
    }
}

#[derive(Args, Debug)]
pub struct MarkArgs {
    /// Project directory
    #[arg(default_value = ".", value_name = "target")]
    pub target: PathBuf,
}

pub fn mark(args: MarkArgs) -> anyhow::Result<i32> {
    let Some(target) = directory(&args.target, "TARGET") else {
        return Ok(2);
    };
    let mut ctx = ProjectContext::new(std::slice::from_ref(&target))?;
    let files = ctx.get_target_files(&[], false, false)?;
    ContextAnchor::new(&ctx.root_path).update(&files)?;
    eprintln!(
        "{} {}",
        green("Baseline set."),
        dim("`cdg diff` now shows changes from here.")
    );
    Ok(0)
}

#[derive(Args, Debug)]
pub struct DiffArgs {
    /// Target directory to check diff
    #[arg(default_value = ".", value_name = "target")]
    pub target: PathBuf,
    #[command(flatten)]
    pub out: OutputArgs,
    /// Add specific instruction context
    #[arg(short, long, default_value = "")]
    pub message: String,
    /// Recursively resolve imports
    #[arg(short, long)]
    pub resolve: bool,
}

pub fn diff(args: DiffArgs) -> anyhow::Result<i32> {
    let Some(target) = directory(&args.target, "TARGET") else {
        return Ok(2);
    };
    let out = Output::new(&args.out, true);
    let mut ctx = ProjectContext::new(std::slice::from_ref(&target))?;
    let root = ctx.root_path.clone();
    let anchor = ContextAnchor::new(&root);

    let last_update = anchor.last_update_time();
    if last_update == "Never" {
        out.log(&yellow("No baseline found."));
        out.log(&format!(
            "   Run {} first to set a baseline.",
            bold(&cyan("cdg mark"))
        ));
        return Ok(1);
    }
    out.log(&dim(&format!(
        "Checking changes since last mark ({last_update})..."
    )));

    let files = ctx.get_target_files(&[], false, args.resolve)?;
    let diff = redact_diff(&anchor.get_changes(&files)?);
    if diff.trim().is_empty() {
        out.log(&green("No changes since the last mark."));
        return Ok(0);
    }

    let project = pypath::name(&root);
    let context = format!("since cdg mark ({last_update})");
    let formatted = prompts::get_engine(&root)
        .render(
            "diff",
            &[
                ("project_name", &project),
                ("context_message", &context),
                ("diff_content", &diff),
                ("instruction", &args.message),
            ],
        )
        .unwrap_or_else(|_| format!("<diff>\n{}\n</diff>", tags::escape_value(&diff)));

    out.log(&format!(
        "{} ({} chars)",
        bold(&green("Changes Detected!")),
        formatted.chars().count()
    ));
    out.deliver(&formatted, "diff", |_| {})?;
    Ok(0)
}

#[derive(Args, Debug)]
pub struct SemdiffArgs {
    /// Target directory
    #[arg(default_value = ".", value_name = "target")]
    pub target: PathBuf,
    #[command(flatten)]
    pub out: OutputArgs,
    /// Add specific instruction context
    #[arg(short, long, default_value = "")]
    pub message: String,
    /// Recursively resolve imports
    #[arg(short, long)]
    pub resolve: bool,
}

pub fn semdiff(args: SemdiffArgs) -> anyhow::Result<i32> {
    let out = Output::new(&args.out, true);
    let mut ctx = ProjectContext::new(std::slice::from_ref(&pypath::resolve(&args.target)))?;
    let root = ctx.root_path.clone();
    let anchor = ContextAnchor::new(&root);

    let last_update = anchor.last_update_time();
    if last_update == "Never" {
        out.log(&yellow(&format!(
            "No baseline found. Run {} first.",
            bold("cdg mark")
        )));
        return Ok(1);
    }
    out.log(&dim(&format!(
        "Analyzing structural changes since ({last_update})..."
    )));

    let files = ctx.get_target_files(&[], false, args.resolve)?;
    let changed = anchor.get_changed_files(&files)?;
    let mut reports = Vec::new();
    for path in changed
        .iter()
        .filter(|p| matches!(pypath::suffix(&pypath::name(p)), ".py" | ".pyi"))
    {
        let rel = pypath::rel_posix(path, &root);
        let new_code = std::fs::read_to_string(path).unwrap_or_default();
        // The anchor only holds files inside the project.
        let old_code = rel
            .as_deref()
            .map_or_else(String::new, |r| anchor.read_anchor_file(r.as_ref()));
        let changes = compare(&old_code, &new_code);
        if changes.is_empty() {
            continue;
        }
        let lines: Vec<String> = changes
            .iter()
            .map(|c| {
                let icon = match c.change {
                    Change::Added => "[ADDED]    ",
                    Change::Removed => "[REMOVED]  ",
                    Change::Modified => "[SIGNATURE]",
                    Change::LogicChanged => "[LOGIC]    ",
                };
                // The signature is the news only when it changed, and then both sides are in details.
                let detail = if c.change == Change::Modified {
                    format!(" :: {}", c.details)
                } else {
                    String::new()
                };
                format!(
                    "{icon} {}: {} {}{detail}",
                    c.symbol.line,
                    c.symbol.kind.label(),
                    c.symbol.name
                )
            })
            .collect();
        let status = if new_code.is_empty() {
            " (DELETED)"
        } else if old_code.is_empty() {
            " (NEW)"
        } else {
            ""
        };
        let name = rel.unwrap_or_else(|| format!("[EXTERNAL]/{}", pypath::name(path)));
        reports.push(tags::file(&name, &lines.join("\n"), status, ""));
    }

    if reports.is_empty() {
        out.log(&green("No structural (AST) changes detected."));
        return Ok(0);
    }
    let project = pypath::name(&root);
    let context = format!("since cdg mark ({last_update})");
    let content = reports.join("\n");
    let rendered = prompts::get_engine(&root).render(
        "semdiff",
        &[
            ("project_name", &project),
            ("context_message", &context),
            ("semdiff_content", &content),
            ("instruction", &args.message),
        ],
    );
    let rendered = match rendered {
        Ok(text) => text,
        Err(err) => {
            out.log(&format!("Template Error: {err}"));
            return Ok(1);
        }
    };
    out.log(&format!(
        "{} ({})",
        bold(&green("SemDiff Generated!")),
        bold(&cyan(&format!(
            "~{} Tokens",
            crate::pystr::thousands(estimate_tokens(&rendered) as u64)
        )))
    ));
    out.deliver(&rendered, "semdiff", |_| {})?;
    Ok(0)
}
