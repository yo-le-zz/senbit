use anyhow::{Context, Result};
use colored::Colorize;
use std::path::Path;
use std::process::Command;

use crate::config::Config;

pub fn build(
    config: &Config,
    source: &Path,
    output: &Path,
) -> Result<()> {
    println!("{}", "Building GRUB targets...".cyan().bold());

    let build_dir_legacy = output.join("build-i386-pc");
    let build_dir_efi = output.join("build-x86_64-efi");

    std::fs::create_dir_all(&build_dir_legacy)
        .context("Failed to create GRUB Legacy build directory")?;

    std::fs::create_dir_all(&build_dir_efi)
        .context("Failed to create GRUB UEFI build directory")?;

    let source = source
        .canonicalize()
        .context("Failed to resolve GRUB source path")?;

    let build_dir_legacy = build_dir_legacy
        .canonicalize()
        .context("Failed to resolve GRUB Legacy build path")?;

    let build_dir_efi = build_dir_efi
        .canonicalize()
        .context("Failed to resolve GRUB UEFI build path")?;

    let output = output
        .canonicalize()
        .context("Failed to resolve GRUB output path")?;

    prepare_source(&source)?;

    configure_legacy(
        config,
        &source,
        &build_dir_legacy,
    )?;

    build_legacy(
        config,
        &build_dir_legacy,
    )?;

    configure_efi(
        config,
        &source,
        &build_dir_efi,
    )?;

    build_efi(
        config,
        &build_dir_efi,
    )?;

    install_grub(
        &build_dir_legacy,
        &build_dir_efi,
        &output,
    )?;

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
        anyhow::bail!(
            "GRUB autogen.sh not found: {}",
            autogen.display()
        );
    }

    if configure.exists() {
        println!(
            "{} {}",
            "GRUB source already prepared:".cyan(),
            source.display()
        );

        return Ok(());
    }

    println!("{}", "Preparing GRUB source...".cyan());

    let status = Command::new("sh")
        .arg(&autogen)
        .current_dir(source)
        .status()
        .context("Failed to execute GRUB autogen.sh")?;

    if !status.success() {
        anyhow::bail!(
            "GRUB autogen.sh failed with status {}",
            status
        );
    }

    Ok(())
}

fn configure_legacy(
    config: &Config,
    source: &Path,
    build_dir: &Path,
) -> Result<()> {
    let makefile = build_dir.join("Makefile");

    if makefile.exists() {
        println!(
            "{} {}",
            "GRUB Legacy already configured:".cyan(),
            build_dir.display()
        );

        return Ok(());
    }

    println!("{}", "Configuring GRUB (i386/pc)...".cyan());

    let configure = source.join("configure");

    if !configure.exists() {
        anyhow::bail!(
            "GRUB configure script not found: {}",
            configure.display()
        );
    }

    let status = Command::new("sh")
        .arg(&configure)
        .arg("--target=i386")
        .arg("--with-platform=pc")
        .arg("--prefix=/usr")
        .arg("--disable-werror")
        .current_dir(build_dir)
        .status()
        .context("Failed to execute GRUB Legacy configure")?;

    if !status.success() {
        anyhow::bail!(
            "Configuring GRUB (i386/pc) failed with status {}",
            status
        );
    }

    let _ = config;

    Ok(())
}

fn build_legacy(
    config: &Config,
    build_dir: &Path,
) -> Result<()> {
    let kernel_img = build_dir
        .join("grub-core")
        .join("kernel.img");

    let grub_install = build_dir.join("grub-install");

    if kernel_img.exists() && grub_install.exists() {
        println!(
            "{}",
            "GRUB i386/pc already compiled - skipping compilation."
                .cyan()
        );

        return Ok(());
    }

    println!("{}", "Compiling GRUB (i386/pc)...".cyan());

    let mut command = Command::new("make");

    command.current_dir(build_dir);

    add_jobs(&mut command, config);

    let status = command
        .status()
        .context("Failed to execute GRUB Legacy make")?;

    if !status.success() {
        anyhow::bail!(
            "Building GRUB (i386/pc) failed with status {}",
            status
        );
    }

    Ok(())
}

fn configure_efi(
    config: &Config,
    source: &Path,
    build_dir: &Path,
) -> Result<()> {
    let makefile = build_dir.join("Makefile");

    if makefile.exists() {
        println!(
            "{} {}",
            "GRUB UEFI already configured:".cyan(),
            build_dir.display()
        );

        return Ok(());
    }

    println!("{}", "Configuring GRUB (x86_64/efi)...".cyan());

    let configure = source.join("configure");

    if !configure.exists() {
        anyhow::bail!(
            "GRUB configure script not found: {}",
            configure.display()
        );
    }

    let status = Command::new("sh")
        .arg(&configure)
        .arg("--target=x86_64")
        .arg("--with-platform=efi")
        .arg("--prefix=/usr")
        .arg("--disable-werror")
        .current_dir(build_dir)
        .status()
        .context("Failed to execute GRUB UEFI configure")?;

    if !status.success() {
        anyhow::bail!(
            "Configuring GRUB (x86_64/efi) failed with status {}",
            status
        );
    }

    let _ = config;

    Ok(())
}

fn build_efi(
    config: &Config,
    build_dir: &Path,
) -> Result<()> {
    let kernel_img = build_dir
        .join("grub-core")
        .join("kernel.img");

    let grub_install = build_dir.join("grub-install");

    if kernel_img.exists() && grub_install.exists() {
        println!(
            "{}",
            "GRUB x86_64/efi already compiled - skipping compilation."
                .cyan()
        );

        return Ok(());
    }

    println!("{}", "Compiling GRUB (x86_64/efi)...".cyan());

    let mut command = Command::new("make");

    command.current_dir(build_dir);

    add_jobs(&mut command, config);

    let status = command
        .status()
        .context("Failed to execute GRUB UEFI make")?;

    if !status.success() {
        anyhow::bail!(
            "Building GRUB (x86_64/efi) failed with status {}",
            status
        );
    }

    Ok(())
}

