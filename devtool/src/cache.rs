//! Cache de build : ne recompile un composant que si son empreinte (commit git,
//! statut "dirty", ou version épinglée) a changé depuis la dernière build réussie.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

pub struct Cache {
    path: PathBuf,
    entries: BTreeMap<String, String>,
}

impl Cache {
    pub fn open(path: PathBuf) -> Self {
        let entries = fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        Self { path, entries }
    }

    /// Vrai si `fingerprint` correspond à la dernière build réussie ET que
    /// toutes les sorties existent encore.
    pub fn is_fresh(&self, key: &str, fingerprint: &str, outputs: &[&Path]) -> bool {
        outputs.iter().all(|o| o.exists()) && self.entries.get(key).map(String::as_str) == Some(fingerprint)
    }

    pub fn record(&mut self, key: &str, fingerprint: &str) -> Result<()> {
        self.entries.insert(key.to_string(), fingerprint.to_string());
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, serde_json::to_string_pretty(&self.entries)?)?;
        Ok(())
    }
}

/// Empreinte d'un dépôt git : commit courant + présence de modifications
/// locales non commitées (nouveau commit, patch appliqué à la main...).
pub fn git_fingerprint(dir: &Path) -> String {
    let head = crate::gitutil::head(dir);
    let dirty = !crate::gitutil::status_porcelain(dir).is_empty();
    format!("git:{head}:dirty={dirty}")
}
