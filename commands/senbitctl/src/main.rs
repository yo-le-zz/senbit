//! senbitctl : l'outil de contrôle de Senbit.
//!
//!   senbitctl status                          état du système
//!   senbitctl service <action> [unité]        cycle de vie des services (systemd)
//!   senbitctl start|stop|restart|... <unité>  raccourcis de `service`
//!   senbitctl net show                        interfaces, routes, DNS
//!   senbitctl net dhcp <interface>            DHCP au démarrage et maintenant
//!   senbitctl net static <interface> --address A.B.C.D/N [--gateway G] [--dns D,D]
//!   senbitctl ssh install|enable|disable|status
//!   senbitctl commands                        commandes natives installées
//!
//! Les services sont ceux de systemd (fichiers d'unité = format de définition
//! des services de Senbit, dépendances comprises) ; senbitctl ne réimplémente
//! rien : il appelle systemctl / journalctl avec des options saines.
//! Le réseau est celui de /etc/network/interfaces (lu par init au démarrage)
//! plus l'unité senbit-dhcp@<interface>.service.

use std::fs;
use std::os::unix::process::CommandExt;
use std::process::{Command, ExitCode, Stdio};

const INTERFACES: &str = "/etc/network/interfaces";
const RESOLV_CONF: &str = "/etc/resolv.conf";
const COMMANDS_LIST: &str = "/usr/share/senbit/commands.list";

/* ------------------------------------------------------------------ */
/* Fonctions pures (testées)                                          */
/* ------------------------------------------------------------------ */

const UNIT_SUFFIXES: &[&str] = &[
    ".service", ".socket", ".timer", ".target", ".mount", ".path", ".slice", ".scope", ".device",
    ".automount", ".swap",
];

/// `nginx` -> `nginx.service` ; `ssh.socket` reste tel quel.
fn normalize_unit(name: &str) -> String {
    if UNIT_SUFFIXES.iter().any(|s| name.ends_with(s)) {
        name.to_string()
    } else {
        format!("{name}.service")
    }
}

/// "192.168.1.10/24" -> ("192.168.1.10", 24)
fn parse_cidr(text: &str) -> Result<(String, u8), String> {
    let (address, prefix) = text
        .split_once('/')
        .ok_or_else(|| format!("`{text}`: expected ADDRESS/PREFIX (e.g. 192.168.1.10/24)"))?;
    let _: std::net::Ipv4Addr = address
        .parse()
        .map_err(|_| format!("`{address}` is not a valid IPv4 address"))?;
    let prefix: u8 = prefix
        .parse()
        .map_err(|_| format!("`{prefix}` is not a valid prefix length"))?;
    if prefix > 32 {
        return Err(format!("prefix /{prefix} is out of range (0-32)"));
    }
    Ok((address.to_string(), prefix))
}

fn prefix_to_netmask(prefix: u8) -> String {
    let bits: u32 = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - u32::from(prefix))
    };
    std::net::Ipv4Addr::from(bits).to_string()
}

fn valid_interface(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 15
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':'))
}

/// Une ligne ouvre une nouvelle « stanza » de /etc/network/interfaces.
fn starts_stanza(line: &str) -> bool {
    let first = line.split_whitespace().next().unwrap_or("");
    matches!(first, "auto" | "iface" | "mapping" | "source" | "source-directory")
        || first.starts_with("allow-")
}

/// Une stanza concerne-t-elle `iface` ? (`auto eth0 eth1`, `iface eth0 inet dhcp`)
fn stanza_is_for(line: &str, iface: &str) -> bool {
    let mut words = line.split_whitespace();
    let first = words.next().unwrap_or("");
    if first == "iface" {
        words.next() == Some(iface)
    } else if first == "auto" || first.starts_with("allow-") {
        // `auto eth0` uniquement : une ligne qui en liste plusieurs est conservée.
        let names: Vec<&str> = words.collect();
        names.len() == 1 && names[0] == iface
    } else {
        false
    }
}

