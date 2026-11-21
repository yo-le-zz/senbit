//! Exécution de commandes externes.

use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

fn describe(cmd: &Command) -> String {
    let mut s = cmd.get_program().to_string_lossy().to_string();
    for a in cmd.get_args() {
        s.push(' ');
        s.push_str(&a.to_string_lossy());
    }
    s
}

/// stdin/stdout/stderr branchés au terminal (équivalent d'un appel direct en bash).
pub fn run(cmd: &mut Command) -> Result<()> {
    let status = cmd
        .status()
        .with_context(|| format!("failed to run `{}`", describe(cmd)))?;
    if !status.success() {
        bail!("`{}` exited with {status}", describe(cmd));
    }
    Ok(())
}

/// Sortie standard (trim), échoue si la commande échoue.
pub fn capture(cmd: &mut Command) -> Result<String> {
    let out = cmd
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .output()
        .with_context(|| format!("failed to run `{}`", describe(cmd)))?;
    if !out.status.success() {
        bail!("`{}` exited with {}", describe(cmd), out.status);
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Comme `capture`, mais ne remonte jamais d'erreur (équivalent du `|| true`
/// des scripts pour `git describe --tags --always`).
pub fn capture_or_empty(cmd: &mut Command) -> String {
    cmd.stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

/// Vrai si la commande réussit, sortie masquée (pour les `git apply --check`).
pub fn succeeds(cmd: &mut Command) -> bool {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn nproc() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// Nombre maximal de lignes gardées en mémoire pour le journal d'un échec.
const MAX_LOG_LINES: usize = 500_000;

/// Avancement lu dans une ligne de sortie : `[123/456] ...` (ninja, meson),
/// `[ 45%] ...` (cmake). Renvoie (fait, total).
pub fn progress_hint(line: &str) -> Option<(usize, usize)> {
    let rest = line.trim_start().strip_prefix('[')?;
    let end = rest.find(']')?;
    let inner = rest[..end].trim();

    if let Some((done, total)) = inner.split_once('/') {
        let done: usize = done.trim().parse().ok()?;
        let total: usize = total.trim().parse().ok()?;
        return Some((done, total));
    }
    if let Some(percent) = inner.strip_suffix('%') {
        let percent: usize = percent.trim().parse().ok()?;
        return Some((percent, 100));
    }
    None
}

fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    let cut: String = text.chars().take(width.saturating_sub(1)).collect();
    format!("{cut}…")
}

/// « 27% (123/456) 01:32  Compiling C object foo.o »
/// ou « 1523 lines 00:42  CC drivers/net/foo.o » quand l'outil ne donne pas de total.
pub fn status_line(
    elapsed_secs: u64,
    lines: usize,
    progress: Option<(usize, usize)>,
    last: &str,
    width: usize,
) -> String {
    let time = format!("{:02}:{:02}", elapsed_secs / 60, elapsed_secs % 60);
    let head = match progress {
        Some((done, total)) if total > 0 => format!("{:>3}% ({done}/{total}) {time}", done * 100 / total),
        _ => format!("{lines} lines {time}"),
    };
    let room = width.saturating_sub(head.chars().count() + 6);
    format!("{head}  {}", truncate(last.trim(), room))
}

fn terminal_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|c| c.parse::<usize>().ok())
        .filter(|w| *w >= 40)
        .unwrap_or(100)
}

/// Lit `reader` ligne par ligne (octets invalides tolérés) et envoie les lignes.
fn pump<R: std::io::Read + Send + 'static>(
    reader: R,
    tx: std::sync::mpsc::Sender<String>,
) -> std::thread::JoinHandle<()> {
    use std::io::BufRead;

    std::thread::spawn(move || {
        let mut reader = std::io::BufReader::new(reader);
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            match reader.read_until(b'\n', &mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let line = String::from_utf8_lossy(&buffer)
                        .trim_end_matches(['\r', '\n'])
                        .to_string();
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            }
        }
    })
}

/// Journal complet d'un échec : build/logs/<programme>-<date>.log (ou /tmp).
fn write_failure_log(program: &str, lines: &[String]) -> Option<std::path::PathBuf> {
    let name = std::path::Path::new(program)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "command".to_string());
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let mut dir = std::path::PathBuf::from("build/logs");
    if std::fs::create_dir_all(&dir).is_err() {
        dir = std::env::temp_dir();
    }
    let path = dir.join(format!("{name}-{epoch}.log"));
    std::fs::write(&path, lines.join("\n")).ok()?;
    Some(path)
}

