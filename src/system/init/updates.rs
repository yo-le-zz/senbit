use anyhow::{Context, Result};

/// Version of the installed Senbit (`VERSION=` of /etc/senbit-release, kept up to date by the
/// senbit-userspace package). Updates are never offered at boot: they go through `apt upgrade`
/// (docs/UPDATES.md), so a missing network or repository can never block a boot.
pub fn get_version(root: &str) -> Result<String> {
    let path = std::path::Path::new(root).join("etc/senbit-release");

    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    contents
        .lines()
        .find_map(|line| line.strip_prefix("VERSION="))
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .map(String::from)
        .ok_or_else(|| anyhow::anyhow!("VERSION not found in {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_version_line() {
        let dir = std::env::temp_dir().join(format!("senbit-version-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("etc")).unwrap();
        std::fs::write(dir.join("etc/senbit-release"), "NAME=Senbit\nVERSION=0.4.0\n").unwrap();
        assert_eq!(get_version(dir.to_str().unwrap()).unwrap(), "0.4.0");

        std::fs::write(dir.join("etc/senbit-release"), "NAME=Senbit\n").unwrap();
        assert!(get_version(dir.to_str().unwrap()).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
