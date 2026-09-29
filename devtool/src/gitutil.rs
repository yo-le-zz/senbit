//! Petites fonctions autour de `git`, utilisées par toutes les étapes.

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Result};

use crate::proc::{capture_or_empty, run};

pub fn is_repo(dir: &Path) -> bool {
    dir.join(".git").exists()
}

pub fn describe(dir: &Path) -> String {
    capture_or_empty(Command::new("git").args(["-C", &dir.to_string_lossy(), "describe", "--tags", "--always"]))
}

pub fn describe_exact(dir: &Path) -> String {
    capture_or_empty(Command::new("git").args(["-C", &dir.to_string_lossy(), "describe", "--tags", "--exact-match"]))
}

pub fn head(dir: &Path) -> String {
    capture_or_empty(Command::new("git").args(["-C", &dir.to_string_lossy(), "rev-parse", "HEAD"]))
}

pub fn status_porcelain(dir: &Path) -> String {
    capture_or_empty(Command::new("git").args(["-C", &dir.to_string_lossy(), "status", "--porcelain"]))
}

pub fn fetch_tags(dir: &Path) -> Result<()> {
    run(Command::new("git").args(["-C", &dir.to_string_lossy(), "fetch", "--tags", "--prune", "origin"]))
}

pub fn checkout_detach(dir: &Path, rev: &str) -> Result<()> {
    run(Command::new("git").args(["-C", &dir.to_string_lossy(), "checkout", "--detach", rev]))
}

pub fn clone_tag(repo: &str, tag: &str, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    run(Command::new("git").args(["clone", "--branch", tag, "--depth", "1", repo]).arg(dest))
}

/// Dernier tag distant qui passe `filter`, trié comme `sort -V`.
pub fn latest_tag(repo: &str, filter: impl Fn(&str) -> bool) -> Result<String> {
    let output = crate::proc::capture(Command::new("git").args(["ls-remote", "--tags", "--refs", repo]))?;
    let mut tags: Vec<&str> = output
        .lines()
        .filter_map(|l| l.split_whitespace().nth(1))
        .filter_map(|r| r.strip_prefix("refs/tags/"))
        .filter(|t| filter(t))
        .collect();
    if tags.is_empty() {
        bail!("no matching tag found on {repo}");
    }
    tags.sort_by(|a, b| version_key(a).cmp(&version_key(b)));
    Ok(tags.last().unwrap().to_string())
}

fn version_key(v: &str) -> Vec<u32> {
    v.split(|c: char| !c.is_ascii_digit())
        .filter(|p| !p.is_empty())
        .map(|p| p.parse().unwrap_or(0))
        .collect()
}

/// 1_NN_NN (tags de release de BusyBox).
pub fn is_busybox_tag(t: &str) -> bool {
    let parts: Vec<&str> = t.split('_').collect();
    parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}
