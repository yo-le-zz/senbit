//! Userspace Rust (binaire senbit) — équivalent de build_rust() dans build.sh.

use std::process::Command;

use anyhow::{bail, Result};

use crate::cache::Cache;
use crate::paths::Paths;
use crate::proc::run_quiet;
use crate::{gitutil, ui};

pub fn build(p: &Paths, cache: &mut Cache, jobs: usize) -> Result<()> {
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
    let result = run_quiet(Command::new("cargo").current_dir(&p.root).args([
        "build",
        "--release",
        "--target",
        Paths::RUST_TARGET,
        "-j",
        &jobs.to_string(),
    ]));
    spin.finish_and_clear();
    result?;

    if !binary.is_file() {
        bail!("Senbit init binary was not produced:\n  {}", binary.display());
    }

    println!();
    println!("Senbit init:");
    println!("  {}", binary.display());

    cache.record("rust", &fp)?;
    Ok(())
}
