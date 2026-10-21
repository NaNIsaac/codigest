//! `tree`: print the project file tree.

use std::fs;
use std::path::{Path, PathBuf};

use clap::Args;

use crate::common::ProjectContext;
use crate::output::{Output, OutputArgs};
use crate::pystr::filesize;
use crate::style::{bold, cyan, dim, red, yellow};
use crate::{pypath, structure};

#[derive(Args, Debug)]
pub struct TreeArgs {
    /// Target directory
    #[arg(default_value = ".")]
    pub target: PathBuf,
    #[command(flatten)]
    pub out: OutputArgs,
    /// Ignore config filters
    #[arg(short, long)]
    pub all: bool,
    /// Recursively resolve imports
    #[arg(short, long)]
    pub resolve: bool,
    /// Show this many levels; deeper folders become a file count
    #[arg(short = 'L', long, value_parser = clap::value_parser!(u32).range(1..))]
    pub depth: Option<u32>,
}

pub fn run(args: TreeArgs) -> anyhow::Result<i32> {
    let out = Output::new(&args.out, false);
    let targets = [args.target];
    let mut ctx = ProjectContext::new(&targets)?;
    let root = ctx.root_path.clone();

    let files = match ctx.get_target_files(&targets, args.all, args.resolve) {
        Ok(files) => files,
        Err(err) => {
            out.log(&format!("{} {err}", red("[Error] Scan failed:")));
            return Ok(1);
        }
    };
    if files.is_empty() {
        out.log(&yellow("[Warning] No matching files found."));
        return Ok(0);
    }

    let depth = args.depth.map(|d| d as usize);
    let tree = structure::generate_ascii_tree(&files, &root, depth);
    out.deliver(&tree, "tree", |out| {
        if depth.is_some() {
            out.log(&tree);
        } else {
            for line in rich_tree(&root, &files) {
                out.log(&line);
            }
            out.log(&format!(
                "\n{}",
                dim(&format!("Found {} files.", files.len()))
            ));
        }
    })?;
    Ok(0)
}

#[derive(Default)]
struct Node {
    label: String,
    children: Vec<Node>,
}

/// The sized tree shown when the result is not printed.
fn rich_tree(root: &Path, files: &[PathBuf]) -> Vec<String> {
    let mut top = Node::default();
    for path in files {
        let size = fs::metadata(path).map_or(0, |m| m.len());
        let (parents, name) = match pypath::rel_posix(path, root) {
            Some(rel) => {
                let mut parts: Vec<String> = rel.split('/').map(str::to_owned).collect();
                let name = parts.pop().unwrap_or_default();
                (parts, name)
            }
            None => (Vec::new(), format!("[EXTERNAL] {}", pypath::name(path))),
        };
        let mut node = &mut top;
        for part in parents {
            node = child(node, &cyan(&part));
        }
        node.children.push(Node {
            label: format!("{name} {}", dim(&format!("({})", filesize(size)))),
            children: Vec::new(),
        });
    }
    let mut lines = vec![bold(&pypath::name(root))];
    emit(&top, "", &mut lines);
    lines
}

/// The folder node called `label`, created on first use.
fn child<'a>(node: &'a mut Node, label: &str) -> &'a mut Node {
    let at = match node.children.iter().position(|c| c.label == label) {
        Some(at) => at,
        None => {
            node.children.push(Node {
                label: label.to_owned(),
                children: Vec::new(),
            });
            node.children.len() - 1
        }
    };
    &mut node.children[at]
}

fn emit(node: &Node, prefix: &str, lines: &mut Vec<String>) {
    let last = node.children.len().saturating_sub(1);
    for (i, child) in node.children.iter().enumerate() {
        let (connector, extension) = if i == last {
            ("└── ", "    ")
        } else {
            ("├── ", "│   ")
        };
        lines.push(format!(
            "{}{}",
            dim(&format!("{prefix}{connector}")),
            child.label
        ));
        emit(child, &format!("{prefix}{extension}"), lines);
    }
}
