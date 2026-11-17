
use anyhow::{Context, Result};
use colored::Colorize;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::Config;

/*
 * The Senbit rootfs has no libc and no shared libraries, so every
 * GRUB host tool (grub-install, grub-mkimage, grub-probe, ...) must
 * be fully static. Optional features that pull extra shared
 * libraries are disabled.
 */
const STATIC_HOST_ARGS: &[&str] = &[
    "--disable-nls",
    "--disable-grub-mkfont",
    "--disable-grub-themes",
    "--disable-device-mapper",
    "--disable-liblzma",
];

pub fn build(
    config: &Config,
    source: &Path,
    output: &Path,
) -> Result<()> {
    println!("{}", "Building GRUB targets...".cyan().bold());

    let build_dir_legacy = output.join("build-i386-pc");
    let build_dir_efi = output.join("build-x86_64-efi");

    std::fs::create_dir_all(&build_dir_legacy)?;
    std::fs::create_dir_all(&build_dir_efi)?;

    let source = source.canonicalize()?;
    let build_dir_legacy = build_dir_legacy.canonicalize()?;
    let build_dir_efi = build_dir_efi.canonicalize()?;
    let output = output.canonicalize()?;

    prepare_source(&source)?;

    configure_legacy(config, &source, &build_dir_legacy)?;
    build_legacy(config, &build_dir_legacy)?;

    configure_efi(config, &source, &build_dir_efi)?;
    build_efi(config, &build_dir_efi)?;

    install_grub(&build_dir_legacy, &build_dir_efi, &output)?;

    println!(
        "{}",
        "GRUB i386-pc + x86_64-efi builds completed successfully!"
            .green()
            .bold()
    );

    Ok(())
}

fn prepare_source(source: &Path) -> Result<()> {
    let autogen = source.join("autogen.sh");
    let configure = source.join("configure");

    if !autogen.exists() {
        anyhow::bail!("GRUB autogen.sh not found: {}", autogen.display());
    }

    if configure.exists() {
        println!("{} {}", "GRUB source already prepared:".cyan(), source.display());
        return Ok(());
    }

    println!("{}", "Preparing GRUB source...".cyan());

    let status = Command::new("sh")
        .arg(&autogen)
        .current_dir(source)
        .status()
        .context("Failed to execute GRUB autogen.sh")?;

    if !status.success() {
        anyhow::bail!("GRUB autogen.sh failed with status {}", status);
    }

    Ok(())
}

fn configure_legacy(
    config: &Config,
    source: &Path,
    build_dir: &Path,
) -> Result<()> {
    configure_target(
        config,
        source,
        build_dir,
        "i386",
        "pc",
        "GRUB Legacy",
    )
}

fn configure_efi(
    config: &Config,
    source: &Path,
    build_dir: &Path,
) -> Result<()> {
    configure_target(
        config,
        source,
        build_dir,
        "x86_64",
        "efi",
        "GRUB UEFI",
    )
}

fn configure_target(
    _config: &Config,
    source: &Path,
    build_dir: &Path,
    target: &str,
    platform: &str,
    name: &str,
) -> Result<()> {
    if build_dir.join("Makefile").exists() {
        println!(
            "{} {}",
            format!("{name} already configured:").cyan(),
            build_dir.display()
        );
        return Ok(());
    }

    println!("{}", format!("Configuring {name}...").cyan());

    let configure = source.join("configure");

    let status = Command::new("sh")
        .arg(&configure)
        .arg(format!("--target={target}"))
        .arg(format!("--with-platform={platform}"))
        .arg("--prefix=/usr")
        .arg("--disable-werror")
        .args(STATIC_HOST_ARGS)
        .env("LDFLAGS", "-static")
        .current_dir(build_dir)
        .status()
        .with_context(|| format!("Failed to configure {name}"))?;

    if !status.success() {
        anyhow::bail!("Configuring {name} failed with status {}", status);
    }

    Ok(())
}

fn build_legacy(config: &Config, build_dir: &Path) -> Result<()> {
    build_target(config, build_dir, "i386/pc")
}

fn build_efi(config: &Config, build_dir: &Path) -> Result<()> {
    build_target(config, build_dir, "x86_64/efi")
}

fn build_target(
    config: &Config,
    build_dir: &Path,
    name: &str,
) -> Result<()> {
    let kernel_img = build_dir.join("grub-core/kernel.img");
    let grub_install = build_dir.join("grub-install");

    if kernel_img.exists() && grub_install.exists() {
        println!(
            "{}",
            format!("GRUB {name} already compiled - skipping compilation.").cyan()
        );
        return Ok(());
    }

    println!("{}", format!("Compiling GRUB {name}...").cyan());

    let mut command = Command::new("make");
    command.current_dir(build_dir);
    add_jobs(&mut command, config);

    let status = command
        .status()
        .context("Failed to execute GRUB make")?;

    if !status.success() {
        anyhow::bail!("Building GRUB {name} failed with status {}", status);
    }

    Ok(())
}

