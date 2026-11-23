//! getent : interroge les bases passwd, group, shadow, gshadow, hosts,
//! services, protocols et networks.
//!
//! Les scripts de mainteneur Debian l'utilisent pour tester l'existence d'un
//! compte (`getent passwd postgres`, `getent group ssl-cert`). Elle fait
//! normalement partie de libc-bin, que Senbit ne peut pas installer (libc-bin
//! exige la glibc de Debian, Senbit utilise celle de la machine de build).
//!
//! Lit /etc (ou $SENBIT_ETC) ; `hosts` interroge aussi le DNS.
//! Codes de sortie de getent : 0 trouvé, 1 usage, 2 clé introuvable, 3 base
//! non énumérable.

use std::fs;
use std::net::ToSocketAddrs;
use std::path::PathBuf;
use std::process::ExitCode;

fn etc(file: &str) -> PathBuf {
    std::env::var_os("SENBIT_ETC")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/etc"))
        .join(file)
}

fn read_lines(file: &str) -> Vec<String> {
    fs::read_to_string(etc(file))
        .map(|t| t.lines().map(|l| l.to_string()).collect())
        .unwrap_or_default()
}

/// Lignes de passwd/group/shadow/gshadow dont le nom (champ 1) ou l'identifiant
/// numérique (champ 3, passwd et group seulement) correspond à `key`.
fn colon_matches(line: &str, key: &str, id_field: bool) -> bool {
    let fields: Vec<&str> = line.split(':').collect();
    if fields.first().copied() == Some(key) {
        return true;
    }
    id_field && key.chars().all(|c| c.is_ascii_digit()) && fields.get(2).copied() == Some(key)
}

fn colon_database(file: &str, id_field: bool, keys: &[String]) -> u8 {
    let lines: Vec<String> = read_lines(file)
        .into_iter()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#') && !l.starts_with('+'))
        .collect();

    if keys.is_empty() {
        for line in &lines {
            println!("{line}");
        }
        return 0;
    }

    let mut status = 0;
    for key in keys {
        match lines.iter().find(|l| colon_matches(l, key, id_field)) {
            Some(line) => println!("{line}"),
            None => status = 2,
        }
    }
    status
}

/// Lignes de /etc/hosts : (adresse, [noms]).
fn hosts_file() -> Vec<(String, Vec<String>)> {
    read_lines("hosts")
        .iter()
        .filter_map(|line| {
            let line = line.split('#').next().unwrap_or("").trim();
            let mut parts = line.split_whitespace();
            let address = parts.next()?.to_string();
            let names: Vec<String> = parts.map(|n| n.to_string()).collect();
            if names.is_empty() {
                None
            } else {
                Some((address, names))
            }
        })
        .collect()
}

fn print_host(address: &str, names: &[String]) {
    println!("{:<15} {}", address, names.join(" "));
}

fn hosts_database(keys: &[String]) -> u8 {
    let table = hosts_file();

    if keys.is_empty() {
        for (address, names) in &table {
            print_host(address, names);
        }
        return 0;
    }

    let mut status = 0;
    for key in keys {
        // 1. /etc/hosts : par nom ou par adresse.
        if let Some((address, names)) = table
            .iter()
            .find(|(address, names)| address == key || names.iter().any(|n| n == key))
        {
            print_host(address, names);
            continue;
        }
        // 2. DNS (nom seulement).
        match (key.as_str(), 0).to_socket_addrs() {
            Ok(mut addrs) => match addrs.next() {
                Some(addr) => print_host(&addr.ip().to_string(), std::slice::from_ref(key)),
                None => status = 2,
            },
            Err(_) => status = 2,
        }
    }
    status
}

/// `getent ahosts` : une ligne par adresse, au format `adresse STREAM nom`.
fn ahosts_database(keys: &[String], v4: bool, v6: bool) -> u8 {
    if keys.is_empty() {
        return 3;
    }
    let mut status = 0;
    for key in keys {
        match (key.as_str(), 0).to_socket_addrs() {
            Ok(addrs) => {
                let mut any = false;
                for addr in addrs {
                    let wanted = (addr.is_ipv4() && v4) || (addr.is_ipv6() && v6);
                    if wanted {
                        println!("{:<15} STREAM {}", addr.ip(), key);
                        any = true;
                    }
                }
                if !any {
                    status = 2;
                }
            }
            Err(_) => status = 2,
        }
    }
    status
}

