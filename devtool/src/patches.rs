//! Patches Senbit appliqués aux sources téléchargées.
//!
//! Un patch = un fichier `patches/<composant>/NNNN-description.patch` au
//! format diff unifié (`-p1`), appliqué dans l'ordre alphabétique. Appliquer
//! deux fois est sans effet (détection « déjà appliqué »), donc `devtool build`
//! peut rappeler cette étape à chaque fois.
//!
//! Composants : linux, busybox, util-linux, parted, systemd.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Result};

use crate::paths::Paths;
use crate::proc::{run, succeeds};
use crate::{host, ui};

fn list_patches(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut patches: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("patch"))
        .collect();
    patches.sort();
    patches
}

fn patch_cmd(source: &Path, patch: &Path, extra: &[&str]) -> Command {
    let mut cmd = host::command("patch");
    cmd.args(["-p1", "-s", "-N", "-d"])
        .arg(source)
        .args(extra)
        .arg("-i")
        .arg(patch);
    cmd
}

/// Applique tous les patches de `component` sur `source`.
pub fn apply(p: &Paths, component: &str, label: &str, source: &Path) -> Result<()> {
    let mut patches = list_patches(&p.patch_dir(component));
    patches.extend(list_patches(&p.legacy_patch_dir(component)));

    if patches.is_empty() {
        ui::detail(format!("No {label} patches."));
        return Ok(());
    }

    println!();
    ui::info(format!("Applying Senbit {label} patches..."));

    for patch in &patches {
        let name = patch
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        if succeeds(&mut patch_cmd(source, patch, &["--dry-run"])) {
            run(&mut patch_cmd(source, patch, &[]))?;
            ui::detail(format!("Applied:         {name}"));
        } else if succeeds(&mut patch_cmd(source, patch, &["--dry-run", "-R"])) {
            ui::detail(format!("Already applied: {name}"));
        } else {
            bail!(
                "{label} patch cannot be applied:\n  {}\n\nThe patch probably needs updating for the pinned {label} version.",
                patch.display()
            );
        }
    }
    ui::ok(format!("{label} patches applied."));
    Ok(())
}

/// Résumé de tous les patches (pour l'empreinte de cache : changer un patch
/// relance la compilation).
pub fn fingerprint(p: &Paths, component: &str) -> String {
    let mut all = list_patches(&p.patch_dir(component));
    all.extend(list_patches(&p.legacy_patch_dir(component)));
    all.iter()
        .map(|f| {
            format!(
                "{}={}",
                f.file_name().unwrap_or_default().to_string_lossy(),
                crate::cache::hash_file(f)
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}