fn add_jobs(
    command: &mut Command,
    config: &Config,
) {
    match config.build.jobs.as_str() {
        "auto" => {
            let jobs = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1);

            command.arg(format!("-j{}", jobs));
        }

        jobs => {
            command.arg(format!("-j{}", jobs));
        }
    }
}

fn install_grub(
    build_dir_legacy: &Path,
    build_dir_efi: &Path,
    output: &Path,
) -> Result<()> {
    let staging = output.join("stage");

    let legacy_kernel =
        staging.join("usr/lib/grub/i386-pc/kernel.img");

    let efi_kernel =
        staging.join("usr/lib/grub/x86_64-efi/kernel.img");

    let grub_install =
        staging.join("usr/sbin/grub-install");

    if grub_install.is_file()
        && legacy_kernel.is_file()
        && efi_kernel.is_file()
    {
        println!(
            "{}",
            "GRUB staging already contains BIOS + UEFI - skipping install."
                .cyan()
        );

        println!(
            "{} {}",
            "GRUB staging directory:".cyan(),
            staging.display()
        );

        stage_rootfs(&staging, output)?;

        return Ok(());
    }

    println!(
        "{}",
        "Installing GRUB BIOS + UEFI into staging directory..."
            .cyan()
    );

    if staging.exists() {
        std::fs::remove_dir_all(&staging)
            .context("Failed to clean incomplete GRUB staging directory")?;
    }

    std::fs::create_dir_all(&staging)
        .context("Failed to create GRUB staging directory")?;

    let staging = staging
        .canonicalize()
        .context("Failed to resolve GRUB staging path")?;

    println!(
        "{} {}",
        "GRUB staging directory:".cyan(),
        staging.display()
    );

    println!("{}", "Installing GRUB i386/pc...".cyan());

    let status = Command::new("make")
        .arg("install")
        .arg(format!(
            "DESTDIR={}",
            staging.display()
        ))
        .current_dir(build_dir_legacy)
        .status()
        .context("Failed to execute GRUB Legacy make install")?;

    if !status.success() {
        anyhow::bail!(
            "Installing GRUB i386/pc failed with status {}",
            status
        );
    }

    println!("{}", "Installing GRUB x86_64/efi...".cyan());

    let status = Command::new("make")
        .arg("install")
        .arg(format!(
            "DESTDIR={}",
            staging.display()
        ))
        .current_dir(build_dir_efi)
        .status()
        .context("Failed to execute GRUB UEFI make install")?;

    if !status.success() {
        anyhow::bail!(
            "Installing GRUB x86_64/efi failed with status {}",
            status
        );
    }

    let grub_install =
        staging.join("usr/sbin/grub-install");

    let legacy_kernel =
        staging.join("usr/lib/grub/i386-pc/kernel.img");

    let efi_kernel =
        staging.join("usr/lib/grub/x86_64-efi/kernel.img");

    if !grub_install.is_file() {
        anyhow::bail!(
            "GRUB installation completed but grub-install is missing:\n  {}",
            grub_install.display()
        );
    }

    if !legacy_kernel.is_file() {
        anyhow::bail!(
            "GRUB installation completed but i386-pc kernel.img is missing:\n  {}",
            legacy_kernel.display()
        );
    }

    if !efi_kernel.is_file() {
        anyhow::bail!(
            "GRUB installation completed but x86_64-efi kernel.img is missing:\n  {}",
            efi_kernel.display()
        );
    }

    println!(
        "{} {}",
        "GRUB BIOS + UEFI installed into".green(),
        staging.display()
    );

    stage_rootfs(&staging, output)?;

    Ok(())
}

fn stage_rootfs(
    staging: &Path,
    output: &Path,
) -> Result<()> {
    let generated_rootfs = output.join("rootfs");

    if generated_rootfs.exists() {
        let existing_grub_install =
            generated_rootfs.join("usr/sbin/grub-install");

        let existing_legacy_kernel =
            generated_rootfs.join("usr/lib/grub/i386-pc/kernel.img");

        let existing_efi_kernel =
            generated_rootfs.join("usr/lib/grub/x86_64-efi/kernel.img");

        if existing_grub_install.is_file()
            && existing_legacy_kernel.is_file()
            && existing_efi_kernel.is_file()
        {
            println!(
                "{}",
                "GRUB rootfs staging already contains BIOS + UEFI - skipping copy."
                    .cyan()
            );

            return Ok(());
        }

        std::fs::remove_dir_all(&generated_rootfs)
            .context("Failed to clean incomplete GRUB rootfs staging")?;
    }

    std::fs::create_dir_all(&generated_rootfs)
        .context("Failed to create GRUB rootfs staging directory")?;

    copy_recursive(staging, &generated_rootfs)?;

    println!(
        "{} {}",
        "GRUB resources staged in".green(),
        generated_rootfs.display()
    );

    Ok(())
}

fn copy_recursive(
    source: &Path,
    destination: &Path,
) -> Result<()> {
    std::fs::create_dir_all(destination)
        .with_context(|| {
            format!(
                "Failed to create directory {}",
                destination.display()
            )
        })?;

    for entry in std::fs::read_dir(source)
        .with_context(|| {
            format!(
                "Failed to read directory {}",
                source.display()
            )
        })?
    {
        let entry = entry?;

        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());

        if source_path.is_dir() {
            copy_recursive(
                &source_path,
                &destination_path,
            )?;
        } else {
            std::fs::copy(
                &source_path,
                &destination_path,
            )
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