/// /etc/services et /etc/protocols : `nom numéro[/proto] [alias...]`.
fn name_number_database(file: &str, keys: &[String]) -> u8 {
    let entries: Vec<(String, String, Vec<String>)> = read_lines(file)
        .iter()
        .filter_map(|line| {
            let line = line.split('#').next().unwrap_or("").trim();
            let mut parts = line.split_whitespace();
            let name = parts.next()?.to_string();
            let number = parts.next()?.to_string();
            let aliases: Vec<String> = parts.map(|a| a.to_string()).collect();
            Some((name, number, aliases))
        })
        .collect();

    let print = |name: &str, number: &str, aliases: &[String]| {
        let mut text = format!("{:<21} {}", name, number);
        for alias in aliases {
            text.push(' ');
            text.push_str(alias);
        }
        println!("{text}");
    };

    if keys.is_empty() {
        for (name, number, aliases) in &entries {
            print(name, number, aliases);
        }
        return 0;
    }

    let mut status = 0;
    for key in keys {
        let found = entries.iter().find(|(name, number, aliases)| {
            name == key
                || number == key
                || number.split('/').next() == Some(key.as_str())
                || aliases.iter().any(|a| a == key)
        });
        match found {
            Some((name, number, aliases)) => print(name, number, aliases),
            None => status = 2,
        }
    }
    status
}

fn usage() -> u8 {
    eprintln!("Usage: getent database [key ...]");
    eprintln!("Databases: passwd group shadow gshadow hosts ahosts ahostsv4 ahostsv6 services protocols networks");
    1
}

fn run(args: &[String]) -> u8 {
    let Some(database) = args.first() else {
        return usage();
    };
    let keys = &args[1..];

    match database.as_str() {
        "passwd" => colon_database("passwd", true, keys),
        "group" => colon_database("group", true, keys),
        "shadow" => colon_database("shadow", false, keys),
        "gshadow" => colon_database("gshadow", false, keys),
        "hosts" => hosts_database(keys),
        "ahosts" => ahosts_database(keys, true, true),
        "ahostsv4" => ahosts_database(keys, true, false),
        "ahostsv6" => ahosts_database(keys, false, true),
        "services" => name_number_database("services", keys),
        "protocols" => name_number_database("protocols", keys),
        "networks" => name_number_database("networks", keys),
        other => {
            eprintln!("Unknown database: {other}");
            usage();
            1
        }
    }
}

#[path = "../../_shared/hook.rs"]
mod hook;

fn main() -> ExitCode {
    hook::install();
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Options de getent ignorées : -s service, --service=..., -i.
    let mut cleaned: Vec<String> = Vec::new();
    let mut skip_next = false;
    for arg in args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg == "-s" || arg == "--service" {
            skip_next = true;
        } else if arg.starts_with("--service=") || arg == "-i" || arg == "--no-idn" {
            // ignoré
        } else {
            cleaned.push(arg);
        }
    }
    ExitCode::from(run(&cleaned))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_by_name_and_numeric_id() {
        let line = "postgres:x:105:112:PostgreSQL:/var/lib/postgresql:/bin/bash";
        assert!(colon_matches(line, "postgres", true));
        assert!(colon_matches(line, "105", true));
        assert!(!colon_matches(line, "112", true));
        assert!(!colon_matches(line, "post", true));
        // shadow : jamais de recherche par numéro
        assert!(!colon_matches(line, "105", false));
    }

    #[test]
    fn usage_without_database() {
        assert_eq!(run(&[]), 1);
        assert_eq!(run(&["nope".to_string()]), 1);
    }

    #[test]
    fn missing_key_gives_status_2() {
        let dir = std::env::temp_dir().join(format!("senbit-getent-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("passwd"), "root:x:0:0:root:/root:/bin/sh\n").unwrap();
        // SAFETY (tests) : variable lue seulement par ce binaire, test unique qui la définit.
        unsafe {
            std::env::set_var("SENBIT_ETC", &dir);
        }
        assert_eq!(run(&["passwd".to_string(), "root".to_string()]), 0);
        assert_eq!(run(&["passwd".to_string(), "0".to_string()]), 0);
        assert_eq!(run(&["passwd".to_string(), "nobody-here".to_string()]), 2);
        let _ = fs::remove_dir_all(&dir);
    }
}
