//! Affichage : bannières, couleurs, spinner, chronométrage des étapes.

use std::sync::OnceLock;
use std::time::Instant;

use anyhow::Result;
use colored::Colorize;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

/// All the bars share one MultiProgress: the spinner of a step and the progress
/// line that `proc::run_quiet` draws under it do not overwrite each other.
static MULTI: OnceLock<MultiProgress> = OnceLock::new();

pub fn multi() -> &'static MultiProgress {
    MULTI.get_or_init(MultiProgress::new)
}

pub fn banner(title: &str) {
    let line = "=".repeat(40);
    println!();
    println!("{}", line.cyan());
    println!("{}", format!("  {title}").cyan().bold());
    println!("{}", line.cyan());
}

pub fn section(title: &str) {
    let line = "-".repeat(40);
    println!();
    println!("{}", line.dimmed());
    println!("{}", format!("  {title}").cyan());
    println!("{}", line.dimmed());
}

pub fn info(msg: impl AsRef<str>) {
    println!("{} {}", "==>".green().bold(), msg.as_ref());
}

pub fn detail(msg: impl AsRef<str>) {
    println!("    {}", msg.as_ref());
}

pub fn ok(msg: impl AsRef<str>) {
    println!("{} {}", "==>".green().bold(), msg.as_ref().green());
}

pub fn skip(msg: impl AsRef<str>) {
    println!("{} {}", "==>".yellow().bold(), msg.as_ref().yellow());
}

pub fn warn(msg: impl AsRef<str>) {
    println!("{} {}", "Warning:".yellow().bold(), msg.as_ref());
}

pub fn error(msg: impl AsRef<str>) {
    eprintln!("{} {}", "Error:".red().bold(), msg.as_ref());
}

pub fn format_time(secs: u64) -> String {
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

/// Dimmed line shown under the spinner while a long command runs
/// (percentage, last line of its output, elapsed time).
pub fn progress_line() -> ProgressBar {
    let pb = multi().add(ProgressBar::new_spinner());
    pb.set_style(ProgressStyle::with_template("    {msg}").unwrap());
    pb
}

pub fn spinner(msg: &str) -> ProgressBar {
    let pb = multi().add(ProgressBar::new_spinner());
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ "),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(90));
    pb
}

/// Encadre une étape avec une bannière et affiche son temps d'exécution,
/// équivalent de `run_step()` dans build.sh.
pub fn run_step<F>(name: &str, f: F) -> Result<()>
where
    F: FnOnce() -> Result<()>,
{
    banner(name);
    let start = Instant::now();
    let result = f();
    let secs = start.elapsed().as_secs();
    println!();
    match &result {
        Ok(()) => ok(format!("{name} completed in {}", format_time(secs))),
        // Le détail de l'erreur est affiché une seule fois, par le
        // gestionnaire d'erreurs de main() ; ici on ne signale que l'échec.
        Err(_) => error(format!("{name} failed after {}", format_time(secs))),
    }
    result
}
