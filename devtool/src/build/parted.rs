// GNU Parted - equivalent of check_parted()/apply_parted_patches()/build_parted()
// in build.sh.
// Supports both the pinned Git submodule and official GNU release archives.
//
// Parted is built as a fully static binary (no readline, no shared
// libraries) because the Senbit rootfs does not contain a libc.

use std::process::Command;

use anyhow::{Result, bail};

use crate::cache::Cache;
use crate::paths::Paths;
use crate::proc::run_quiet;
use crate::{gitutil, ui};

fn read_version(p: &Paths) -> Result<String> {
    let file = p.parted_version_file();
    if !file.is_file() {
        bail!("Parted version file not found:\n  {}", file.display());
    }

    let v: String = std::fs::read_to_string(&file)?
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();

    if v.is_empty() {
        bail!("GNU Parted version is empty.");
    }

    Ok(v)
}

fn read_archive_version(dir: &std::path::Path) -> Result<String> {
    for marker in [".tarball-version", ".version"] {
        let file = dir.join(marker);

        if file.is_file() {
            let version = std::fs::read_to_string(&file)?.trim().to_string();

            if !version.is_empty() {
                return Ok(version);
            }
        }
    }

    bail!(
        "GNU Parted archive version marker not found:\n  {}\n\nExpected .tarball-version or .version.",
        dir.display()
    )
}

fn check(p: &Paths) -> Result<String> {
    let dir = p.parted_dir();

    if !dir.is_dir() || !dir.join("configure").is_file() {
        bail!(
            "GNU Parted source tree not found or incomplete:\n  {}\n\nExpected a source directory containing configure.",
            dir.display()
        );
    }

    let version = read_version(p)?;

    // Git submodule: preserve the strict tag verification.
    // Official release archive: use its embedded version marker.
    let current = if gitutil::is_repo(&dir) {
        gitutil::describe_exact(&dir)
    } else {
        read_archive_version(&dir)?
    };

    ui::info("GNU Parted");
    ui::detail(format!("Requested: v{version}"));
    ui::detail(format!("Current:   {current}"));

    let expected = format!("v{version}");
    let current_normalized = current.strip_prefix('v').unwrap_or(&current);

    if current_normalized != version {
        bail!("GNU Parted source version mismatch.\n\nRequested: {version}\nFound:     {current}");
    }

    // Keep the variable for the existing build fingerprint.
    let _ = expected;

    Ok(version)
}

fn apply_patches(p: &Paths) -> Result<()> {
    crate::patches::apply(p, "parted", "GNU Parted", &p.parted_dir())
}

pub fn build(p: &Paths, cache: &mut Cache, jobs: usize) -> Result<()> {
    let build_dir = p.parted_build_dir();
    std::fs::create_dir_all(&build_dir)?;

    let version = check(p)?;
    apply_patches(p)?;

    let dir = p.parted_dir();
    let binary = p.parted_binary();
    let fp = format!(
        "v{version}:patches={}",
        crate::patches::fingerprint(p, "parted")
    );

    if cache.is_fresh("parted", &fp, &[&binary]) {
        println!();
        ui::skip("GNU Parted already built for this version - skipping compilation.");
        ui::detail(format!("Parted: {}", binary.display()));
        return Ok(());
    }

    println!();
    ui::info("Preparing GNU Parted...");

    // Official release archives already contain generated Gnulib files.
    // A separate gnulib/ directory is only needed when bootstrapping Git sources.
    if !dir.join("lib/gnulib.mk").is_file() {
        bail!(
            "GNU Parted Gnulib files are missing:\n  {}\n\nExpected lib/gnulib.mk in the release source tree.",
            dir.join("lib/gnulib.mk").display()
        );
    }

    if !dir.join("configure").is_file() {
        bail!(
            "GNU Parted configure script not found:\n  {}",
            dir.join("configure").display()
        );
    }

    if !build_dir.join("Makefile").is_file() {
        ui::info("Configuring GNU Parted...");

        run_quiet(
            Command::new(dir.join("configure"))
                .current_dir(&build_dir)
                .args([
                    "--prefix=/usr",
                    "--bindir=/usr/bin",
                    "--sbindir=/usr/sbin",
                    "--libdir=/usr/lib",
                    "--disable-device-mapper",
                    // Do NOT pass --disable-debug: it turns PED_ASSERT() into
                    // a no-op, and parted 3.6 (labels/atari.c) calls newlocale()
                    // inside a PED_ASSERT, so the locale stays NULL and
                    // parted crashes in freelocale() when it exits.
                    "--enable-debug",
                    "--without-readline",
                ]),
        )?;
    }

    println!();
    ui::info("Building GNU Parted...");
    ui::detail(format!("Jobs: {jobs}"));

    let spin = ui::spinner("Compiling GNU Parted...");

    let result = run_quiet(
        Command::new("make")
            .arg("-C")
            .arg(&build_dir)
            .arg(format!("-j{jobs}"))
            .arg(format!(
                "LDFLAGS=-static -L{}",
                p.util_linux_build_dir().join(".libs").display()
            )),
    );

    spin.finish_and_clear();
    result?;

    println!();
    ui::info("Installing GNU Parted...");

    let install = build_dir.join("_install");
    let _ = std::fs::remove_dir_all(&install);

    let spin = ui::spinner("Installing GNU Parted...");

    let result = run_quiet(
        Command::new("make")
            .arg("-C")
            .arg(&build_dir)
            .arg(format!("DESTDIR={}", install.display()))
            .arg("install"),
    );

    spin.finish_and_clear();
    result?;

    if !binary.is_file() {
        bail!(
            "GNU Parted binary was not produced:\n  {}",
            binary.display()
        );
    }

    println!();
    println!("GNU Parted:");
    println!("  {}", binary.display());

    cache.record("parted", &fp)?;

    Ok(())
}
