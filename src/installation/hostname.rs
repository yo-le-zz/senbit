use anyhow::Result;
use colored::Colorize;
use inquire::{Text, validator::Validation};
use std::path::Path;

pub fn get_hostname() -> Result<String> {
    let hostname = Text::new(&format!("{} ", "Hostname:".bold().green()))
        .with_default("senbit")
        .with_validator(|input: &str| {
            if input.is_empty() {
                return Ok(Validation::Invalid("Hostname cannot be empty.".into()));
            }

            if input.contains(' ') {
                return Ok(Validation::Invalid(
                    "Hostname cannot contain spaces.".into(),
                ));
            }

            Ok(Validation::Valid)
        })
        .prompt()?;

    Ok(hostname)
}

pub fn write_hostname(root: &Path, hostname: &str) -> Result<()> {
    std::fs::write(root.join("etc/hostname"), format!("{}\n", hostname))?;

    // /etc/hosts: the machine must resolve its own name (sudo, postgresql,
    // mail daemons... print warnings or wait for a DNS timeout otherwise).
    // The base file comes from the rootfs overlay; the line is replaced if the
    // hostname is changed again.
    let hosts_path = root.join("etc/hosts");
    let current = std::fs::read_to_string(&hosts_path).unwrap_or_default();

    let mut hosts: String = current
        .lines()
        .filter(|line| !line.trim_start().starts_with("127.0.1.1"))
        .map(|line| format!("{line}\n"))
        .collect();

    if hosts.trim().is_empty() {
        hosts.push_str("127.0.0.1   localhost\n::1         localhost ip6-localhost ip6-loopback\n");
    }
    hosts.push_str(&format!("127.0.1.1   {hostname}\n"));

    std::fs::write(&hosts_path, hosts)?;

    Ok(())
}
