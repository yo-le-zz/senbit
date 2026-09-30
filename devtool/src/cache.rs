//! Cache de build : ne recompile un composant que si son empreinte (commit git,
//! statut "dirty", ou version épinglée) a changé depuis la dernière build réussie.

use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
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

    /// Supprime le fichier de cache sur disque et repart d'un état vide
    /// (utilisé par `--force`).
    pub fn clear(path: &Path) -> Result<()> {
        if path.is_file() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(String::as_str)
    }

    /// Vrai si `fingerprint` correspond à la dernière build réussie ET que
    /// toutes les sorties existent encore.
    pub fn is_fresh(&self, key: &str, fingerprint: &str, outputs: &[&Path]) -> bool {
        outputs.iter().all(|o| o.exists()) && self.get(key) == Some(fingerprint)
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

/// Empreinte simple du contenu d'un fichier ("missing" s'il n'existe pas) —
/// sert à détecter qu'un fichier de configuration a été modifié à la main
/// (ex. config/busybox.config) même quand les sources elles-mêmes n'ont pas
/// bougé.
pub fn hash_file(path: &Path) -> String {
    match fs::read(path) {
        Ok(bytes) => {
            let mut h = DefaultHasher::new();
            bytes.hash(&mut h);
            format!("{:016x}", h.finish())
        }
        Err(_) => "missing".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_file_changes_with_content() {
        let dir = std::env::temp_dir().join(format!("devtool-hash-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let f = dir.join("busybox.config");

        fs::write(&f, "CONFIG_A=y\n").unwrap();
        let h1 = hash_file(&f);
        fs::write(&f, "CONFIG_A=y\nCONFIG_B=y\n").unwrap();
        let h2 = hash_file(&f);
        fs::write(&f, "CONFIG_A=y\n").unwrap();
        let h3 = hash_file(&f);

        assert_ne!(h1, h2, "changing the file content must change the hash");
        assert_eq!(h1, h3, "identical content must hash the same");
        assert_eq!(hash_file(&dir.join("missing")), "missing");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cache_roundtrip_and_freshness() {
        let dir = std::env::temp_dir().join(format!("devtool-cache-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let out = dir.join("out.bin");
        fs::write(&out, b"x").unwrap();

        let file = dir.join("state.json");
        let mut c = Cache::open(file.clone());
        assert!(!c.is_fresh("k", "fp1", &[&out]));
        c.record("k", "fp1").unwrap();
        assert!(c.is_fresh("k", "fp1", &[&out]));
        assert!(!c.is_fresh("k", "fp2", &[&out]));
        assert_eq!(c.get("k"), Some("fp1"));

        // La sortie disparaît : plus "fresh" même avec la bonne empreinte.
        fs::remove_file(&out).unwrap();
        assert!(!c.is_fresh("k", "fp1", &[&out]));

        // Rechargé depuis le disque : même état.
        fs::write(&out, b"x").unwrap();
        let reopened = Cache::open(file.clone());
        assert!(reopened.is_fresh("k", "fp1", &[&out]));

        Cache::clear(&file).unwrap();
        let cleared = Cache::open(file);
        assert!(!cleared.is_fresh("k", "fp1", &[&out]));

        let _ = fs::remove_dir_all(&dir);
    }
}
