use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct Config {
    repository: Repository,
    keymaps: Vec<Keymap>,
}

#[derive(Debug, Deserialize)]
struct Repository {
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct Keymap {
    name: String,
    path: String,
}

fn download_url(base_url: &str, path: &str) -> Result<Option<Vec<u8>>> {
    let url = format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    );

    let response = reqwest::blocking::get(&url)
        .with_context(|| format!("Impossible de télécharger {}", url))?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let data = response
        .bytes()
        .with_context(|| format!("Impossible de lire {}", url))?;

    Ok(Some(data.to_vec()))
}

fn global_base_url(base_url: &str) -> String {
    base_url
        .trim_end_matches('/')
        .strip_suffix("/i386")
        .unwrap_or(base_url)
        .to_string()
}

fn include_candidates(current_path: &str, include: &str) -> Vec<String> {
    let mut candidates = Vec::new();

    let current_dir = Path::new(current_path)
        .parent()
        .unwrap_or_else(|| Path::new(""));

    if !current_dir.as_os_str().is_empty() {
        candidates.push(
            current_dir
                .join(include)
                .to_string_lossy()
                .to_string(),
        );
    }

    candidates.push(format!("include/{}", include));

    if include.ends_with(".inc") || include.ends_with(".map") {
        candidates.push(format!("i386/include/{}", include));
    } else {
        candidates.push(format!("include/{}.inc", include));
        candidates.push(format!("i386/include/{}.inc", include));
        candidates.push(format!("i386/include/{}", include));
    }

    candidates.push(include.to_string());

    candidates
}

fn is_map_alias(content: &[u8]) -> Option<String> {
    let content = String::from_utf8_lossy(content);
    let content = content.trim();

    if content.is_empty() {
        return None;
    }

    let mut lines = content.lines();

    let line = lines.next()?.trim();

    if lines.next().is_some() {
        return None;
    }

    if line.starts_with('#') || line.starts_with("//") {
        return None;
    }

    if !line.ends_with(".map") {
        return None;
    }

    if !line
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'))
    {
        return None;
    }

    Some(line.to_string())
}

fn resolve_alias_path(current_path: &str, alias: &str) -> String {
    let current_dir = Path::new(current_path)
        .parent()
        .unwrap_or_else(|| Path::new(""));

    current_dir.join(alias).to_string_lossy().to_string()
}

fn download_file(
    base_url: &str,
    global_url: &str,
    path: &str,
    output_path: &Path,
    downloaded: &mut HashSet<String>,
) -> Result<Vec<u8>> {
    let output = output_path.join(path);

    // Déjà traité pendant cette exécution.
    if downloaded.contains(path) {
        return fs::read(&output)
            .with_context(|| format!("Impossible de lire {}", output.display()));
    }

    // Déjà présent sur le disque : pas de téléchargement.
    if output.exists() {
        println!("Already downloaded: {}", path);

        let data = fs::read(&output)
            .with_context(|| format!("Impossible de lire {}", output.display()))?;

        downloaded.insert(path.to_string());

        return Ok(data);
    }

    println!("Downloading {}...", path);

    let data = download_url(base_url, path)?
        .or_else(|| {
            if path.starts_with("include/") {
                download_url(global_url, path).ok().flatten()
            } else {
                None
            }
        })
        .ok_or_else(|| anyhow::anyhow!("Échec du téléchargement du fichier '{}'", path))?;

    downloaded.insert(path.to_string());

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(&output, &data)?;

    if let Some(alias) = is_map_alias(&data) {
        let target = resolve_alias_path(path, &alias);

        println!("Resolving keymap alias {} -> {}", path, target);

        let target_data = download_file(
            base_url,
            global_url,
            &target,
            output_path,
            downloaded,
        )?;

        fs::write(&output, &target_data)?;

        return Ok(target_data);
    }

    let content = String::from_utf8_lossy(&data);

    for line in content.lines() {
        let line = line.trim();

        if !line.starts_with("include ") {
            continue;
        }

        let include = line
            .strip_prefix("include ")
            .unwrap()
            .trim()
            .trim_matches('"')
            .trim_matches('\'');

        if include.is_empty() {
            continue;
        }

        let candidates = include_candidates(path, include);

        let mut found = false;

        for candidate in candidates {
            if downloaded.contains(&candidate) {
                found = true;
                break;
            }

            let candidate_output = output_path.join(&candidate);

            // Include déjà présent : pas de requête réseau.
            if candidate_output.exists() {
                println!("Already downloaded: {}", candidate);

                downloaded.insert(candidate.clone());
                found = true;
                break;
            }

            let data = if candidate.starts_with("include/") {
                download_url(global_url, &candidate)?
                    .or_else(|| download_url(base_url, &candidate).ok().flatten())
            } else {
                download_url(base_url, &candidate)?
            };

            if data.is_some() {
                download_file(
                    base_url,
                    global_url,
                    &candidate,
                    output_path,
                    downloaded,
                )?;

                found = true;
                break;
            }
        }

        if !found {
            anyhow::bail!(
                "Impossible de résoudre l'include '{}' utilisé par '{}'",
                include,
                path
            );
        }
    }

    Ok(data)
}

pub fn download_all(config_path: &str, output_path: &str) -> Result<()> {
    let content =
        fs::read_to_string(config_path).context("Impossible de lire config.toml")?;

    let config: Config =
        toml::from_str(&content).context("Impossible de parser config.toml")?;

    fs::create_dir_all(output_path)?;

    let mut downloaded = HashSet::new();
    let global_url = global_base_url(&config.repository.base_url);

    for keymap in &config.keymaps {
        download_file(
            &config.repository.base_url,
            &global_url,
            &keymap.path,
            Path::new(output_path),
            &mut downloaded,
        )?;

        let source = Path::new(output_path).join(&keymap.path);

        let destination = PathBuf::from(output_path)
            .join(format!("{}.map", keymap.name));

        if source != destination && !destination.exists() {
            fs::copy(source, destination)?;
        }
    }

    Ok(())
}