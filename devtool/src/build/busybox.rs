//! BusyBox — équivalent de update_busybox()/apply_busybox_patches()/build_busybox()
//! dans build.sh. BusyBox est épinglé comme le reste (config/busybox/version).

use std::process::Command;

use anyhow::{bail, Result};

use crate::cache::{git_fingerprint, hash_file, Cache};
use crate::paths::Paths;
use crate::proc::run_quiet;
use crate::{gitutil, ui};

/// Utilisé par `devtool build busybox` : les sources doivent déjà être à la
/// version ÉPINGLÉE (config/busybox/version) ; plus aucune mise à jour
/// automatique vers « la dernière version ».
pub(crate) fn update_only(p: &Paths) -> Result<()> {
    let dir = p.busybox_dir();

    if !gitutil::is_repo(&dir) {
        bail!(
            "BusyBox source tree not found:\n  {}\n\nRun:\n  devtool get busybox",
            dir.display()
        );
    }

    let version = crate::get::read_pinned(&p.busybox_version_file(), "BusyBox version")?;
    let tag = version.replace('.', "_");
    let current = gitutil::describe_exact(&dir);

    ui::info("BusyBox");
    ui::detail(format!("Pinned:  {tag}"));
    ui::detail(format!("Current: {current}"));

    if current != tag {
        bail!("BusyBox is not on the pinned version {tag}.\n\nRun:\n  devtool get busybox");
    }
    Ok(())
}

fn apply_patches(p: &Paths) -> Result<()> {
    crate::patches::apply(p, "busybox", "BusyBox", &p.busybox_dir())
}

/// Vérifie si la configuration source a changé depuis la dernière
/// configuration utilisée pour BusyBox.
///
/// Si elle a changé, les fichiers générés à partir de l'ancienne
/// configuration sont supprimés.
///
/// IMPORTANT : cette fonction ne modifie PAS le cache. Le cache de
/// configuration n'est enregistré qu'après un build réussi.
fn reset_if_config_changed(
    build_dir: &std::path::Path,
    config_hash: &str,
    previous_config_hash: Option<&str>,
) -> Result<()> {
    if previous_config_hash == Some(config_hash) {
        return Ok(());
    }

    if build_dir.join(".config").is_file() || build_dir.join("_install").is_dir() {
        println!();
        ui::info("BusyBox configuration changed - resetting build directory...");

        let _ = std::fs::remove_file(build_dir.join(".config"));
        let _ = std::fs::remove_dir_all(build_dir.join("_install"));
    }

    Ok(())
}

/// Synchronise la configuration BusyBox sans jamais lancer une
/// configuration Kconfig interactive.
///
/// `olddefconfig` prend les valeurs par défaut pour les nouveaux symboles,
/// ce qui évite les prompts du genre :
///
///     Support --version (FEATURE_VERSION) [Y/n] (NEW)
///
/// et permet à `devtool build` de fonctionner avec stdin=/dev/null.c
fn prepare_config(
    dir: &std::path::Path,
    build_dir: &std::path::Path,
    config: &std::path::Path,
) -> Result<()> {
    if !build_dir.join(".config").is_file() {
        ui::info("Installing Senbit BusyBox configuration...");

        std::fs::copy(config, build_dir.join(".config"))?;
    }

    println!();
    ui::info("Synchronizing BusyBox configuration...");

    let o = format!("O={}", build_dir.display());

    // Répond automatiquement avec les valeurs par défaut.
    let mut yes = Command::new("yes")
        .arg("")
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    let yes_stdout = yes
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("Failed to capture yes stdout"))?;

    let mut make = Command::new("make")
        .arg("-C")
        .arg(dir)
        .arg(&o)
        .arg("oldconfig")
        .stdin(yes_stdout)
        .spawn()?;

    let make_status = make.wait()?;

    // make terminé : arrêter le processus yes.
    let _ = yes.kill();
    let _ = yes.wait();

    if !make_status.success() {
        bail!(
            "BusyBox configuration synchronization failed with status {}",
            make_status
        );
    }

    Ok(())
}

pub fn build(p: &Paths, cache: &mut Cache, jobs: usize) -> Result<()> {
    let build_dir = p.busybox_build_dir();

    std::fs::create_dir_all(&build_dir)?;

    update_only(p)?;
    apply_patches(p)?;

    let config = p.busybox_config();

    if !config.is_file() {
        bail!("BusyBox configuration not found:\n  {}", config.display());
    }

    let dir = p.busybox_dir();

    /*
     * Le fingerprint du build dépend maintenant :
     *
     * - des sources BusyBox ;
     * - de config/busybox.config.
     *
     * Ainsi, modifier uniquement la configuration force bien une
     * recompilation, même si les sources BusyBox sont inchangées.
     */
    let source_fp = git_fingerprint(&dir);
    let config_hash = hash_file(&config);

    let build_fp = format!("{source_fp}:config={config_hash}");

    let previous_config_hash = cache.get("busybox-config").map(str::to_string);

    reset_if_config_changed(&build_dir, &config_hash, previous_config_hash.as_deref())?;

    let binary = p.busybox_binary();

    if cache.is_fresh("busybox", &build_fp, &[&binary]) {
        println!();
        ui::skip(
            "BusyBox sources and configuration unchanged since last build - skipping compilation.",
        );
        ui::detail(format!("BusyBox: {}", binary.display()));

        return Ok(());
    }

    println!();
    ui::info("Preparing BusyBox...");

    /*
     * Copie la configuration puis lance explicitement olddefconfig.
     *
     * Cela est volontairement fait AVANT le make principal pour empêcher
     * Kconfig de lancer une configuration interactive pendant la compilation.
     */
    prepare_config(&dir, &build_dir, &config)?;

    println!();
    ui::info("Building BusyBox...");
    ui::detail(format!("Jobs: {jobs}"));
    ui::detail("Incremental build enabled.");

    let o = format!("O={}", build_dir.display());

    let spin = ui::spinner("Compiling BusyBox...");

    let result = run_quiet(
        Command::new("make")
            .arg("-C")
            .arg(&dir)
            .arg(&o)
            .arg(format!("-j{jobs}")),
    );

    spin.finish_and_clear();

    result?;

    println!();
    ui::info("Installing BusyBox...");

    let spin = ui::spinner("Installing BusyBox...");

    let result = run_quiet(
        Command::new("make")
            .arg("-C")
            .arg(&dir)
            .arg(&o)
            .arg(format!(
                "CONFIG_PREFIX={}",
                build_dir.join("_install").display()
            ))
            .arg("install"),
    );

    spin.finish_and_clear();

    result?;

    if !binary.is_file() {
        bail!("BusyBox binary was not produced:\n  {}", binary.display());
    }

    println!();
    println!("BusyBox:");
    println!("  {}", binary.display());

    /*
     * IMPORTANT :
     * On n'enregistre les empreintes qu'après un build + install réussi.
     * Si le build échoue, le prochain lancement devra donc réellement
     * considérer BusyBox comme non construit.
     */
    cache.record("busybox", &build_fp)?;

    cache.record("busybox-config", &config_hash)?;

    Ok(())
}
