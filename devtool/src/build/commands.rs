//! Commandes natives Senbit : un dossier `commands/<nom>/` = une crate Rust.
//!
//! Remplace l'ancien `tools/cmdtool` : plus d'outil intermédiaire, `devtool`
//! détecte lui-même les commandes. Pour en ajouter une :
//!
//! ```text
//! ./newcmds.sh macommande          # crée commands/macommande/ (crate vide)
//! scripts/devtool build            # la détecte, la compile, l'installe
//! ```
//!
//! Chaque crate décrit son installation dans son `Cargo.toml` :
//!
//! ```toml
//! [package.metadata.senbit]
//! install = "bin"                      # "bin" ou "sbin" (défaut : "bin")
//! aliases = ["addgroup", "deluser"]    # liens vers le même binaire (optionnel)
//! setuid  = true                       # bit setuid root (optionnel, ex. sudo)
//! ```
//!
//! Sortie : `build/commands/stage/{bin,sbin}` + `usr/share/senbit/commands.list`,
//! fusionnée dans le rootfs par `build::rootfs`. `commands/INDEX.md` est
//! régénéré à chaque build (liste de toutes les commandes détectées).

use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::build::rust_build::configure_rust_command;
use crate::cache::Cache;
use crate::paths::Paths;
use crate::proc::run_quiet;
use crate::ui;

/// Une commande détectée dans `commands/`.
#[derive(Debug, Clone)]
pub struct Cmd {
    /// Nom du dossier (`commands/<dir_name>/`).
    pub dir_name: String,
    pub dir: PathBuf,
    pub manifest: PathBuf,
    /// Nom du binaire produit par cargo.
    pub bin: String,
    /// "bin" ou "sbin".
    pub install: String,
    pub aliases: Vec<String>,
    pub setuid: bool,
    pub description: String,
}

/// Nom de commande acceptable : lettres, chiffres et `- _ . +`, jamais de `/`
/// ni de tiret/point en tête.
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with(['-', '.'])
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+'))
}

