//! What a new worktree needs that `git worktree add` will not bring: the
//! gitignored things the main checkout holds. `.env`, `.claude/settings.local.json`,
//! per-skill symlinks into a config repo — without them an agent that moves
//! into the worktree finds the project stripped of its local setup.
//!
//! Ignored *directories* are skipped on purpose. `node_modules`, `target` and
//! `.venv` are large, regenerable and often branch-specific, and
//! `git ls-files --directory` reports each as a single `dir/` entry — so one
//! check on the trailing slash excludes all of them, where a list of names to
//! exclude would never be complete. A directory that genuinely has to be
//! shared is shared the way the user already shares one: as a symlink, which
//! is an entry in its own right and is carried over.

use std::path::{Path, PathBuf};

use crate::git;
use pacer_core::paths;

/// Fill a freshly created `worktree` from `main`.
///
/// Best-effort throughout: an unreadable file or a symlink the platform
/// refuses is logged and skipped, never propagated. A worktree missing one of
/// these is worth having; a worktree that failed to be created is not.
pub async fn seed(main: &Path, worktree: &Path) {
    let entries = match git::ignored_entries(main).await {
        Ok(entries) => entries,
        Err(e) => {
            tracing::warn!(error = %e, "listing ignored files failed; worktree starts bare");
            return;
        }
    };
    for entry in entries {
        match entry.strip_suffix('/') {
            Some(dir) => links_inside(&main.join(dir), &worktree.join(dir), main, worktree),
            None => {
                let dst = worktree.join(&entry);
                // The branch tracks a path the main checkout ignores: what git
                // just checked out is the branch's own version, and it wins.
                if dst.symlink_metadata().is_ok() {
                    continue;
                }
                if let Err(e) = place(&main.join(&entry), &dst, main, worktree) {
                    tracing::warn!(error = %e, entry = %entry, "seeding worktree entry failed");
                }
            }
        }
    }
}

/// Recreate the symlinks sitting directly inside a fully ignored directory.
///
/// The directory itself stays behind, but it must still be looked into: git
/// collapses a directory it ignores in full to a single `dir/` entry, which
/// hides `.claude/skills/<skill>` — exactly the links this whole module
/// exists to carry. One shallow pass finds them without walking the hundred
/// thousand build artefacts under a `node_modules/` collapsed the same way.
fn links_inside(src_dir: &Path, dst_dir: &Path, main: &Path, worktree: &Path) {
    let Ok(children) = std::fs::read_dir(src_dir) else {
        return;
    };
    for child in children.flatten() {
        if !child.file_type().is_ok_and(|t| t.is_symlink()) {
            continue;
        }
        let dst = dst_dir.join(child.file_name());
        if dst.symlink_metadata().is_ok() {
            continue;
        }
        if let Err(e) = place(&child.path(), &dst, main, worktree) {
            tracing::warn!(error = %e, entry = ?child.path(), "seeding worktree link failed");
        }
    }
}

fn place(src: &Path, dst: &Path, main: &Path, worktree: &Path) -> std::io::Result<()> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if src.symlink_metadata()?.is_symlink() {
        // Recreated as a link, never copied: the whole point of the link is
        // that both checkouts read one file, and a copy starts diverging the
        // moment either side is edited.
        link(src, dst, &link_target(src, main, worktree)?)
    } else {
        std::fs::copy(src, dst).map(|_| ())
    }
}

/// Where the worktree's copy of the link at `src` should point.
///
/// A target inside the main checkout is re-pointed at the worktree's own copy:
/// the branch may hold a different version of that file, and following the
/// link back into main would edit the wrong checkout. Every other target is
/// kept but made absolute, because the worktree sits at a different depth than
/// main and a relative target would resolve somewhere else entirely.
fn link_target(src: &Path, main: &Path, worktree: &Path) -> std::io::Result<PathBuf> {
    let raw = std::fs::read_link(src)?;
    let absolute = if raw.is_absolute() {
        raw
    } else {
        src.parent().unwrap_or(main).join(raw)
    };
    let absolute = paths::canonical_or_raw(&absolute);
    match absolute.strip_prefix(paths::canonical_or_raw(main)) {
        Ok(inside) => Ok(worktree.join(inside)),
        Err(_) => Ok(absolute),
    }
}

#[cfg(unix)]
fn link(_src: &Path, dst: &Path, target: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, dst)
}

#[cfg(windows)]
fn link(src: &Path, dst: &Path, target: &Path) -> std::io::Result<()> {
    // Windows picks the symlink flavour at creation time, so which kind the
    // target is has to be settled first — `src.metadata()` follows the link
    // and answers for the target. A refusal here is almost always the
    // platform's: creating a symlink needs Developer Mode or elevation.
    if src.metadata()?.is_dir() {
        std::os::windows::fs::symlink_dir(target, dst)
    } else {
        std::os::windows::fs::symlink_file(target, dst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn git(repo: &Path, args: &[&str]) {
        let out = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {:?}", out.stderr);
    }

    fn symlink_dir(target: &Path, link: &Path) -> std::io::Result<()> {
        #[cfg(unix)]
        return std::os::unix::fs::symlink(target, link);
        #[cfg(windows)]
        return std::os::windows::fs::symlink_dir(target, link);
    }

    /// A directory ignored in full collapses to one `dir/` entry, so skipping
    /// every such entry — the obvious reading of "don't copy node_modules" —
    /// silently drops the per-skill links that are the reason this module
    /// exists. They live inside exactly such a directory.
    #[tokio::test]
    async fn a_link_inside_a_fully_ignored_directory_reaches_the_worktree() {
        let tmp = tempfile::tempdir().unwrap();
        let shared = tmp.path().join("shared-skill");
        std::fs::create_dir_all(&shared).unwrap();
        std::fs::write(shared.join("SKILL.md"), "shared").unwrap();

        let main = tmp.path().join("main");
        std::fs::create_dir_all(main.join(".claude/skills")).unwrap();
        if symlink_dir(&shared, &main.join(".claude/skills/linked")).is_err() {
            return; // No symlink privilege here; the rest proves nothing.
        }
        std::fs::write(main.join(".gitignore"), ".claude/skills/\n.env\n").unwrap();
        std::fs::write(main.join(".env"), "SECRET=1").unwrap();
        git(&main, &["init", "-b", "main"]);
        git(&main, &["add", "-A"]);
        git(
            &main,
            &[
                "-c",
                "user.email=t@t",
                "-c",
                "user.name=t",
                "commit",
                "-m",
                "init",
            ],
        );

        let worktree = tmp.path().join("wt");
        std::fs::create_dir_all(&worktree).unwrap();
        seed(&main, &worktree).await;

        let linked = worktree.join(".claude/skills/linked");
        assert!(
            linked.symlink_metadata().unwrap().is_symlink(),
            "the skill link did not come across"
        );
        assert_eq!(
            std::fs::read_to_string(linked.join("SKILL.md")).unwrap(),
            "shared"
        );
        assert_eq!(
            std::fs::read_to_string(worktree.join(".env")).unwrap(),
            "SECRET=1"
        );
    }
}
