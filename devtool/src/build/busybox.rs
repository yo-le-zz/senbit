//! BusyBox — équivalent de update_busybox()/apply_busybox_patches()/build_busybox()
//! dans build.sh. Contrairement au noyau, BusyBox se met à jour automatiquement
//! vers la dernière version à chaque build.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Result};

use crate::cache::{git_fingerprint, hash_file, Cache};
use crate::paths::Paths;
use crate::proc::{run, run_quiet, succeeds};
use crate::{gitutil, ui};

/// Utilisé par `devtool build busybox` : erreur si les sources n'existent pas
/// encore (comme update_busybox() dans build.sh).
pub(crate) fn update_only(p: &Paths) -> Result<()> {
    let dir = p.busybox_dir();

    if !gitutil::is_repo(&dir) {
        bail!(
            "BusyBox source tree not found:\n  {}\n\nRun:\n  devtool get busybox",
            dir.display()
        );
    }

    update_existing(p, &dir)
}

/// Utilisé par `devtool get busybox` : clone à la dernière version si les
/// sources n'existent pas encore (il n'existait pas d'ancien script dédié
/// à la seule récupération de BusyBox).
pub(crate) fn fetch(p: &Paths) -> Result<()> {
    let dir = p.busybox_dir();

    if !gitutil::is_repo(&dir) {
        let latest =
            gitutil::latest_tag(Paths::BUSYBOX_REPO, gitutil::is_busybox_tag)?;

        ui::info("BusyBox source tree not found.");
        ui::info("Cloning BusyBox...");
        ui::detail(format!("Version: {latest}"));

        let spin =
            ui::spinner(&format!("Cloning BusyBox {latest} (depth 1)..."));

        let result =
            gitutil::clone_tag(Paths::BUSYBOX_REPO, &latest, &dir);

        spin.finish_and_clear();

        result?;

        std::fs::write(
            p.busybox_version_file(),
            format!("{}\n", latest.replace('_', ".")),
        )?;

        println!();
        ui::ok(format!("BusyBox {latest} downloaded."));

        return Ok(());
    }

    update_existing(p, &dir)
}

fn update_existing(p: &Paths, dir: &std::path::Path) -> Result<()> {
    let dir = dir.to_path_buf();

    let latest =
        gitutil::latest_tag(Paths::BUSYBOX_REPO, gitutil::is_busybox_tag)?;

    let current = gitutil::describe(&dir);

    ui::info("BusyBox version");
    ui::detail(format!("Current: {current}"));
    ui::detail(format!("Latest:  {latest}"));

    if current == latest {
        ui::ok("BusyBox is already up to date.");
        return Ok(());
    }

    println!();
    ui::info("Updating BusyBox...");
    ui::detail(format!("{current} -> {latest}"));

    let status = gitutil::status_porcelain(&dir);

    if !status.is_empty() {
        bail!(
            "BusyBox source tree contains local modifications:\n\n{status}\n\nSenbit patches must be stored in:\n  third_party/patches/busybox/\n\nThe working tree must be clean before updating BusyBox."
        );
    }

    gitutil::fetch_tags(&dir)?;
    gitutil::checkout_detach(&dir, &latest)?;

    let version = latest.replace('_', ".");

    std::fs::write(
        p.busybox_version_file(),
        format!("{version}\n"),
    )?;

    ui::ok("BusyBox updated.");

    Ok(())
}

fn apply_patches(p: &Paths) -> Result<()> {
    let patch_dir = p.busybox_patch_dir();

    if !patch_dir.is_dir() {
        ui::info("No BusyBox patches directory.");
        return Ok(());
    }

    let mut patches: Vec<PathBuf> = std::fs::read_dir(&patch_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                == Some("patch")
        })
        .collect();

    patches.sort();

    if patches.is_empty() {
        ui::info("No BusyBox patches to apply.");
        return Ok(());
    }

    println!();
    ui::info("Applying Senbit BusyBox patches...");

    let dir = p.busybox_dir();

    for patch in &patches {
        let name = patch
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();

        println!();
        ui::detail(format!("Applying: {name}"));

        if succeeds(
            Command::new("git")
                .arg("-C")
                .arg(&dir)
                .arg("apply")
                .arg("--check")
                .arg(patch),
        ) {
            run(
                Command::new("git")
                    .arg("-C")
                    .arg(&dir)
                    .arg("apply")
                    .arg(patch),
            )?;
        } else if succeeds(
            Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(["apply", "--reverse", "--check"])
                .arg(patch),
        ) {
            ui::detail(format!("Already applied: {name}"));
        } else {
            bail!(
                "BusyBox patch cannot be applied:\n  {}\n\nBusyBox source version:\n{}\n\nThe patch may need to be updated for this BusyBox version.",
                patch.display(),
                gitutil::describe(&dir)
            );
        }
    }

    println!();
    ui::ok("BusyBox patches applied.");

    Ok(())
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

    if build_dir.join(".config").is_file()
        || build_dir.join("_install").is_dir()
    {
        println!();
        ui::info(
            "BusyBox configuration changed - resetting build directory...",
        );

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
/// et permet à `devtool build` de fonctionner avec stdin=/dev/null.
fn prepare_config(
    dir: &std::path::Path,
    build_dir: &std::path::Path,
    config: &std::path::Path,
) -> Result<()> {
    if !build_dir.join(".config").is_file() {
        ui::info("Installing Senbit BusyBox configuration...");

        std::fs::copy(
            config,
            build_dir.join(".config"),
        )?;
    }

    println!();
    ui::info("Synchronizing BusyBox configuration...");

    let o = format!("O={}", build_dir.display());

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

pub fn build(
    p: &Paths,
    cache: &mut Cache,
    jobs: usize,
) -> Result<()> {
    let build_dir = p.busybox_build_dir();

    std::fs::create_dir_all(&build_dir)?;

    update_only(p)?;
    apply_patches(p)?;

    let config = p.busybox_config();

    if !config.is_file() {
        bail!(
            "BusyBox configuration not found:\n  {}",
            config.display()
        );
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

    let build_fp = format!(
        "{source_fp}:config={config_hash}"
    );

    let previous_config_hash = cache
        .get("busybox-config")
        .map(str::to_string);

    reset_if_config_changed(
        &build_dir,
        &config_hash,
        previous_config_hash.as_deref(),
    )?;

    let binary = p.busybox_binary();

    if cache.is_fresh(
        "busybox",
        &build_fp,
        &[&binary],
    ) {
        println!();
        ui::skip(
            "BusyBox sources and configuration unchanged since last build - skipping compilation.",
        );
        ui::detail(format!(
            "BusyBox: {}",
            binary.display()
        ));

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
    prepare_config(
        &dir,
        &build_dir,
        &config,
    )?;

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
        bail!(
            "BusyBox binary was not produced:\n  {}",
            binary.display()
        );
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
    cache.record(
        "busybox",
        &build_fp,
    )?;

    cache.record(
        "busybox-config",
        &config_hash,
    )?;

    Ok(())
}