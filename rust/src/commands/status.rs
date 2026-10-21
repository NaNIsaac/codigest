//! `status`: what `pack` would use here: the last failure, your changes and the baseline.

use std::path::PathBuf;

use clap::Args;

use crate::commands::pack;
use crate::common::ProjectContext;
use crate::shadow::ContextAnchor;
use crate::style::{bold, cyan, dim};
use crate::{pypath, term};

#[derive(Args, Debug)]
pub struct StatusArgs {
    /// Project directory
    #[arg(default_value = ".", value_name = "target")]
    pub target: PathBuf,
}

pub fn run(args: StatusArgs) -> anyhow::Result<i32> {
    if !args.target.is_dir() {
        eprintln!(
            "Error: Invalid value for '[TARGET]': Directory '{}' does not exist.",
            args.target.display()
        );
        return Ok(2);
    }
    let target = pypath::resolve(&args.target);
    let mut ctx = ProjectContext::new(std::slice::from_ref(&target))?;
    let root = ctx.root_path.clone();
    let files = ctx.get_target_files(&[], false, false)?;
    let entries = term::select(term::load_entries(), false, Some(&root));
    let say = |line: String| {
        let text = crate::output::text_mode(&format!("{line}\n"));
        let _ = std::io::Write::write_all(&mut std::io::stdout(), text.as_bytes());
    };

    say(format!(
        "{}  {}",
        bold(&pypath::name(&root)),
        dim(&root.display().to_string())
    ));

    if let Some(failure) = pack::pick_failure(entries.clone(), &root) {
        let when = pack::ago(&failure.started_at);
        let code = failure.exit_code.map_or("None".into(), |c| c.to_string());
        say(format!(
            "  failure   {} {}",
            cyan(&failure.command),
            dim(&format!("(exit {code}, {when})"))
        ));
    } else {
        let hint = if entries.is_empty() {
            "run your tests through cdg cap".to_string()
        } else {
            format!("{} captured commands here", entries.len())
        };
        say(format!(
            "  failure   {}",
            dim(&format!("none in the last hour ({hint})"))
        ));
    }

    let (_, source, changed) = pack::working_changes(&root, &files)?;
    if !changed.is_empty() {
        let plural = if changed.len() != 1 { "s" } else { "" };
        say(format!(
            "  changes   {} file{plural}, {source}",
            changed.len()
        ));
        for path in changed.iter().take(10) {
            say(format!(
                "            {}",
                dim(&pypath::rel_posix(path, &root).unwrap_or_default())
            ));
        }
        if changed.len() > 10 {
            say(format!(
                "            {}",
                dim(&format!("... and {} more", changed.len() - 10))
            ));
        }
    } else if !source.is_empty() {
        say(format!("  changes   {}", dim("none")));
    } else {
        say(format!(
            "  changes   {}",
            dim("not a Git repository; cdg mark sets a baseline to compare against")
        ));
    }

    let anchor = ContextAnchor::new(&root);
    if anchor.has_history() {
        say(format!(
            "  baseline  set {} {}",
            anchor.last_update_time(),
            dim("(cdg mark moves it)")
        ));
    } else {
        let tail = if source.is_empty() {
            ""
        } else {
            ", so pack compares against Git"
        };
        say(format!("  baseline  {}", dim(&format!("none{tail}"))));
    }

    if root.join(".codigest").join("config.toml").is_file() {
        let exts = match &ctx.config_extensions {
            Some(set) if !set.is_empty() => set.iter().cloned().collect::<Vec<_>>().join(", "),
            _ => "all".into(),
        };
        say(format!(
            "  filters   {}",
            dim(&format!(".codigest/config.toml: extensions {exts}"))
        ));
    }
    say(format!(
        "  files     {}",
        dim(&format!("{} considered", files.len()))
    ));
    Ok(0)
}
