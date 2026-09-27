use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

#[derive(Debug, Deserialize)]
struct Config {
    keymaps: Vec<Keymap>,
}

#[derive(Debug, Deserialize)]
struct Keymap {
    name: String,
    path: String,
}

const KERNEL_DEFKEYMAP: &str =
    "kernel/linux/drivers/tty/vt/defkeymap.map";

pub fn compile(config_path: &str, output_path: &str) -> Result<()> {
    let content = fs::read_to_string(config_path)
        .context("Impossible de lire config.toml")?;

    let config: Config = toml::from_str(&content)
        .context("Impossible de parser config.toml")?;

    let root = std::env::current_dir()
        .context("Impossible de déterminer la racine du projet")?;

    let output_root = root.join(output_path);

    fs::create_dir_all(&output_root)?;

    let kernel_defkeymap = root.join(KERNEL_DEFKEYMAP);
    let local_defkeymap = output_root.join("defkeymap.map");

    if !kernel_defkeymap.exists() {
        bail!(
            "defkeymap.map introuvable: {}",
            kernel_defkeymap.display()
        );
    }

    if !local_defkeymap.exists() {
        fs::copy(&kernel_defkeymap, &local_defkeymap)?;
    }

    let loadkeys = find_loadkeys()?;

    println!("Using loadkeys: {}", loadkeys.display());

    for keymap in config.keymaps {
        let input = output_root.join(&keymap.path);
        let output = output_root.join(format!("{}.bmap", keymap.name));

        if !input.exists() {
            bail!("Keymap introuvable: {}", input.display());
        }

        if is_up_to_date(&input, &output)? {
            println!("Already compiled: {}", keymap.name);
            continue;
        }

        println!("Compiling {}...", keymap.name);

        compile_keymap(
            &loadkeys,
            &input,
            &output,
            &output_root,
        )?;
    }

    Ok(())
}

fn find_loadkeys() -> Result<PathBuf> {
    let root = std::env::current_dir()?;

    let path = root.join("tools/keymaps/kbd-install/bin/loadkeys");

    if !path.exists() {
        bail!(
            "loadkeys Senbit introuvable: {}",
            path.display()
        );
    }

    Ok(path)
}

fn is_up_to_date(input: &Path, output: &Path) -> Result<bool> {
    if !output.exists() {
        return Ok(false);
    }

    let input_time = newest_source_time(input)?;
    let output_time = fs::metadata(output)?
        .modified()
        .context("Impossible de lire la date du fichier bmap")?;

    Ok(output_time >= input_time)
}

fn newest_source_time(path: &Path) -> Result<SystemTime> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("Impossible de lire {}", path.display()))?;

    let mut newest = metadata
        .modified()
        .with_context(|| format!("Impossible de lire la date de {}", path.display()))?;

    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let time = newest_source_time(&entry.path())?;

            if time > newest {
                newest = time;
            }
        }
    }

    Ok(newest)
}

fn compile_keymap(
    loadkeys: &Path,
    input: &Path,
    output: &Path,
    keymap_root: &Path,
) -> Result<()> {
    let input = absolute_path(input)?;
    let output = absolute_path(output)?;
    let keymap_root = absolute_path(keymap_root)?;

    if !input.exists() {
        bail!(
            "Keymap introuvable: {}",
            input.display()
        );
    }

    let result = Command::new(loadkeys)
        .arg("-u")
        .arg("-b")
        .arg(&input)
        .env("LOADKEYS_KEYMAP_PATH", &keymap_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| {
            format!(
                "Impossible d'exécuter {}",
                loadkeys.display()
            )
        })?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);

        bail!(
            "Échec de compilation de {}:\n{}",
            input.display(),
            stderr.trim()
        );
    }

    fs::write(output, result.stdout)?;

    Ok(())
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }

    std::env::current_dir()
        .context("Impossible de déterminer la racine du projet")
        .map(|root| root.join(path))
}