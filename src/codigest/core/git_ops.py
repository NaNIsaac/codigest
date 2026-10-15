"""
Git Operations.
Retains the 'Untracked File' detection logic from the prototype.
"""
lazy import subprocess
from pathlib import Path
from loguru import logger

def is_git_repo(root_path: Path) -> bool:
    return (root_path / ".git").exists()

def get_smart_diff(root_path: Path) -> str:
    """
    Fetches git diff AND content of untracked (new) files.
    """
    if not is_git_repo(root_path):
        return "Warning: Not a git repository."

    try:
        # 1. Standard Git Diff (Staged + Unstaged)
        diff_output = subprocess.check_output(
            ["git", "diff", "HEAD"], 
            cwd=root_path, 
            text=True, 
            encoding='utf-8',
            stderr=subprocess.DEVNULL 
        )
        
        # 2. Untracked Files (The 'Killer Feature' from prototype)
        untracked_files = subprocess.check_output(
            ["git", "ls-files", "--others", "--exclude-standard"],
            cwd=root_path,
            text=True,
            encoding='utf-8',
            stderr=subprocess.DEVNULL
        ).splitlines()
        
        full_output = diff_output
        new_files_content = []

        for f in untracked_files:
            file_path = root_path / f
            if not file_path.is_file(): 
                continue

            try:
                # Basic binary check (skip if cannot decode)
                content = file_path.read_text(encoding='utf-8')
                line_count = len(content.splitlines())
                
                # Manually construct a Diff Header
                header = (
                    f"diff --git a/{f} b/{f}\n"
                    f"new file mode 100644\n"
                    f"--- /dev/null\n"
                    f"+++ b/{f}\n"
                    f"@@ -0,0 +1,{line_count} @@\n"
                    f"{content}\n"
                )
                new_files_content.append(header)
                
            except UnicodeDecodeError:
                logger.debug(f"Skipping binary untracked file: {f}")
                continue

        if new_files_content:
            full_output += "\n# Untracked (New) Source Files:\n" + "".join(new_files_content)

        return full_output.strip()

    except subprocess.CalledProcessError as e:
        logger.error(f"Git command failed: {e}")
        return "Error: Failed to run git diff."