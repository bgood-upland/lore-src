use std::path::Path;
use std::process::Command;
use anyhow::{Context, Result};

use crate::config::{ProjectEntry, ProjectMode};

/// The result of a sync operation, used for reporting to the user
pub enum SyncOutcome {
    /// Fresh clone was created
    Cloned,
    /// Existing clone was updated
    Updated,
    /// Fetch failed but existing clone is still usable (stale data)
    StaleButUsable(String),
}

impl SyncOutcome {
    pub fn summary(&self) -> String {
        match self {
            SyncOutcome::Cloned => "Cloned repository".to_string(),
            SyncOutcome::Updated => "Updated to latest".to_string(),
            SyncOutcome::StaleButUsable(reason) => {
                format!("Using existing clone (fetch failed: {})", reason)
            }
        }
    }
}

/// Shallow-clone a repo into target_dir.
/// Equivalent to `git clone --depth 1 --branch {branch} {repo} {target_dir}`
fn clone_shallow(repo: &str, branch: &str, target_dir: &Path) -> Result<()> {
    let output = Command::new("git")
        .arg("clone")
        .arg("--depth").arg("1")
        .arg("--branch").arg(branch)
        .arg(repo)
        .arg(target_dir)
        .output()
        .context("Failed to execute git clone")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git clone failed: {}", stderr.trim());
    }

    Ok(())
}

/// Fetch and hard-reset an existing clone to the latest remote state.
/// Equivalent to:
///   `git -C {target_dir} fetch origin {branch} &&
///   git -C {target_dir} reset --hard origin/{branch}`
fn fetch_and_reset(branch: &str, target_dir: &Path) -> Result<()> {
    let fetch = Command::new("git")
        .arg("-C").arg(target_dir)
        .arg("fetch")
        .arg("origin")
        .arg(branch)
        .output()
        .context("Failed to execute git fetch")?;
    if !fetch.status.success() {
        let stderr = String::from_utf8_lossy(&fetch.stderr);
        anyhow::bail!("git fetch failed: {}", stderr.trim());
    }

    let reset = Command::new("git")
        .arg("-C").arg(target_dir)
        .arg("reset")
        .arg("--hard")
        .arg(format!("origin/{}", branch))
        .output()
        .context("Failed to execute git reset")?;
    if !reset.status.success() {
        let stderr = String::from_utf8_lossy(&reset.stderr);
        anyhow::bail!("git reset failed: {}", stderr.trim());
    }

    Ok(())
}

/// Ensure a project's clone is up-to-date.
///
/// - If target_dir doesn't exist: clone (errors are fatal — nothing to fall back to)
/// - If target_dir exists: fetch + reset (errors are non-fatal — stale data is usable)
pub fn sync_project(repo: &str, branch: &str, target_dir: &Path) -> Result<SyncOutcome> {
    if !target_dir.exists() {
        // First run must succeed or we have nothing
        clone_shallow(repo, branch, target_dir)?;
        Ok(SyncOutcome::Cloned)
    } else {
        // Subsequent runs network failure is survivable
        match fetch_and_reset(branch, target_dir) {
            Ok(()) => Ok(SyncOutcome::Updated),
            Err(e) => Ok(SyncOutcome::StaleButUsable(e.to_string())),
        }
    }
}

/// Sync all Autopilot projects.
/// Returns the number of projects that synced successfully.
pub fn sync_all_autopilot(projects: &[ProjectEntry], data_dir: &Path) -> usize {
    let mut synced = 0;
    for project in projects {
        if !matches!(project.mode, ProjectMode::Autopilot) {
            continue;
        }
        let repo = match project.repo.as_ref() {
            Some(r) => r,
            None => continue,
        };
        let branch = project.branch.as_deref().unwrap_or("main");
        let target = project.effective_root(data_dir);

        match sync_project(repo, branch, &target) {
            Ok(outcome) => {
                eprintln!("[sync] {}: {}", project.name, outcome.summary());
                synced += 1;
            }
            Err(e) => {
                eprintln!("[sync] {} failed: {}", project.name, e);
            }
        }
    }
    synced
}