//! Isolate a run in a linked git worktree. Nested tasks inherit cwd.
//!
//! Layout matches the house: sibling `{repo}.wt/rung-task--{id}` on branch
//! `rung-task/{id}`, forked from the current HEAD. Not `git-wt-new` — that
//! command is for feature worktrees, not task isolation.

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: String,
    pub created: bool,
}

/// Create or reuse the task worktree. `cwd` is the git workdir the user is in
/// (may itself be a worktree); the sibling `.wt/` hangs off the primary clone.
pub fn ensure(id: &str, cwd: &Path) -> Result<Worktree, String> {
    crate::session::check_id(id)?;
    let primary = primary_clone(cwd)?;
    let head = git_stdout(cwd, &["rev-parse", "HEAD"])?;
    let repo_name = primary
        .file_name()
        .ok_or_else(|| "repo path has no name".to_string())?
        .to_string_lossy();
    let parent = primary
        .parent()
        .ok_or_else(|| "repo has no parent".to_string())?;
    let wt_root = parent.join(format!("{repo_name}.wt"));
    std::fs::create_dir_all(&wt_root).map_err(|e| format!("create {}: {e}", wt_root.display()))?;
    let path = wt_root.join(format!("rung-task--{id}"));
    let branch = format!("rung-task/{id}");
    if path.is_dir() {
        return Ok(Worktree {
            path,
            branch,
            created: false,
        });
    }
    let mut args = vec!["worktree".into(), "add".into()];
    if branch_exists(cwd, &branch) {
        args.push(path.to_string_lossy().into_owned());
        args.push(branch.clone());
    } else {
        args.push("-b".into());
        args.push(branch.clone());
        args.push(path.to_string_lossy().into_owned());
        args.push(head);
    }
    git_ok(
        &primary,
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
    )?;
    Ok(Worktree {
        path,
        branch,
        created: true,
    })
}

pub fn primary_clone(cwd: &Path) -> Result<PathBuf, String> {
    let common = git_stdout(cwd, &["rev-parse", "--git-common-dir"])?;
    let common = PathBuf::from(common);
    let common = if common.is_absolute() {
        common
    } else {
        cwd.join(common)
    };
    let common = common
        .canonicalize()
        .map_err(|e| format!("git common dir: {e}"))?;
    common
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "git common dir has no parent".into())
}

fn branch_exists(cwd: &Path, branch: &str) -> bool {
    git_ok(
        cwd,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    )
    .is_ok()
}

fn git_ok(cwd: &Path, args: &[&str]) -> Result<(), String> {
    let out = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

fn git_stdout(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn git_repo() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "rung-iso-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&p).unwrap();
        assert!(
            Command::new("git")
                .current_dir(&p)
                .args(["init", "-b", "master"])
                .output()
                .unwrap()
                .status
                .success()
        );
        assert!(
            Command::new("git")
                .current_dir(&p)
                .args(["config", "user.email", "t@t.t"])
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .current_dir(&p)
                .args(["config", "user.name", "t"])
                .status()
                .unwrap()
                .success()
        );
        std::fs::write(p.join("README"), "x").unwrap();
        assert!(
            Command::new("git")
                .current_dir(&p)
                .args(["add", "README"])
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .current_dir(&p)
                .args(["commit", "-m", "init"])
                .output()
                .unwrap()
                .status
                .success()
        );
        p
    }

    #[test]
    fn worktree_sits_in_sibling_wt() {
        let repo = git_repo();
        let wt = ensure("abc1", &repo).unwrap();
        let name = repo.file_name().unwrap();
        let expected = repo
            .parent()
            .unwrap()
            .join(format!("{}.wt", name.to_string_lossy()))
            .join("rung-task--abc1");
        assert_eq!(wt.path, expected);
        assert!(wt.path.join("README").is_file());
        assert_eq!(wt.branch, "rung-task/abc1");
        let again = ensure("abc1", &repo).unwrap();
        assert!(!again.created);
        let _ = Command::new("git")
            .current_dir(&repo)
            .args(["worktree", "remove", "--force", &wt.path.to_string_lossy()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        let _ = Command::new("git")
            .current_dir(&repo)
            .args(["branch", "-D", "rung-task/abc1"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(repo.parent().unwrap().join(format!(
            "{}.wt",
            repo.file_name().unwrap().to_string_lossy()
        )));
    }
}
