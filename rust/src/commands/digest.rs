//! `digest`: numbered signatures and docstrings of Python and MATLAB files, no bodies.

use std::fs;
use std::path::PathBuf;

use clap::Args;

use crate::cache::{ContentCache, compute_git_blob_hash};
use crate::common::ProjectContext;
use crate::output::{Output, OutputArgs};
use crate::par;
use crate::pystr::{thousands, universal_newlines};
use crate::style::{bold, yellow};
use crate::{outline, prompts, pypath, structure, tags};

#[derive(Args, Debug)]
pub struct DigestArgs {
    /// Files or directories to outline (default: the project)
    #[arg(value_name = "targets")]
    pub targets: Vec<PathBuf>,
    #[command(flatten)]
    pub out: OutputArgs,
    /// Add specific instruction context
    #[arg(short, long, default_value = "")]
    pub message: String,
    /// Recursively resolve imports
    #[arg(short, long)]
    pub resolve: bool,
}

pub fn run(args: DigestArgs) -> anyhow::Result<i32> {
    let out = Output::new(&args.out, true);
    if let Some(missing) = args.targets.iter().find(|t| !t.exists()) {
        eprintln!(
            "Error: Invalid value for '[TARGETS]...': Path '{}' does not exist.",
            missing.display()
        );
        return Ok(2);
    }
    let targets: Vec<PathBuf> = args.targets.iter().map(|t| pypath::resolve(t)).collect();

    let mut ctx = ProjectContext::new(&targets)?;
    let root = ctx.root_path.clone();
    let engine = prompts::get_engine(&root);

    let files = ctx.get_target_files(&targets, false, args.resolve)?;
    let tree = structure::generate_ascii_tree(&files, &root, None);

    let mut cache = ContentCache::new(&root);
    let mut blocks = Vec::new();
    let mut unreadable = Vec::new();
    // Cache lookups in order, the outlines of the misses in parallel, then everything back
    // in file order.
    let mut wanted: Vec<(String, Option<String>, PathBuf, String)> = Vec::new();
    for file in &files {
        let suffix = pypath::suffix(&pypath::name(file)).to_owned();
        let Some(extension) = suffix.strip_prefix('.') else {
            continue;
        };
        if !outline::SUFFIXES.contains(&extension) {
            continue;
        }
        let rel = pypath::rel_posix(file, &root)
            .unwrap_or_else(|| format!("[EXTERNAL]/{}", pypath::name(file)));
        // The original looks the summary up under a missing hash, so only the stat check
        // can hit.
        let hit = cache.get_summary(&rel, "None", Some(file));
        wanted.push((rel, hit, file.clone(), extension.to_owned()));
    }
    let persists = cache.persists();
    let computed = par::map(&wanted, |(_, hit, file, extension)| {
        if hit.is_some() {
            return None;
        }
        let bytes = fs::read(file).ok()?;
        let content = universal_newlines(&String::from_utf8_lossy(&bytes));
        let text = outline::summarize(extension, &content);
        let hash = (persists && !text.is_empty()).then(|| compute_git_blob_hash(&bytes));
        Some((text, hash))
    });
    for ((rel, hit, file, _), fresh) in wanted.into_iter().zip(computed) {
        let summary = match (hit, fresh) {
            (Some(text), _) => text,
            (None, Some((text, hash))) => {
                if let Some(hash) = hash {
                    cache.set_summary(&rel, &hash, &text, Some(&file));
                }
                text
            }
            (None, None) => {
                unreadable.push(rel);
                continue;
            }
        };
        if !summary.is_empty() {
            blocks.push(tags::file(&rel, &summary, "", ""));
        }
    }
    cache.save();

    let project = pypath::name(&root);
    let digest_content = engine.render(
        "digest",
        &[
            ("project_name", &project),
            ("tree_structure", &tree),
            ("digest_content", &blocks.join("\n")),
            ("instruction", &args.message),
        ],
    )?;

    if !unreadable.is_empty() {
        out.log(&format!(
            "{} {}",
            yellow("Could not read:"),
            unreadable.join(", ")
        ));
    }
    out.log(&format!(
        "{} {} files, ~{} tokens",
        bold("Digest"),
        blocks.len(),
        thousands(digest_content.chars().count().div_ceil(4) as u64)
    ));
    out.deliver(&digest_content, "digest", |_| {})?;
    Ok(0)
}
