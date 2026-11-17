//! Userspace Rust : `senbit` (init, installeur) et `senbit-login` (login,
//! lancé par systemd). Deux crates indépendantes, deux binaires statiques.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Result};

use crate::cache::Cache;
use crate::paths::Paths;
use crate::proc::run_quiet;
use crate::{gitutil, ui};

/// Configure le compilateur C et le linker pour les builds Rust ciblant musl.
pub(crate) fn configure_rust_command(cmd: &mut Command, target: &str) -> Result<()> {
    if target == "x86_64-unknown-linux-musl" {
        let compiler = "musl-gcc";

        let available = Command::new(compiler)
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);

        if !available {
            bail!(
                "Missing musl C compiler: {compiler}\n\
                 Install it with: sudo dnf install musl-gcc musl-libc-static"
            );
        }

        cmd.env("CC_x86_64_unknown_linux_musl", compiler);
        cmd.env(
            "CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER",
            compiler,
        );
    }

    Ok(())
}

pub fn build(p: &Paths, cache: &mut Cache, jobs: usize) -> Result<()> {
    build_senbit(p, cache, jobs)?;
    build_login(p, cache, jobs)
}

fn build_senbit(p: &Paths, cache: &mut Cache, jobs: usize) -> Result<()> {
    ui::info("Building Senbit Rust userspace...");

    let cargo_toml = p.r("Cargo.toml");
    if !cargo_toml.is_file() {
        bail!("Cargo.toml not found:\n  {}", cargo_toml.display());
    }

    let binary = p.senbit_binary();

    // Le code source vit dans le même dépôt (pas de sous-module) : on utilise
    // l'état git de la racine du projet comme empreinte.
    let fp = gitutil::describe(&p.root) + &gitutil::status_porcelain(&p.root);

    if cache.is_fresh("rust", &fp, &[&binary]) {
        println!();
        ui::skip("Senbit Rust sources unchanged since last build - skipping compilation.");
        ui::detail(format!("Senbit init: {}", binary.display()));
        return Ok(());
    }

    let spin = ui::spinner("Compiling Senbit Rust userspace (cargo build --release)...");

    let mut cmd = Command::new("cargo");
    cmd.current_dir(&p.root).args([
        "build",
        "--release",
        "--target",
        p.rust_target(),
        "-j",
        &jobs.to_string(),
    ]);

    configure_rust_command(&mut cmd, p.rust_target())?;

    let result = run_quiet(&mut cmd);

    spin.finish_and_clear();
    result?;

    if !binary.is_file() {
        bail!(
            "Senbit init binary was not produced:\n  {}",
            binary.display()
        );
    }

    println!();
    println!("Senbit init:");
    println!("  {}", binary.display());

    cache.record("rust", &fp)?;
    Ok(())
}

/// Empreinte du contenu de la crate login (Cargo.toml, Cargo.lock, src/**).
/// Indépendante de l'état git du reste du dépôt : le login ne se recompile
/// que si ses propres fichiers changent.
fn hash_tree(dir: &Path, hasher: &mut DefaultHasher) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(dir)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();

        if path.is_dir() {
            if name == "target" {
                continue;
            }

            name.hash(hasher);
            hash_tree(&path, hasher)?;
        } else {
            name.hash(hasher);
            fs::read(&path)?.hash(hasher);
        }
    }

    Ok(())
}

fn build_login(p: &Paths, cache: &mut Cache, jobs: usize) -> Result<()> {
    let dir = p.login_dir();
    let manifest = dir.join("Cargo.toml");

    if !manifest.is_file() {
        bail!(
            "senbit-login crate not found:\n  {}\n\n\
             The login is a separate component run by systemd (senbit-login.service).",
            manifest.display()
        );
    }

    ui::info("Building senbit-login (systemd login component)...");

    let binary = p.login_binary();

    let mut hasher = DefaultHasher::new();
    hash_tree(&dir, &mut hasher)?;

    let fp = format!("login:{:016x}:{}", hasher.finish(), p.rust_target());

    if cache.is_fresh("login", &fp, &[&binary]) {
        println!();
        ui::skip("senbit-login sources unchanged since last build - skipping compilation.");
        ui::detail(format!("senbit-login: {}", binary.display()));
        return Ok(());
    }

    let spin = ui::spinner("Compiling senbit-login (cargo build --release)...");

    let mut cmd = Command::new("cargo");
    cmd.current_dir(&dir)
        .env("CARGO_TARGET_DIR", p.login_target_dir())
        .args(["build", "--release", "--target", p.rust_target(), "-j"])
        .arg(jobs.to_string())
        .arg("--manifest-path")
        .arg(&manifest);

    configure_rust_command(&mut cmd, p.rust_target())?;

    let result = run_quiet(&mut cmd);

    spin.finish_and_clear();
    result?;

    if !binary.is_file() {
        bail!(
            "senbit-login binary was not produced:\n  {}",
            binary.display()
        );
    }

    println!();
    println!("senbit-login:");
    println!("  {}", binary.display());

    cache.record("login", &fp)?;
    Ok(())
}
