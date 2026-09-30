use anyhow::Result;
use std::process::Command;
use anyhow::anyhow;

pub fn detect_network_interfaces() -> Result<Vec<String>> {
    let output = Command::new("ip")
        .args(["-o", "link"])
        .output()
        .map_err(|e| anyhow!("Failed to execute ip: {}", e))?;

    if !output.status.success() {
        return Err(anyhow!(
            "ip command failed with status: {}",
            output.status
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    let interfaces = stdout
        .lines()
        .filter_map(|line| {
            let name = line.split(':').nth(1)?.trim();

            if name == "lo" {
                return None;
            }

            Some(name.to_string())
        })
        .collect();

    Ok(interfaces)
}

pub fn filter_network_interfaces(
    interfaces: &[String],
) -> Vec<String> {
    interfaces
        .iter()
        .filter(|interface| {
            let name = interface
                .split('@')
                .next()
                .unwrap_or(interface);

            name != "lo"
                && !name.starts_with("sit")
                && !name.starts_with("ip6")
                && !name.starts_with("tun")
                && !name.starts_with("tap")
                && !name.starts_with("docker")
                && !name.starts_with("br-")
                && !name.starts_with("veth")
        })
        .cloned()
        .collect()
}