fn add_jobs(command: &mut Command, config: &Config) {
    match config.build.jobs.as_str() {
        "auto" => {
            let jobs = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1);

            command.arg(format!("-j{jobs}"));
        }
        jobs => {
            command.arg(format!("-j{jobs}"));
        }
    }
}

/// Find the GRUB platform directory regardless of whether the
/// installation uses lib or lib64.
fn find_grub_platform_dir(
    staging: &Path,
    platform: &str,
) -> Result<PathBuf> {
    let candidates = [
        staging.join("usr/lib/grub").join(platform),
        staging.join("usr/lib64/grub").join(platform),
    ];

    for candidate in &candidates {
        if candidate.join("kernel.img").is_file() {
            return Ok(candidate.clone());
        }
    }

    anyhow::bail!(
        "GRUB platform '{}' kernel.img is missing. Checked:\n  {}",
        platform,
        candidates
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n  ")
    )
}

fn validate_installation(staging: &Path) -> Result<()> {
    let grub_install = staging.join("usr/sbin/grub-install");

    if !grub_install.is_file() {
        anyhow::bail!(
            "GRUB grub-install is missing:\n  {}",
            grub_install.display()
        );
    }

    find_grub_platform_dir(staging, "i386-pc")?;
    find_grub_platform_dir(staging, "x86_64-efi")?;

    for tool in ["grub-install", "grub-mkimage", "grub-probe"] {
        let mut bin = staging.join("usr/sbin").join(tool);

        if !bin.is_file() {
            bin = staging.join("usr/bin").join(tool);
        }

        if bin.is_file() {
            ensure_static(&bin)?;
        }
    }

    Ok(())
}

fn install_grub(
    build_dir_legacy: &Path,
    build_dir_efi: &Path,
    output: &Path,
) -> Result<()> {
    let staging = output.join("stage");

    if staging.exists()
        && validate_installation(&staging).is_ok()
    {
        println!(
            "{}",
            "GRUB staging already contains BIOS + UEFI - skipping install."
                .cyan()
        );

        stage_rootfs(&staging, output)?;
        return Ok(());
    }

    println!(
        "{}",
        "Installing GRUB BIOS + UEFI into staging directory...".cyan()
    );

    if staging.exists() {
        std::fs::remove_dir_all(&staging)
            .context("Failed to clean incomplete GRUB staging directory")?;
    }

    std::fs::create_dir_all(&staging)?;

    let staging = staging.canonicalize()?;

    for (name, build_dir) in [
        ("i386/pc", build_dir_legacy),
        ("x86_64/efi", build_dir_efi),
    ] {
        println!("{}", format!("Installing GRUB {name}...").cyan());

        let status = Command::new("make")
            .arg("install")
            .arg(format!("DESTDIR={}", staging.display()))
            .current_dir(build_dir)
            .status()
            .context("Failed to execute GRUB make install")?;

        if !status.success() {
            anyhow::bail!("Installing GRUB {name} failed with status {}", status);
        }
    }

    validate_installation(&staging)?;

    println!(
        "{} {}",
        "GRUB BIOS + UEFI installed into".green(),
        staging.display()
    );

    stage_rootfs(&staging, output)?;

    Ok(())
}

fn stage_rootfs(staging: &Path, output: &Path) -> Result<()> {
    validate_installation(staging)?;

    let generated_rootfs = output.join("rootfs");

    if generated_rootfs.exists() {
        std::fs::remove_dir_all(&generated_rootfs)
            .context("Failed to clean previous GRUB rootfs staging")?;
    }

    std::fs::create_dir_all(&generated_rootfs)?;

    copy_recursive(staging, &generated_rootfs)?;

    println!(
        "{} {}",
        "GRUB resources staged in".green(),
        generated_rootfs.display()
    );

    Ok(())
}

fn copy_recursive(source: &Path, destination: &Path) -> Result<()> {
    std::fs::create_dir_all(destination)?;

    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());

        if source_path.is_dir() {
            copy_recursive(&source_path, &destination_path)?;
        } else {
            std::fs::copy(&source_path, &destination_path)
                .with_context(|| {
                    format!(
                        "Failed to copy {} to {}",
                        source_path.display(),
                        destination_path.display()
                    )
                })?;
        }
    }

    Ok(())
}

/// Fails if the ELF binary is dynamically linked (has a PT_INTERP header).
fn ensure_static(bin: &Path) -> Result<()> {
    let out = Command::new("file")
        .arg("-L")
        .arg(bin)
        .output()
        .context("Failed to execute `file`")?;

    if !out.status.success() {
        anyhow::bail!("`file` failed for {}", bin.display());
    }

    let text = String::from_utf8_lossy(&out.stdout);

    if text.contains("dynamically linked") {
        anyhow::bail!(
            "{} is dynamically linked but the rootfs has no libc:\n  {}",
            bin.display(),
            text.trim()
        );
    }

    Ok(())
}
