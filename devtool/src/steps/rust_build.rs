use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Result};

use crate::common::*;

const RUST_TARGET: &str = "x86_64-unknown-linux-musl";

pub fn senbit_init_binary(p: &Paths) -> PathBuf {
    p.join(&format!("target/{RUST_TARGET}/release/senbit-init"))
}

pub fn build_rust(p: &Paths, cache: &mut StateCache) -> Result<()> {
    info("Building Senbit Rust userspace...");

    let cargo_toml = p.join("Cargo.toml");
    if !cargo_toml.is_file() {
        bail!("Cargo.toml not found:\n  {}", cargo_toml.display());
    }

    let src_dir = p.join("src");
    let fingerprint = tree_fingerprint(&src_dir);
    let binary = senbit_init_binary(p);

    if cache.is_up_to_date("rust", &fingerprint, &binary) {
        println!();
        ok("Senbit Rust sources unchanged since last build - skipping compilation.");
        detail(format!("Senbit init: {}", binary.display()));
        return Ok(());
    }

    let spin = spinner("Compiling Senbit Rust userspace (cargo build --release)...");
    let result = run(Command::new("cargo")
        .current_dir(p.root.clone())
        .args(["build", "--release", "--target", RUST_TARGET]));
    spin.finish_and_clear();
    result?;

    if !binary.is_file() {
        bail!("Senbit init binary was not produced:\n  {}", binary.display());
    }

    println!();
    println!("Senbit init:");
    println!("  {}", binary.display());

    cache.mark_built("rust", &fingerprint)?;
    Ok(())
}
