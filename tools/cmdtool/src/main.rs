use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const COMMANDS_DIR: &str =
    "tools/cmdtool/src/commands";

const COMMANDS_OUTPUT_DIR: &str =
    "build/tools/generated/commands";

/// Resolve the Senbit project root from the cmdtool executable.
///
/// Expected layout:
///
/// Senbit/
/// ├── tools/
/// │   └── cmdtool/
/// │       └── target/
/// │           └── release/
/// │               └── senbit-cmdtool
/// └── commands/
///
/// The project root is therefore five parents above the executable:
///
/// senbit-cmdtool
/// release
/// target
/// cmdtool
/// tools
/// Senbit
fn project_root() -> Result<PathBuf> {
    let executable =
        std::env::current_exe()
            .context(
                "Failed to get cmdtool executable path",
            )?;

    let executable =
        executable.canonicalize()
            .with_context(|| {
                format!(
                    "Failed to canonicalize executable: {}",
                    executable.display()
                )
            })?;

    let release_dir =
        executable.parent()
            .context(
                "Failed to get release directory",
            )?;

    let target_dir =
        release_dir.parent()
            .context(
                "Failed to get target directory",
            )?;

    let cmdtool_dir =
        target_dir.parent()
            .context(
                "Failed to get cmdtool directory",
            )?;

    let tools_dir =
        cmdtool_dir.parent()
            .context(
                "Failed to get tools directory",
            )?;

    let project_root =
        tools_dir.parent()
            .context(
                "Failed to get Senbit project root",
            )?;

    Ok(
        project_root.to_path_buf()
    )
}

/// Find every command crate inside the top-level
/// `commands/` directory.
///
/// Each command must contain its own Cargo.toml.
fn find_commands(
    commands_dir: &Path,
) -> Result<Vec<PathBuf>> {
    let mut commands = Vec::new();

    if !commands_dir.is_dir() {
        bail!(
            "Commands directory does not exist: {}",
            commands_dir.display()
        );
    }

    for entry in fs::read_dir(commands_dir)
        .with_context(|| {
            format!(
                "Failed to read commands directory: {}",
                commands_dir.display()
            )
        })?
    {
        let entry = entry?;

        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let manifest =
            path.join("Cargo.toml");

        if manifest.is_file() {
            commands.push(manifest);
        }
    }

    commands.sort();

    Ok(commands)
}

/// Read the Senbit installation directory from Cargo.toml.
///
/// Expected:
///
/// [package.metadata.senbit]
/// install = "bin"
///
/// or:
///
/// install = "sbin"
fn get_install_directory(
    manifest: &Path,
) -> Result<&'static str> {
    let content =
        fs::read_to_string(manifest)
            .with_context(|| {
                format!(
                    "Failed to read {}",
                    manifest.display()
                )
            })?;

    let mut in_senbit_metadata = false;

    for line in content.lines() {
        let line = line.trim();

        if line == "[package.metadata.senbit]" {
            in_senbit_metadata = true;
            continue;
        }

        if line.starts_with('[') {
            in_senbit_metadata = false;
        }

        if !in_senbit_metadata {
            continue;
        }

        if let Some(value) =
            line.strip_prefix("install =")
        {
            let value = value
                .trim()
                .trim_matches('"');

            return match value {
                "bin" => Ok("bin"),
                "sbin" => Ok("sbin"),

                _ => bail!(
                    "Invalid install directory '{}' in {}. \
                     Expected 'bin' or 'sbin'.",
                    value,
                    manifest.display()
                ),
            };
        }
    }

    bail!(
        "Missing [package.metadata.senbit] install field in {}",
        manifest.display()
    );
}

/// Determine the Cargo package name.
fn package_name(
    manifest: &Path,
) -> Result<String> {
    let content =
        fs::read_to_string(manifest)
            .with_context(|| {
                format!(
                    "Failed to read {}",
                    manifest.display()
                )
            })?;

    let mut in_package_section = false;

    for line in content.lines() {
        let line = line.trim();

        if line == "[package]" {
            in_package_section = true;
            continue;
        }

        if line.starts_with('[') {
            in_package_section = false;
        }

        if !in_package_section {
            continue;
        }

        if let Some(value) =
            line.strip_prefix("name =")
        {
            return Ok(
                value
                    .trim()
                    .trim_matches('"')
                    .to_string()
            );
        }
    }

    bail!(
        "Unable to determine package name from {}",
        manifest.display()
    );
}