/// Comme `run`, mais la sortie n'est pas affichée telle quelle : elle est lue au
/// fil de l'eau et résumée sous le spinner de l'étape (pourcentage quand l'outil
/// le donne, dernière ligne, durée). En cas de succès elle est jetée ; en cas
/// d'échec la fin de la sortie est affichée et le journal complet est écrit dans
/// build/logs/. Hors terminal (CI), une ligne d'avancement est imprimée toutes
/// les minutes.
pub fn run_quiet(cmd: &mut Command) -> Result<()> {
    use std::io::IsTerminal;
    use std::sync::mpsc::{self, RecvTimeoutError};
    use std::time::{Duration, Instant};

    let description = describe(cmd);
    let program = cmd.get_program().to_string_lossy().to_string();

    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to run `{description}`"))?;

    let (tx, rx) = mpsc::channel::<String>();
    let mut readers = Vec::new();
    if let Some(out) = child.stdout.take() {
        readers.push(pump(out, tx.clone()));
    }
    if let Some(err) = child.stderr.take() {
        readers.push(pump(err, tx.clone()));
    }
    drop(tx);

    let interactive = std::io::stderr().is_terminal();
    let width = terminal_width();
    let started = Instant::now();
    let mut last_draw = Instant::now();
    let mut last_beat = Instant::now();
    let mut bar: Option<indicatif::ProgressBar> = None;

    let mut log: Vec<String> = Vec::new();
    let mut lines = 0usize;
    let mut last = String::new();
    let mut progress: Option<(usize, usize)> = None;

    loop {
        match rx.recv_timeout(Duration::from_millis(150)) {
            Ok(line) => {
                lines += 1;
                if let Some(hint) = progress_hint(&line) {
                    progress = Some(hint);
                }
                if !line.trim().is_empty() {
                    last = line.clone();
                }
                if log.len() < MAX_LOG_LINES {
                    log.push(line);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        let elapsed = started.elapsed();

        if interactive {
            // The line only appears for commands that last (not for `ar x`...).
            if elapsed >= Duration::from_millis(1500) && last_draw.elapsed() >= Duration::from_millis(120) {
                let line = status_line(elapsed.as_secs(), lines, progress, &last, width);
                bar.get_or_insert_with(crate::ui::progress_line).set_message(line);
                last_draw = Instant::now();
            }
        } else if last_beat.elapsed() >= Duration::from_secs(60) {
            eprintln!("    ... {}", status_line(elapsed.as_secs(), lines, progress, &last, width));
            last_beat = Instant::now();
        }
    }

    for reader in readers {
        let _ = reader.join();
    }
    let status = child
        .wait()
        .with_context(|| format!("failed to wait for `{description}`"))?;

    if let Some(bar) = bar {
        bar.finish_and_clear();
    }

    if !status.success() {
        let shown = 150.min(log.len());
        eprintln!();
        if log.len() > shown {
            eprintln!("... ({} earlier lines not shown)", log.len() - shown);
        }
        for line in &log[log.len() - shown..] {
            eprintln!("{line}");
        }
        if let Some(path) = write_failure_log(&program, &log) {
            eprintln!("\nFull log: {}", path.display());
        }
        bail!("`{description}` exited with {status}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_hints() {
        assert_eq!(progress_hint("[123/456] Compiling C object foo.o"), Some((123, 456)));
        assert_eq!(progress_hint("  [ 12/345] Linking"), Some((12, 345)));
        assert_eq!(progress_hint("[ 45%] Building CXX"), Some((45, 100)));
        assert_eq!(progress_hint("  CC      drivers/net/foo.o"), None);
        assert_eq!(progress_hint("[abc] nope"), None);
        assert_eq!(progress_hint("[1/x] nope"), None);
    }

    #[test]
    fn status_lines() {
        let with_total = status_line(92, 10, Some((123, 456)), "Compiling foo.o", 100);
        assert!(with_total.starts_with(" 26% (123/456) 01:32"));
        assert!(with_total.ends_with("Compiling foo.o"));

        let without_total = status_line(42, 1523, None, "  CC drivers/net/foo.o", 100);
        assert!(without_total.starts_with("1523 lines 00:42"));
        assert!(without_total.contains("CC drivers/net/foo.o"));
    }

    #[test]
    fn long_lines_are_cut_to_the_width() {
        let text = status_line(1, 1, None, &"x".repeat(300), 60);
        assert!(text.chars().count() <= 60);
        assert!(text.ends_with('…'));
    }

    #[test]
    fn successful_commands_stay_quiet_and_failures_are_reported() {
        assert!(run_quiet(Command::new("sh").args(["-c", "echo ok; echo oops >&2"])).is_ok());
        let error = run_quiet(Command::new("sh").args(["-c", "echo boom >&2; exit 3"]))
            .unwrap_err()
            .to_string();
        assert!(error.contains("exited with"));
    }
}
