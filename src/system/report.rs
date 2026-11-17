//! Rapport de démarrage : ce qui a marché, ce qui a échoué.
//!
//! Chaque étape non critique du démarrage (hostname, réseau, logs, clavier...)
//! enregistre ici son résultat au lieu de faire paniquer le système : un réseau
//! absent ne doit jamais empêcher de se connecter. Juste avant de passer la main
//! à systemd, le rapport est écrit dans :
//!
//!   /run/senbit/boot-report            démarrage en cours (lu par `bootreport`)
//!   /var/log/senbit/boot-report.log    historique des démarrages
//!
//! Format (une ligne par étape, champs séparés par des tabulations) :
//!
//!   statut <TAB> uptime_ms <TAB> étape <TAB> détail
//!
//! avec statut = ok | warn | failed. `commands/bootreport` lit ce fichier et
//! `/etc/profile.d/senbit-bootreport.sh` l'affiche à la connexion.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::Mutex;

pub const REPORT_DIR: &str = "/run/senbit";
pub const REPORT_FILE: &str = "/run/senbit/boot-report";
pub const HISTORY_DIR: &str = "/var/log/senbit";
pub const HISTORY_FILE: &str = "/var/log/senbit/boot-report.log";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Warn,
    Failed,
}

impl Status {
    pub fn tag(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Warn => "warn",
            Status::Failed => "failed",
        }
    }

    #[allow(dead_code)] // used by the tests and kept as the inverse of `tag`
    pub fn from_tag(tag: &str) -> Option<Status> {
        match tag {
            "ok" => Some(Status::Ok),
            "warn" => Some(Status::Warn),
            "failed" => Some(Status::Failed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub status: Status,
    pub uptime_ms: u64,
    pub step: String,
    pub detail: String,
}

static ENTRIES: Mutex<Vec<Entry>> = Mutex::new(Vec::new());

fn uptime_ms() -> u64 {
    fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|text| {
            text.split_whitespace()
                .next()
                .and_then(|s| s.parse::<f64>().ok())
        })
        .map(|seconds| (seconds * 1000.0) as u64)
        .unwrap_or(0)
}

/// Un champ ne contient ni tabulation ni saut de ligne (format une-ligne-par-étape).
fn clean(text: &str) -> String {
    text.replace(['\t', '\n', '\r'], " ")
}

fn record(status: Status, step: &str, detail: &str) {
    if let Ok(mut entries) = ENTRIES.lock() {
        entries.push(Entry {
            status,
            uptime_ms: uptime_ms(),
            step: clean(step),
            detail: clean(detail),
        });
    }
}

/// L'étape a réussi.
pub fn ok(step: &str, detail: impl AsRef<str>) {
    record(Status::Ok, step, detail.as_ref());
}

/// L'étape a échoué mais le système démarre quand même.
pub fn warn(step: &str, detail: impl AsRef<str>) {
    record(Status::Warn, step, detail.as_ref());
}

/// L'étape a échoué et une fonction importante manque.
#[allow(dead_code)]
pub fn failed(step: &str, detail: impl AsRef<str>) {
    record(Status::Failed, step, detail.as_ref());
}

pub fn render(entries: &[Entry]) -> String {
    let mut out = String::new();
    for entry in entries {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            entry.status.tag(),
            entry.uptime_ms,
            clean(&entry.step),
            clean(&entry.detail)
        ));
    }
    out
}

#[allow(dead_code)] // inverse of `render` (tested); the reader is commands/bootreport
pub fn parse(text: &str) -> Vec<Entry> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.splitn(4, '\t');
            let status = Status::from_tag(fields.next()?)?;
            let uptime_ms = fields.next()?.parse().ok()?;
            let step = fields.next()?.to_string();
            let detail = fields.next().unwrap_or("").to_string();
            Some(Entry {
                status,
                uptime_ms,
                step,
                detail,
            })
        })
        .collect()
}

/// Écrit le rapport. À appeler après switch_root (le /run du nouveau système
/// est alors en place) et avant l'exec de systemd. Ne échoue jamais bruyamment :
/// un rapport impossible à écrire ne doit pas empêcher de démarrer.
pub fn flush() {
    let entries = match ENTRIES.lock() {
        Ok(entries) => entries.clone(),
        Err(_) => return,
    };
    let text = render(&entries);

    if fs::create_dir_all(REPORT_DIR).is_ok() {
        let _ = fs::write(REPORT_FILE, &text);
    }

    if fs::create_dir_all(HISTORY_DIR).is_ok() {
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(HISTORY_FILE)
        {
            let epoch = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let _ = writeln!(file, "# boot {epoch}");
            let _ = file.write_all(text.as_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_and_parse_round_trip() {
        let entries = vec![
            Entry {
                status: Status::Ok,
                uptime_ms: 1200,
                step: "Root filesystem".into(),
                detail: "/dev/sda2 mounted".into(),
            },
            Entry {
                status: Status::Warn,
                uptime_ms: 3400,
                step: "Network".into(),
                detail: "gateway 10.0.2.2 does not answer".into(),
            },
        ];
        let parsed = parse(&render(&entries));
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[1].status, Status::Warn);
        assert_eq!(parsed[1].step, "Network");
        assert_eq!(parsed[1].detail, "gateway 10.0.2.2 does not answer");
        assert_eq!(parsed[0].uptime_ms, 1200);
    }

    #[test]
    fn fields_never_break_the_one_line_format() {
        let text = render(&[Entry {
            status: Status::Failed,
            uptime_ms: 1,
            step: "a\tb".into(),
            detail: "line1\nline2".into(),
        }]);
        assert_eq!(text.lines().count(), 1);
        assert_eq!(text.matches('\t').count(), 3);
    }

    #[test]
    fn unknown_lines_are_skipped() {
        assert!(parse("garbage\nnope\t1\tx\ty\n").is_empty());
    }
}
