//! Exécution de commandes externes.

use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

fn describe(cmd: &Command) -> String {
    let mut s = cmd.get_program().to_string_lossy().to_string();
    for a in cmd.get_args() {
        s.push(' ');
        s.push_str(&a.to_string_lossy());
    }
    s
}

/// stdin/stdout/stderr branchés au terminal (équivalent d'un appel direct en bash).
pub fn run(cmd: &mut Command) -> Result<()> {
    let status = cmd.status().with_context(|| format!("failed to run `{}`", describe(cmd)))?;
    if !status.success() {
        bail!("`{}` exited with {status}", describe(cmd));
    }
    Ok(())
}

/// Sortie standard (trim), échoue si la commande échoue.
pub fn capture(cmd: &mut Command) -> Result<String> {
    let out = cmd
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .output()
        .with_context(|| format!("failed to run `{}`", describe(cmd)))?;
    if !out.status.success() {
        bail!("`{}` exited with {}", describe(cmd), out.status);
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Comme `capture`, mais ne remonte jamais d'erreur (équivalent du `|| true`
/// des scripts pour `git describe --tags --always`).
pub fn capture_or_empty(cmd: &mut Command) -> String {
    cmd.stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

/// Vrai si la commande réussit, sortie masquée (pour les `git apply --check`).
pub fn succeeds(cmd: &mut Command) -> bool {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn nproc() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)
}
