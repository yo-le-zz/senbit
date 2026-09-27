use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::Instant;

use anyhow::{bail, Context, Result};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};

/// Tous les chemins du projet, équivalent des variables $ROOT_DIR, $BUILD_DIR, etc.
/// des scripts shell d'origine.
#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    pub fn resolve(explicit_root: Option<PathBuf>) -> Result<Self> {
        // Ordre de résolution, du plus explicite au plus permissif :
        //   1. --root passé en argument
        //   2. variable d'environnement SENBIT_ROOT
        //   3. le binaire est dans <root>/scripts/senbit  -> on remonte de deux niveaux
        //   4. répertoire courant
        if let Some(r) = explicit_root {
            return Ok(Self { root: r.canonicalize().unwrap_or(r) });
        }

        if let Ok(r) = std::env::var("SENBIT_ROOT") {
            let p = PathBuf::from(r);
            return Ok(Self { root: p.canonicalize().unwrap_or(p) });
        }

        if let Ok(exe) = std::env::current_exe() {
            if let Some(scripts_dir) = exe.parent() {
                if scripts_dir.file_name().and_then(|s| s.to_str()) == Some("scripts") {
                    if let Some(root) = scripts_dir.parent() {
                        return Ok(Self { root: root.to_path_buf() });
                    }
                }
            }
        }

        let cwd = std::env::current_dir().context("failed to read current directory")?;
        Ok(Self { root: cwd })
    }

    pub fn join(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
}

pub fn banner(title: &str) {
    let line = "=".repeat(40);
    println!();
    println!("{}", line.bright_blue());
    println!("{}", format!("  {title}").bright_blue().bold());
    println!("{}", line.bright_blue());
}

pub fn section(title: &str) {
    let line = "-".repeat(40);
    println!();
    println!("{}", line.dimmed());
    println!("{}", format!("  {title}").cyan().bold());
    println!("{}", line.dimmed());
}

pub fn info(msg: impl AsRef<str>) {
    println!("{} {}", "==>".green().bold(), msg.as_ref());
}

pub fn detail(msg: impl AsRef<str>) {
    println!("    {}", msg.as_ref());
}

pub fn warn(msg: impl AsRef<str>) {
    println!("{} {}", "Warning:".yellow().bold(), msg.as_ref());
}

pub fn error(msg: impl AsRef<str>) {
    eprintln!("{} {}", "Error:".red().bold(), msg.as_ref());
}

pub fn ok(msg: impl AsRef<str>) {
    println!("{} {}", "==>".green().bold(), msg.as_ref().green());
}

pub fn format_time(elapsed_secs: u64) -> String {
    format!("{:02}:{:02}", elapsed_secs / 60, elapsed_secs % 60)
}

/// Équivalent de run_step() dans build.sh / getbusy.sh : encadre une étape,
/// affiche un spinner pendant qu'elle tourne et son temps d'exécution à la fin.
pub fn run_step<F>(name: &str, f: F) -> Result<()>
where
    F: FnOnce() -> Result<()>,
{
    banner(name);

    let start = Instant::now();
    let result = f();
    let elapsed = start.elapsed().as_secs();

    match &result {
        Ok(()) => {
            println!();
            ok(format!("{name} completed in {}", format_time(elapsed)));
        }
        Err(e) => {
            println!();
            error(format!("{name} failed after {}: {e}", format_time(elapsed)));
        }
    }

    result
}

/// Spinner générique pour habiller une commande longue (compilation, clonage...).
pub fn spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(90));
    pb
}

/// Exécute une commande en affichant sa sortie en direct (équivalent d'un appel
/// direct dans bash), en s'assurant qu'elle réussit.
pub fn run(cmd: &mut Command) -> Result<()> {
    let program = format!("{:?}", cmd);
    let status: ExitStatus = cmd
        .status()
        .with_context(|| format!("failed to spawn: {program}"))?;

    if !status.success() {
        bail!("command exited with status {status}: {program}");
    }
    Ok(())
}

/// Comme `run`, mais capture stdout (utile pour git describe, git ls-remote...).
pub fn capture(cmd: &mut Command) -> Result<String> {
    let output = cmd
        .stderr(Stdio::inherit())
        .output()
        .with_context(|| format!("failed to spawn: {:?}", cmd))?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Comme `capture`, mais ne remonte jamais d'erreur (équivalent de `|| true` en bash).
pub fn capture_or_empty(cmd: &mut Command) -> String {
    match cmd.stderr(Stdio::null()).output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => String::new(),
    }
}