/// Build one command crate and return its executable.
fn build_command(
    manifest: &Path,
) -> Result<PathBuf> {
    let command_dir =
        manifest.parent()
            .context(
                "Invalid command manifest path",
            )?;

    let command_name =
        package_name(manifest)?;

    println!(
        "Building command '{}'...",
        command_name
    );

    let status =
        Command::new("cargo")
            .arg("build")
            .arg("--release")
            .arg("--target")
            .arg("x86_64-unknown-linux-musl")
            .arg("--manifest-path")
            .arg(manifest)
            .current_dir(command_dir)
            .status()
            .with_context(|| {
                format!(
                    "Failed to execute Cargo for command '{}'",
                    command_name
                )
            })?;

    if !status.success() {
        bail!(
            "Failed to build command '{}'",
            command_name
        );
    }

    let binary =
        command_dir
            .join("target")
            .join("x86_64-unknown-linux-musl")
            .join("release")
            .join(&command_name);

    if !binary.is_file() {
        bail!(
            "Built executable not found for '{}': {}",
            command_name,
            binary.display()
        );
    }

    Ok(binary)
}

/// Install a compiled command into the generated
/// command tree.
fn install_command(
    binary: &Path,
    manifest: &Path,
    output_dir: &Path,
) -> Result<()> {
    let install_dir =
        get_install_directory(manifest)?;

    let destination_dir =
        output_dir.join(install_dir);

    fs::create_dir_all(
        &destination_dir,
    )
    .with_context(|| {
        format!(
            "Failed to create {}",
            destination_dir.display()
        )
    })?;

    let command_name =
        binary.file_name()
            .context(
                "Invalid binary filename",
            )?;

    let destination =
        destination_dir.join(command_name);

    println!(
        "Installing {} -> {}",
        command_name.to_string_lossy(),
        destination.display()
    );

    if fs::symlink_metadata(
        &destination,
    )
    .is_ok()
    {
        fs::remove_file(
            &destination,
        )
        .with_context(|| {
            format!(
                "Failed to remove old command {}",
                destination.display()
            )
        })?;
    }

    fs::copy(
        binary,
        &destination,
    )
    .with_context(|| {
        format!(
            "Failed to install {}",
            binary.display()
        )
    })?;

    Ok(())
}

fn main() -> Result<()> {
    println!("Senbit command builder");
    println!();

    let project_root =
        project_root()?;

    println!(
        "Project root: {}",
        project_root.display()
    );

    let commands_dir =
        project_root.join(
            COMMANDS_DIR,
        );

    let output_dir =
        project_root.join(
            COMMANDS_OUTPUT_DIR,
        );

    println!(
        "Commands directory: {}",
        commands_dir.display()
    );

    println!(
        "Output directory: {}",
        output_dir.display()
    );

    println!();

    if output_dir.exists() {
        fs::remove_dir_all(
            &output_dir,
        )
        .with_context(|| {
            format!(
                "Failed to clean {}",
                output_dir.display()
            )
        })?;
    }

    fs::create_dir_all(
        &output_dir,
    )
    .with_context(|| {
        format!(
            "Failed to create {}",
            output_dir.display()
        )
    })?;

    let manifests =
        find_commands(
            &commands_dir,
        )?;

    if manifests.is_empty() {
        println!(
            "No commands found."
        );

        return Ok(());
    }

    println!(
        "Found {} command(s).",
        manifests.len()
    );

    println!();

    let mut built = 0usize;

    for manifest in manifests {
        let binary =
            build_command(
                &manifest,
            )?;

        install_command(
            &binary,
            &manifest,
            &output_dir,
        )?;

        built += 1;

        println!();
    }

    println!(
        "Successfully built and staged {} command(s).",
        built
    );

    Ok(())
}