/// Supprime du fichier toutes les stanzas de `iface` (et leurs lignes indentées).
fn remove_interface(text: &str, iface: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut skipping = false;

    for line in text.lines() {
        if starts_stanza(line) {
            skipping = stanza_is_for(line, iface);
        } else if !line.trim().is_empty() && !line.starts_with([' ', '\t']) {
            // commentaire ou autre ligne non indentée : fin de la stanza
            skipping = false;
        }
        if !skipping {
            out.push(line);
        }
    }

    let mut result = out.join("\n");
    while result.ends_with("\n\n") {
        result.pop();
    }
    if !result.is_empty() && !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

struct StaticConfig {
    address: String,
    prefix: u8,
    gateway: Option<String>,
}

/// Nouveau contenu de /etc/network/interfaces pour `iface`.
fn render_interfaces(existing: &str, iface: &str, config: Option<&StaticConfig>) -> String {
    let mut text = remove_interface(existing, iface);
    if !text.is_empty() && !text.ends_with("\n\n") {
        text.push('\n');
    }
    text.push_str(&format!("auto {iface}\n"));
    match config {
        None => text.push_str(&format!("iface {iface} inet dhcp\n")),
        Some(c) => {
            text.push_str(&format!("iface {iface} inet static\n"));
            text.push_str(&format!("    address {}\n", c.address));
            text.push_str(&format!("    netmask {}\n", prefix_to_netmask(c.prefix)));
            if let Some(gateway) = &c.gateway {
                text.push_str(&format!("    gateway {gateway}\n"));
            }
        }
    }
    text
}

fn render_resolv_conf(servers: &[String]) -> String {
    let mut text = String::from("# Written by senbitctl net static\n");
    for server in servers {
        text.push_str(&format!("nameserver {server}\n"));
    }
    text
}

fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;
    if days > 0 {
        format!("{days}d {hours}h {minutes}m")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

/// Valeur (en ko) d'une ligne de /proc/meminfo.
fn meminfo_kb(meminfo: &str, key: &str) -> Option<u64> {
    meminfo
        .lines()
        .find(|l| l.starts_with(key))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
}

/* ------------------------------------------------------------------ */
/* Exécution de commandes                                             */
/* ------------------------------------------------------------------ */

/// Lance une commande et renvoie son code de sortie.
fn run(program: &str, args: &[&str]) -> i32 {
    match Command::new(program).args(args).status() {
        Ok(status) => status.code().unwrap_or(1),
        Err(e) => {
            eprintln!("senbitctl: cannot run {program}: {e}");
            127
        }
    }
}

/// Remplace le processus par la commande (pas de code de sortie à relayer).
fn exec(program: &str, args: &[String]) -> ExitCode {
    let error = Command::new(program).args(args).exec();
    eprintln!("senbitctl: cannot run {program}: {error}");
    ExitCode::from(127)
}

fn capture(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn is_root() -> bool {
    use std::os::unix::fs::MetadataExt;
    fs::metadata("/proc/self").map(|m| m.uid() == 0).unwrap_or(false)
}

fn need_root(what: &str) -> Result<(), String> {
    if is_root() {
        Ok(())
    } else {
        Err(format!("{what} requires root (use sudo)"))
    }
}

/* ------------------------------------------------------------------ */
/* senbitctl status                                                   */
/* ------------------------------------------------------------------ */

fn status() -> ExitCode {
    let read = |path: &str| fs::read_to_string(path).unwrap_or_default().trim().to_string();

    println!("Hostname : {}", read("/proc/sys/kernel/hostname"));
    println!("Kernel   : {}", read("/proc/sys/kernel/osrelease"));

    let uptime = read("/proc/uptime")
        .split_whitespace()
        .next()
        .and_then(|s| s.parse::<f64>().ok())
        .map(|s| format_uptime(s as u64))
        .unwrap_or_else(|| "?".to_string());
    println!("Uptime   : {uptime}");

    let load = read("/proc/loadavg")
        .split_whitespace()
        .take(3)
        .collect::<Vec<_>>()
        .join(" ");
    println!("Load     : {load}");

    let meminfo = read("/proc/meminfo");
    if let (Some(total), Some(available)) = (
        meminfo_kb(&meminfo, "MemTotal:"),
        meminfo_kb(&meminfo, "MemAvailable:"),
    ) {
        println!(
            "Memory   : {} MiB used / {} MiB",
            total.saturating_sub(available) / 1024,
            total / 1024
        );
    }

    let state = capture("systemctl", &["is-system-running"]).unwrap_or_else(|| "unknown".into());
    println!("systemd  : {state}");

    if let Some(failed) = capture("systemctl", &["--failed", "--no-legend", "--plain", "--no-pager"]) {
        let count = failed.lines().filter(|l| !l.trim().is_empty()).count();
        if count == 0 {
            println!("Failed   : none");
        } else {
            println!("Failed   : {count} unit(s)");
            for line in failed.lines().filter(|l| !l.trim().is_empty()) {
                println!("           {}", line.split_whitespace().next().unwrap_or(line));
            }
        }
    }

    if let Some(addresses) = capture("ip", &["-br", "-4", "addr"]) {
        println!("Network  :");
        for line in addresses.lines() {
            println!("           {line}");
        }
    }
    ExitCode::SUCCESS
}

/* ------------------------------------------------------------------ */
/* senbitctl service                                                  */
/* ------------------------------------------------------------------ */

const SYSTEMCTL_ACTIONS: &[&str] = &[
    "start", "stop", "restart", "reload", "status", "is-active", "is-enabled", "mask", "unmask",
    "try-restart", "reload-or-restart", "cat", "show",
];

fn service(args: &[String]) -> Result<ExitCode, String> {
    let Some(action) = args.first() else {
        return Err(service_usage());
    };
    let rest = &args[1..];

    match action.as_str() {
        "list" => {
            let mut a = vec![
                "list-units".to_string(),
                "--type=service".to_string(),
                "--no-pager".to_string(),
            ];
            a.extend(rest.iter().cloned());
            Ok(exec("systemctl", &a))
        }
        "failed" => Ok(exec(
            "systemctl",
            &["--failed".to_string(), "--no-pager".to_string()],
        )),
        "logs" => {
            let unit = rest
                .first()
                .ok_or_else(|| "usage: senbitctl service logs <service> [-f] [-n N]".to_string())?;
            let mut a = vec![
                "-u".to_string(),
                normalize_unit(unit),
                "--no-pager".to_string(),
            ];
            a.extend(rest[1..].iter().cloned());
            Ok(exec("journalctl", &a))
        }
        // enable / disable : activation au démarrage ET démarrage/arrêt immédiat.
        "enable" | "disable" => {
            need_root(action)?;
            let unit = rest
                .first()
                .ok_or_else(|| format!("usage: senbitctl service {action} <service>"))?;
            let mut a = vec![
                action.clone(),
                "--now".to_string(),
                normalize_unit(unit),
            ];
            a.extend(rest[1..].iter().cloned());
            Ok(exec("systemctl", &a))
        }
        other if SYSTEMCTL_ACTIONS.contains(&other) => {
            let unit = rest
                .first()
                .ok_or_else(|| format!("usage: senbitctl service {other} <service>"))?;
            if matches!(other, "start" | "stop" | "restart" | "reload" | "try-restart" | "reload-or-restart" | "mask" | "unmask") {
                need_root(other)?;
            }
            let mut a = vec![other.to_string(), "--no-pager".to_string(), normalize_unit(unit)];
            a.extend(rest[1..].iter().cloned());
            Ok(exec("systemctl", &a))
        }
        other => Err(format!("unknown service action `{other}`\n{}", service_usage())),
    }
}

fn service_usage() -> String {
    "usage: senbitctl service <action> [service]\n\
     \x20 start | stop | restart | reload   change the state now\n\
     \x20 enable | disable                  start/stop now AND at every boot\n\
     \x20 status | is-active | is-enabled   inspect\n\
     \x20 list | failed | logs <service>     overview and logs\n\
     \x20 mask | unmask                     forbid / allow starting"
        .to_string()
}

/* ------------------------------------------------------------------ */
/* senbitctl net                                                      */
/* ------------------------------------------------------------------ */

fn net_show() -> ExitCode {
    println!("== Interfaces ==");
    run("ip", &["-br", "addr"]);
    println!("\n== Routes ==");
    run("ip", &["route"]);
    println!("\n== DNS ({RESOLV_CONF}) ==");
    print!("{}", fs::read_to_string(RESOLV_CONF).unwrap_or_default());
    println!("\n== Configuration ({INTERFACES}) ==");
    print!("{}", fs::read_to_string(INTERFACES).unwrap_or_default());
    ExitCode::SUCCESS
}

fn write_interfaces(iface: &str, config: Option<&StaticConfig>) -> Result<(), String> {
    let existing = fs::read_to_string(INTERFACES).unwrap_or_default();
    let updated = render_interfaces(&existing, iface, config);
    if let Some(parent) = std::path::Path::new(INTERFACES).parent() {
        fs::create_dir_all(parent).map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    fs::write(INTERFACES, updated).map_err(|e| format!("cannot write {INTERFACES}: {e}"))
}

fn net_dhcp(args: &[String]) -> Result<ExitCode, String> {
    need_root("net dhcp")?;
    let iface = args
        .first()
        .ok_or_else(|| "usage: senbitctl net dhcp <interface>".to_string())?;
    if !valid_interface(iface) {
        return Err(format!("`{iface}` is not a valid interface name"));
    }

    write_interfaces(iface, None)?;
    println!("{INTERFACES}: {iface} -> DHCP");

    let unit = format!("senbit-dhcp@{iface}.service");
    let code = run("systemctl", &["enable", "--now", &unit]);
    Ok(ExitCode::from(code.clamp(0, 255) as u8))
}

#[derive(Default)]
struct StaticArgs {
    address: Option<String>,
    gateway: Option<String>,
    dns: Vec<String>,
}

fn parse_static_args(args: &[String]) -> Result<StaticArgs, String> {
    let mut parsed = StaticArgs::default();
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        i += 1;
        let (key, inline) = match arg.split_once('=') {
            Some((k, v)) if arg.starts_with("--") => (k, Some(v.to_string())),
            _ => (arg, None),
        };
        let mut value = |name: &str| -> Result<String, String> {
            if let Some(v) = inline.clone() {
                return Ok(v);
            }
            let v = args
                .get(i)
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))?;
            i += 1;
            Ok(v)
        };
        match key {
            "--address" | "-a" => parsed.address = Some(value("--address")?),
            "--gateway" | "-g" => parsed.gateway = Some(value("--gateway")?),
            "--dns" | "-d" => {
                let list = value("--dns")?;
                parsed
                    .dns
                    .extend(list.split(',').filter(|s| !s.is_empty()).map(str::to_string));
            }
            other => return Err(format!("unknown option `{other}`")),
        }
    }
    Ok(parsed)
}

fn net_static(args: &[String]) -> Result<ExitCode, String> {
    need_root("net static")?;
    let iface = args.first().ok_or_else(|| {
        "usage: senbitctl net static <interface> --address A.B.C.D/N [--gateway G] [--dns D1,D2]"
            .to_string()
    })?;
    if !valid_interface(iface) {
        return Err(format!("`{iface}` is not a valid interface name"));
    }
    let parsed = parse_static_args(&args[1..])?;
    let address_text = parsed
        .address
        .ok_or_else(|| "--address A.B.C.D/N is required".to_string())?;
    let (address, prefix) = parse_cidr(&address_text)?;

    if let Some(gateway) = &parsed.gateway {
        gateway
            .parse::<std::net::Ipv4Addr>()
            .map_err(|_| format!("`{gateway}` is not a valid IPv4 address"))?;
    }
    for server in &parsed.dns {
        server
            .parse::<std::net::IpAddr>()
            .map_err(|_| format!("`{server}` is not a valid DNS server address"))?;
    }

    let config = StaticConfig {
        address: address.clone(),
        prefix,
        gateway: parsed.gateway.clone(),
    };
    write_interfaces(iface, Some(&config))?;
    println!("{INTERFACES}: {iface} -> static {address}/{prefix}");

    if !parsed.dns.is_empty() {
        fs::write(RESOLV_CONF, render_resolv_conf(&parsed.dns))
            .map_err(|e| format!("cannot write {RESOLV_CONF}: {e}"))?;
        println!("{RESOLV_CONF}: {}", parsed.dns.join(", "));
    }

    // DHCP and a static address do not mix: stop the DHCP client for this interface.
    let unit = format!("senbit-dhcp@{iface}.service");
    run("systemctl", &["disable", "--now", &unit]);

    // Apply now.
    run("ip", &["addr", "flush", "dev", iface]);
    run("ip", &["link", "set", iface, "up"]);
    let cidr = format!("{address}/{prefix}");
    let code = run("ip", &["addr", "add", &cidr, "dev", iface]);
    if code != 0 {
        return Err(format!("`ip addr add {cidr} dev {iface}` failed"));
    }
    if let Some(gateway) = &parsed.gateway {
        run("ip", &["route", "replace", "default", "via", gateway, "dev", iface]);
    }
    println!("{iface} configured.");
    Ok(ExitCode::SUCCESS)
}

fn net(args: &[String]) -> Result<ExitCode, String> {
    match args.first().map(String::as_str) {
        None | Some("show") => Ok(net_show()),
        Some("dhcp") => net_dhcp(&args[1..]),
        Some("static") => net_static(&args[1..]),
        Some(other) => Err(format!(
            "unknown net command `{other}`\nusage: senbitctl net show | dhcp <if> | static <if> --address A/N [--gateway G] [--dns D,D]"
        )),
    }
}

/* ------------------------------------------------------------------ */
/* senbitctl ssh                                                      */
/* ------------------------------------------------------------------ */

fn ssh_installed() -> bool {
    ["/usr/sbin/sshd", "/usr/bin/sshd"]
        .iter()
        .any(|p| std::path::Path::new(p).exists())
}

fn ssh(args: &[String]) -> Result<ExitCode, String> {
    let action = args.first().map(String::as_str).unwrap_or("status");

    match action {
        "install" => {
            need_root("ssh install")?;
            if !ssh_installed() {
                let code = run("senbit-pkg", &["install", "openssh-server"]);
                if code != 0 {
                    return Err("could not install openssh-server".to_string());
                }
            }
            let code = run("systemctl", &["enable", "--now", "ssh.service"]);
            Ok(ExitCode::from(code.clamp(0, 255) as u8))
        }
        "enable" | "disable" => {
            need_root(action)?;
            if !ssh_installed() {
                return Err("OpenSSH server is not installed: run `senbitctl ssh install`".to_string());
            }
            let code = run("systemctl", &[action, "--now", "ssh.service"]);
            Ok(ExitCode::from(code.clamp(0, 255) as u8))
        }
        "status" => {
            if !ssh_installed() {
                println!("OpenSSH server: not installed (senbitctl ssh install)");
                return Ok(ExitCode::from(3));
            }
            Ok(exec(
                "systemctl",
                &["status".to_string(), "--no-pager".to_string(), "ssh.service".to_string()],
            ))
        }
        other => Err(format!(
            "unknown ssh command `{other}`\nusage: senbitctl ssh install | enable | disable | status"
        )),
    }
}

/* ------------------------------------------------------------------ */
/* Point d'entrée                                                     */
/* ------------------------------------------------------------------ */

fn commands_list() -> ExitCode {
    match fs::read_to_string(COMMANDS_LIST) {
        Ok(text) => {
            println!("{:<18} {:<6} {}", "COMMAND", "DIR", "ALIASES");
            for line in text.lines() {
                let mut parts = line.split_whitespace();
                if let (Some(name), Some(dir)) = (parts.next(), parts.next()) {
                    println!("{:<18} /{:<5} {}", name, dir, parts.next().unwrap_or("-"));
                }
            }
            ExitCode::SUCCESS
        }
        Err(_) => {
            eprintln!("senbitctl: {COMMANDS_LIST} not found");
            ExitCode::FAILURE
        }
    }
}

fn usage() -> &'static str {
    "senbitctl - Senbit control tool\n\
     \n\
     usage:\n\
     \x20 senbitctl status\n\
     \x20 senbitctl service <start|stop|restart|reload|enable|disable|status|list|failed|logs> [service]\n\
     \x20 senbitctl <start|stop|restart|enable|disable|status|logs> <service>   (shortcut)\n\
     \x20 senbitctl net show\n\
     \x20 senbitctl net dhcp <interface>\n\
     \x20 senbitctl net static <interface> --address A.B.C.D/N [--gateway G] [--dns D1,D2]\n\
     \x20 senbitctl ssh <install|enable|disable|status>\n\
     \x20 senbitctl commands\n\
     \x20 senbitctl version"
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let result = match args.first().map(String::as_str) {
        None | Some("help") | Some("-h") | Some("--help") => {
            println!("{}", usage());
            Ok(ExitCode::SUCCESS)
        }
        Some("version") | Some("--version") | Some("-V") => {
            println!("senbitctl {}", env!("CARGO_PKG_VERSION"));
            Ok(ExitCode::SUCCESS)
        }
        // `senbitctl status` : état du système ; `senbitctl status nginx` : état du service.
        Some("status") if args.len() == 1 => Ok(status()),
        Some("commands") => Ok(commands_list()),
        Some("service") | Some("svc") => service(&args[1..]),
        Some("net") | Some("network") => net(&args[1..]),
        Some("ssh") => ssh(&args[1..]),
        // Raccourcis : `senbitctl restart nginx` == `senbitctl service restart nginx`.
        Some(action)
            if matches!(
                action,
                "start" | "stop" | "restart" | "reload" | "enable" | "disable" | "status"
                    | "logs" | "list" | "failed" | "is-active" | "is-enabled" | "mask" | "unmask"
            ) =>
        {
            service(&args)
        }
        Some(other) => Err(format!("unknown command `{other}`\n\n{}", usage())),
    };

    match result {
        Ok(code) => code,
        Err(message) => {
            eprintln!("senbitctl: {message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_get_a_suffix() {
        assert_eq!(normalize_unit("nginx"), "nginx.service");
        assert_eq!(normalize_unit("ssh.socket"), "ssh.socket");
        assert_eq!(normalize_unit("getty@tty1"), "getty@tty1.service");
        assert_eq!(normalize_unit("multi-user.target"), "multi-user.target");
    }

    #[test]
    fn cidr_parsing() {
        assert_eq!(parse_cidr("192.168.1.10/24").unwrap(), ("192.168.1.10".to_string(), 24));
        assert!(parse_cidr("192.168.1.10").is_err());
        assert!(parse_cidr("192.168.1.10/33").is_err());
        assert!(parse_cidr("nope/24").is_err());
    }

    #[test]
    fn netmasks() {
        assert_eq!(prefix_to_netmask(24), "255.255.255.0");
        assert_eq!(prefix_to_netmask(16), "255.255.0.0");
        assert_eq!(prefix_to_netmask(32), "255.255.255.255");
        assert_eq!(prefix_to_netmask(0), "0.0.0.0");
        assert_eq!(prefix_to_netmask(22), "255.255.252.0");
    }

    #[test]
    fn interface_names() {
        assert!(valid_interface("eth0"));
        assert!(valid_interface("ens3"));
        assert!(!valid_interface(""));
        assert!(!valid_interface("a b"));
        assert!(!valid_interface("a/b"));
        assert!(!valid_interface("averyveryverylongname"));
    }

    #[test]
    fn dhcp_stanza_is_replaced_by_a_static_one() {
        let before = "auto lo\niface lo inet loopback\n\nauto eth0\niface eth0 inet dhcp\n";
        let config = StaticConfig {
            address: "192.168.1.10".into(),
            prefix: 24,
            gateway: Some("192.168.1.1".into()),
        };
        let after = render_interfaces(before, "eth0", Some(&config));

        assert!(after.contains("auto lo\niface lo inet loopback"));
        assert!(!after.contains("inet dhcp"));
        assert!(after.contains("iface eth0 inet static"));
        assert!(after.contains("    address 192.168.1.10\n"));
        assert!(after.contains("    netmask 255.255.255.0\n"));
        assert!(after.contains("    gateway 192.168.1.1\n"));
        assert_eq!(after.matches("auto eth0").count(), 1);
    }

    #[test]
    fn static_stanza_goes_back_to_dhcp() {
        let before = "auto eth0\niface eth0 inet static\n    address 10.0.0.2\n    netmask 255.255.255.0\n    gateway 10.0.0.1\n\nauto eth1\niface eth1 inet dhcp\n";
        let after = render_interfaces(before, "eth0", None);

        assert!(!after.contains("10.0.0.2"));
        assert!(after.contains("iface eth0 inet dhcp"));
        // l'autre interface est intacte
        assert!(after.contains("auto eth1\niface eth1 inet dhcp"));
    }

    #[test]
    fn rendering_into_an_empty_file() {
        let after = render_interfaces("", "eth0", None);
        assert_eq!(after, "auto eth0\niface eth0 inet dhcp\n");
    }

    #[test]
    fn resolv_conf_lists_the_servers() {
        let text = render_resolv_conf(&["1.1.1.1".to_string(), "9.9.9.9".to_string()]);
        assert!(text.contains("nameserver 1.1.1.1\n"));
        assert!(text.contains("nameserver 9.9.9.9\n"));
    }

    #[test]
    fn static_args_are_parsed() {
        let args: Vec<String> = ["--address", "10.0.0.5/24", "--gateway=10.0.0.1", "--dns", "1.1.1.1,9.9.9.9"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let parsed = parse_static_args(&args).unwrap();
        assert_eq!(parsed.address.as_deref(), Some("10.0.0.5/24"));
        assert_eq!(parsed.gateway.as_deref(), Some("10.0.0.1"));
        assert_eq!(parsed.dns, vec!["1.1.1.1", "9.9.9.9"]);
        assert!(parse_static_args(&["--bogus".to_string()]).is_err());
        assert!(parse_static_args(&["--address".to_string()]).is_err());
    }

    #[test]
    fn uptime_and_meminfo() {
        assert_eq!(format_uptime(59), "0m");
        assert_eq!(format_uptime(3_700), "1h 1m");
        assert_eq!(format_uptime(90_000), "1d 1h 0m");
        let meminfo = "MemTotal:        2048000 kB\nMemAvailable:    1024000 kB\n";
        assert_eq!(meminfo_kb(meminfo, "MemTotal:"), Some(2_048_000));
        assert_eq!(meminfo_kb(meminfo, "Nope:"), None);
    }
}