/// Lit le `Cargo.toml` d'une commande.
fn parse_manifest(dir: &Path) -> Result<Cmd> {
    let manifest = dir.join("Cargo.toml");
    let text = fs::read_to_string(&manifest)
        .with_context(|| format!("cannot read {}", manifest.display()))?;
    let doc: toml::Value =
        toml::from_str(&text).with_context(|| format!("invalid {}", manifest.display()))?;

    let package = doc
        .get("package")
        .and_then(|v| v.as_table())
        .with_context(|| format!("{}: missing [package]", manifest.display()))?;

    let package_name = package
        .get("name")
        .and_then(|v| v.as_str())
        .with_context(|| format!("{}: missing package name", manifest.display()))?
        .to_string();

    let description = package
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let senbit = package
        .get("metadata")
        .and_then(|m| m.get("senbit"))
        .and_then(|s| s.as_table());

    let install = senbit
        .and_then(|s| s.get("install"))
        .and_then(|v| v.as_str())
        .unwrap_or("bin")
        .to_string();
    if install != "bin" && install != "sbin" {
        bail!(
            "{}: invalid install = \"{install}\" (expected \"bin\" or \"sbin\")",
            manifest.display()
        );
    }

    let aliases: Vec<String> = senbit
        .and_then(|s| s.get("aliases"))
        .and_then(|v| v.as_array())
        .map(|list| {
            list.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    let setuid = senbit
        .and_then(|s| s.get("setuid"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Nom du binaire : premier [[bin]] s'il existe, sinon le nom du package.
    let bin = doc
        .get("bin")
        .and_then(|b| b.as_array())
        .and_then(|a| a.first())
        .and_then(|b| b.get("name"))
        .and_then(|n| n.as_str())
        .map(str::to_string)
        .unwrap_or(package_name);

    for name in std::iter::once(&bin).chain(aliases.iter()) {
        if !valid_name(name) {
            bail!(
                "{}: invalid command name `{name}` (letters, digits and - _ . + only)",
                manifest.display()
            );
        }
    }

    let dir_name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    Ok(Cmd {
        dir_name,
        dir: dir.to_path_buf(),
        manifest,
        bin,
        install,
        aliases,
        setuid,
        description,
    })
}

/// Toutes les commandes de `commands/` (tri alphabétique). Les dossiers dont
/// le nom commence par `_` ou `.` et ceux sans `Cargo.toml` sont ignorés
/// (ex. `commands/_shared/`).
pub fn discover(dir: &Path) -> Result<Vec<Cmd>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut dirs: Vec<PathBuf> = fs::read_dir(dir)
        .with_context(|| format!("cannot read {}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|d| d.is_dir() && d.join("Cargo.toml").is_file())
        .filter(|d| {
            d.file_name()
                .map(|n| {
                    let n = n.to_string_lossy();
                    !n.starts_with('_') && !n.starts_with('.')
                })
                .unwrap_or(false)
        })
        .collect();
    dirs.sort();

    let mut cmds = Vec::new();
    let mut seen: BTreeMap<String, String> = BTreeMap::new();

    for d in dirs {
        let cmd = parse_manifest(&d)?;
        for name in std::iter::once(&cmd.bin).chain(cmd.aliases.iter()) {
            if let Some(owner) = seen.insert(name.clone(), cmd.dir_name.clone()) {
                bail!(
                    "command name `{name}` is provided by both `{owner}` and `{}`",
                    cmd.dir_name
                );
            }
        }
        cmds.push(cmd);
    }

    Ok(cmds)
}

/// Empreinte du contenu d'une crate (noms + contenu des fichiers, hors
/// `target/`) : une commande n'est recompilée que si ses fichiers changent.
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

fn binary_path(p: &Paths, cmd: &Cmd) -> PathBuf {
    p.commands_target_dir()
        .join(p.rust_target())
        .join("release")
        .join(&cmd.bin)
}

/// Contenu de `commands/INDEX.md`.
fn render_index(cmds: &[Cmd]) -> String {
    let mut out = String::new();
    out.push_str("# Senbit commands\n\n");
    out.push_str(
        "Generated by `devtool` (do not edit by hand): every folder of `commands/` that holds\n\
         a `Cargo.toml` is a command. Add one with `./newcmds.sh <name>`.\n\n",
    );
    out.push_str("| Command | Folder | Installed in | Aliases | Description |\n");
    out.push_str("|---|---|---|---|---|\n");
    for c in cmds {
        let aliases = if c.aliases.is_empty() {
            "-".to_string()
        } else {
            c.aliases
                .iter()
                .map(|a| format!("`{a}`"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let description = if c.description.is_empty() {
            "-"
        } else {
            c.description.as_str()
        };
        let setuid = if c.setuid { " (setuid)" } else { "" };
        out.push_str(&format!(
            "| `{}` | `commands/{}` | `/{}`{} | {} | {} |\n",
            c.bin, c.dir_name, c.install, setuid, aliases, description
        ));
    }
    out
}

/// Écrit `commands/INDEX.md` seulement s'il a changé.
fn sync_index(dir: &Path, cmds: &[Cmd]) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    let index = dir.join("INDEX.md");
    let wanted = render_index(cmds);
    if fs::read_to_string(&index).ok().as_deref() != Some(wanted.as_str()) {
        fs::write(&index, wanted).with_context(|| format!("cannot write {}", index.display()))?;
        ui::detail(format!("{} updated.", index.display()));
    }
    Ok(())
}

/// Prépare `build/commands/stage` à partir des binaires compilés.
fn stage(p: &Paths, cmds: &[Cmd]) -> Result<()> {
    let stage = p.commands_stage_dir();
    let _ = fs::remove_dir_all(&stage);
    fs::create_dir_all(stage.join("bin"))?;
    fs::create_dir_all(stage.join("sbin"))?;

    let mut list = String::new();

    for cmd in cmds {
        let binary = binary_path(p, cmd);
        let dest_dir = stage.join(&cmd.install);
        let dest = dest_dir.join(&cmd.bin);

        fs::copy(&binary, &dest)
            .with_context(|| format!("cannot stage {}", binary.display()))?;
        let mode = if cmd.setuid { 0o4755 } else { 0o755 };
        fs::set_permissions(&dest, fs::Permissions::from_mode(mode))?;

        // Alias = lien relatif dans le même dossier (le binaire se reconnaît
        // à argv[0]).
        for alias in &cmd.aliases {
            let link = dest_dir.join(alias);
            if fs::symlink_metadata(&link).is_ok() {
                fs::remove_file(&link)?;
            }
            symlink(&cmd.bin, &link)
                .with_context(|| format!("cannot create alias {}", link.display()))?;
        }

        list.push_str(&format!(
            "{} {} {}\n",
            cmd.bin,
            cmd.install,
            if cmd.aliases.is_empty() {
                "-".to_string()
            } else {
                cmd.aliases.join(",")
            }
        ));
    }

    let share = stage.join("usr/share/senbit");
    fs::create_dir_all(&share)?;
    fs::write(share.join("commands.list"), list)?;

    Ok(())
}

/// `devtool build commands` : détecte, compile et prépare toutes les
/// commandes de `commands/`.
pub fn build(p: &Paths, cache: &mut Cache, jobs: usize) -> Result<()> {
    let dir = p.commands_dir();
    ui::info(format!("Searching for Senbit commands in {}...", dir.display()));

    let cmds = discover(&dir)?;
    if cmds.is_empty() {
        ui::warn("No command found: the system will have no Senbit command (sudo, reboot...).");
    }
    sync_index(&dir, &cmds)?;

    let target_dir = p.commands_target_dir();
    fs::create_dir_all(&target_dir)?;

    for cmd in &cmds {
        let binary = binary_path(p, cmd);

        let mut hasher = DefaultHasher::new();
        hash_tree(&cmd.dir, &mut hasher)?;
        let fp = format!("command:{:016x}:{}", hasher.finish(), p.rust_target());
        let key = format!("command:{}", cmd.dir_name);

        if cache.is_fresh(&key, &fp, &[binary.as_path()]) {
            ui::skip(format!("{}: unchanged - skipping compilation.", cmd.dir_name));
            continue;
        }

        let mut cargo = Command::new("cargo");
        cargo
            .current_dir(&cmd.dir)
            .env("CARGO_TARGET_DIR", &target_dir)
            // Binaires petits : ils vivent dans l'initramfs.
            .env("CARGO_PROFILE_RELEASE_OPT_LEVEL", "s")
            .env("CARGO_PROFILE_RELEASE_STRIP", "true")
            .args(["build", "--release", "--target", p.rust_target(), "-j"])
            .arg(jobs.to_string())
            .arg("--manifest-path")
            .arg(&cmd.manifest);
        configure_rust_command(&mut cargo, p.rust_target())?;

        let spin = ui::spinner(&format!("Compiling command '{}'...", cmd.dir_name));
        let result = run_quiet(&mut cargo);
        spin.finish_and_clear();
        result.with_context(|| format!("command '{}' failed to build", cmd.dir_name))?;

        if !binary.is_file() {
            bail!(
                "command '{}': built executable not found:\n  {}\n\n\
                 The binary must be named `{}` (package name, or the name of the first [[bin]]).",
                cmd.dir_name,
                binary.display(),
                cmd.bin
            );
        }

        cache.record(&key, &fp)?;
        ui::detail(format!("{} -> /{}/{}", cmd.dir_name, cmd.install, cmd.bin));
    }

    stage(p, &cmds)?;

    ui::ok(format!("{} Senbit command(s) staged.", cmds.len()));
    Ok(())
}

/// `devtool commands list`
pub fn list(p: &Paths) -> Result<()> {
    let cmds = discover(&p.commands_dir())?;
    if cmds.is_empty() {
        println!("No command found in {}.", p.commands_dir().display());
        return Ok(());
    }
    println!("{:<18} {:<6} {}", "COMMAND", "DIR", "ALIASES");
    for c in &cmds {
        println!(
            "{:<18} {:<6} {}",
            c.bin,
            c.install,
            if c.aliases.is_empty() {
                "-".to_string()
            } else {
                c.aliases.join(", ")
            }
        );
    }
    println!();
    println!("{} command(s). Add one with ./newcmds.sh <name>.", cmds.len());
    Ok(())
}

/// `devtool commands index` : régénère `commands/INDEX.md` sans compiler.
pub fn index(p: &Paths) -> Result<()> {
    let dir = p.commands_dir();
    let cmds = discover(&dir)?;
    sync_index(&dir, &cmds)?;
    ui::ok(format!(
        "{} command(s) indexed in {}.",
        cmds.len(),
        dir.join("INDEX.md").display()
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("devtool-cmds-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_cmd(root: &Path, dir: &str, manifest: &str) {
        let d = root.join(dir);
        fs::create_dir_all(d.join("src")).unwrap();
        fs::write(d.join("Cargo.toml"), manifest).unwrap();
        fs::write(d.join("src/main.rs"), "fn main() {}\n").unwrap();
    }

    #[test]
    fn names_are_validated() {
        assert!(valid_name("boot-report"));
        assert!(valid_name("g++"));
        assert!(!valid_name(""));
        assert!(!valid_name("-x"));
        assert!(!valid_name("a/b"));
        assert!(!valid_name("a b"));
    }

    #[test]
    fn detects_commands_metadata_and_aliases() {
        let root = temp("detect");
        write_cmd(
            &root,
            "adduser",
            "[package]\nname = \"adduser\"\nversion = \"0.1.0\"\n\n\
             [package.metadata.senbit]\ninstall = \"sbin\"\naliases = [\"addgroup\", \"deluser\"]\n",
        );
        write_cmd(
            &root,
            "log",
            "[package]\nname = \"log\"\nversion = \"0.1.0\"\n\n[[bin]]\nname = \"log\"\npath = \"src/main.rs\"\n",
        );
        write_cmd(
            &root,
            "sudo",
            "[package]\nname = \"sudo\"\nversion = \"0.1.0\"\n\n\
             [package.metadata.senbit]\ninstall = \"bin\"\nsetuid = true\n",
        );
        // Ignorés : dossier `_shared`, dossier sans Cargo.toml.
        write_cmd(&root, "_shared", "[package]\nname = \"x\"\nversion = \"0.1.0\"\n");
        fs::create_dir_all(root.join("empty")).unwrap();

        let cmds = discover(&root).unwrap();
        let names: Vec<&str> = cmds.iter().map(|c| c.bin.as_str()).collect();
        assert_eq!(names, vec!["adduser", "log", "sudo"]);

        assert_eq!(cmds[0].install, "sbin");
        assert_eq!(cmds[0].aliases, vec!["addgroup", "deluser"]);
        // install absent : "bin" par défaut.
        assert_eq!(cmds[1].install, "bin");
        assert!(cmds[2].setuid);
        assert!(!cmds[0].setuid);

        let index = render_index(&cmds);
        assert!(index.contains("`addgroup`"));
        assert!(index.contains("(setuid)"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn duplicate_names_are_rejected() {
        let root = temp("dup");
        write_cmd(
            &root,
            "a",
            "[package]\nname = \"a\"\nversion = \"0.1.0\"\n\n\
             [package.metadata.senbit]\naliases = [\"b\"]\n",
        );
        write_cmd(&root, "b", "[package]\nname = \"b\"\nversion = \"0.1.0\"\n");
        assert!(discover(&root).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn invalid_install_is_rejected() {
        let root = temp("install");
        write_cmd(
            &root,
            "a",
            "[package]\nname = \"a\"\nversion = \"0.1.0\"\n\n\
             [package.metadata.senbit]\ninstall = \"usr\"\n",
        );
        assert!(discover(&root).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_directory_gives_no_command() {
        assert!(discover(Path::new("/nonexistent/senbit/commands"))
            .unwrap()
            .is_empty());
    }
}
