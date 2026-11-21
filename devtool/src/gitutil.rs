//! Petites fonctions autour de `git`, utilisées par toutes les étapes.

use std::path::Path;
use std::process::Command;

use anyhow::Result;

use crate::proc::{capture_or_empty, run};

pub fn is_repo(dir: &Path) -> bool {
    dir.join(".git").exists()
}

/// Dossier existant et vide. `git clone` de Senbit crée les dossiers des sources
/// (kernel/linux, third_party/busybox...) SANS leur contenu : un dossier vide est
/// un emplacement à remplir, pas une erreur.
pub fn is_empty_dir(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false)
}

pub fn describe(dir: &Path) -> String {
    capture_or_empty(Command::new("git").args([
        "-C",
        &dir.to_string_lossy(),
        "describe",
        "--tags",
        "--always",
    ]))
}

pub fn describe_exact(dir: &Path) -> String {
    capture_or_empty(Command::new("git").args([
        "-C",
        &dir.to_string_lossy(),
        "describe",
        "--tags",
        "--exact-match",
    ]))
}

pub fn head(dir: &Path) -> String {
    capture_or_empty(Command::new("git").args(["-C", &dir.to_string_lossy(), "rev-parse", "HEAD"]))
}

pub fn status_porcelain(dir: &Path) -> String {
    capture_or_empty(Command::new("git").args([
        "-C",
        &dir.to_string_lossy(),
        "status",
        "--porcelain",
    ]))
}

pub fn fetch_tags(dir: &Path) -> Result<()> {
    run(Command::new("git").args([
        "-C",
        &dir.to_string_lossy(),
        "fetch",
        "--tags",
        "--prune",
        "origin",
    ]))
}

pub fn checkout_detach(dir: &Path, rev: &str) -> Result<()> {
    run(Command::new("git").args(["-C", &dir.to_string_lossy(), "checkout", "--detach", rev]))
}

pub fn clone_tag(repo: &str, tag: &str, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    run(Command::new("git")
        .args(["clone", "--branch", tag, "--depth", "1", repo])
        .arg(dest))
}