pub fn nproc() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

pub fn read_version_file(path: &Path) -> Result<String> {
    if !path.is_file() {
        bail!("version file not found:\n  {}", path.display());
    }
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let version: String = content.chars().filter(|c| !c.is_whitespace()).collect();
    if version.is_empty() {
        bail!("version file is empty: {}", path.display());
    }
    Ok(version)
}

// --------------------------------------------------------------------------
// Cache d'état : ne recompile un composant que si sa version / ses sources
// ont réellement changé depuis la dernière build réussie.
// --------------------------------------------------------------------------

#[derive(Debug, Default, Serialize, Deserialize)]
struct StateFile {
    #[serde(flatten)]
    entries: BTreeMap<String, String>,
}

pub struct StateCache {
    path: PathBuf,
    state: StateFile,
}

impl StateCache {
    pub fn open(paths: &Paths) -> Result<Self> {
        let dir = paths.join("build/.state");
        fs::create_dir_all(&dir).context("failed to create build/.state")?;
        let path = dir.join("cache.json");

        let state = if path.is_file() {
            let content = fs::read_to_string(&path).unwrap_or_default();
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            StateFile::default()
        };

        Ok(Self { path, state })
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.state.entries.get(key).map(|s| s.as_str())
    }

    /// Retourne true si `fingerprint` correspond à ce qui est déjà enregistré
    /// pour `key` ET que `output` existe déjà sur le disque : dans ce cas
    /// l'étape peut être sautée, aucune recompilation n'est nécessaire.
    pub fn is_up_to_date(&self, key: &str, fingerprint: &str, output: &Path) -> bool {
        output.exists() && self.get(key) == Some(fingerprint)
    }

    pub fn mark_built(&mut self, key: &str, fingerprint: &str) -> Result<()> {
        self.state
            .entries
            .insert(key.to_string(), fingerprint.to_string());
        let content = serde_json::to_string_pretty(&self.state)?;
        fs::write(&self.path, content)
            .with_context(|| format!("failed to write {}", self.path.display()))?;
        Ok(())
    }
}

/// Empreinte "légère" d'un arbre de sources : combine le hash git HEAD (si
/// dépôt git) avec le mtime le plus récent parmi les fichiers, pour détecter
/// tout changement (nouveau commit, patch appliqué à la main, etc.) sans
/// avoir à hasher l'intégralité des sources à chaque run.
pub fn tree_fingerprint(dir: &Path) -> String {
    let git_rev = capture_or_empty(
        Command::new("git")
            .args(["-C", &dir.to_string_lossy(), "rev-parse", "HEAD"]),
    );

    let dirty = capture_or_empty(
        Command::new("git")
            .args(["-C", &dir.to_string_lossy(), "status", "--porcelain"]),
    );

    let mut newest: u64 = 0;
    if let Ok(walker) = walkdir::WalkDir::new(dir)
        .max_depth(6)
        .into_iter()
        .collect::<std::result::Result<Vec<_>, _>>()
    {
        for entry in walker {
            if entry.file_type().is_file() {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if let Ok(dur) = modified.duration_since(std::time::UNIX_EPOCH) {
                            newest = newest.max(dur.as_secs());
                        }
                    }
                }
            }
        }
    }

    format!("git={git_rev};dirty={};mtime={newest}", !dirty.is_empty())
}

pub fn require_git_repo(dir: &Path, hint: &str) -> Result<()> {
    if !dir.join(".git").exists() {
        error(format!("source tree not found:\n  {}", dir.display()));
        println!();
        println!("Run:");
        println!("  {hint}");
        std::process::exit(1);
    }
    Ok(())
}

pub fn describe_tags(dir: &Path) -> String {
    capture_or_empty(
        Command::new("git")
            .args(["-C", &dir.to_string_lossy(), "describe", "--tags", "--always"]),
    )
}

pub fn describe_tags_exact(dir: &Path) -> String {
    capture_or_empty(Command::new("git").args([
        "-C",
        &dir.to_string_lossy(),
        "describe",
        "--tags",
        "--exact-match",
    ]))
}

pub fn git_status_porcelain(dir: &Path) -> String {
    capture_or_empty(
        Command::new("git")
            .args(["-C", &dir.to_string_lossy(), "status", "--porcelain"]),
    )
}


