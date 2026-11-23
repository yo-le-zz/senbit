//! adduser / addgroup / deluser / delgroup, compatibles avec les scripts de
//! mainteneur Debian (`adduser --system --quiet --group --home /var/lib/x ... x`).
//!
//! Un seul binaire : le comportement dépend du nom sous lequel il est appelé
//! (argv[0]) ; addgroup, deluser et delgroup sont des liens créés par `devtool`
//! à partir de `aliases` dans Cargo.toml.
//!
//! Les options inconnues sont ignorées (un script ne doit pas échouer parce
//! qu'il passe `--force-badname` ou `--conf`).

#[path = "../../_shared/accounts.rs"]
mod accounts;

use std::io::IsTerminal;
use std::path::Path;
use std::process::{Command, ExitCode};

use accounts::{Accounts, User};

#[derive(Debug, Default)]
struct Opts {
    system: bool,
    group: bool,
    quiet: bool,
    disabled_password: bool,
    no_create_home: bool,
    remove_home: bool,
    only_if_empty: bool,
    help: bool,
    home: Option<String>,
    shell: Option<String>,
    gecos: Option<String>,
    uid: Option<u32>,
    gid: Option<u32>,
    ingroup: Option<String>,
    positional: Vec<String>,
}

fn parse_id(text: &str) -> Result<u32, String> {
    text.parse::<u32>()
        .map_err(|_| format!("invalid numeric id `{text}'"))
}

/// Options qui consomment une valeur (`--home /x` ou `--home=/x`).
fn takes_value(key: &str) -> bool {
    matches!(
        key,
        "home"
            | "shell"
            | "gecos"
            | "comment"
            | "uid"
            | "gid"
            | "ingroup"
            | "firstuid"
            | "lastuid"
            | "firstgid"
            | "lastgid"
            | "conf"
            | "stdoutmsglevel"
            | "stderrmsglevel"
            | "logmsglevel"
            | "backup-to"
    )
}

fn parse(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts::default();
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];
        i += 1;

        if arg == "--" {
            o.positional.extend(args[i..].iter().cloned());
            break;
        }

        if let Some(long) = arg.strip_prefix("--") {
            let (key, inline) = match long.split_once('=') {
                Some((k, v)) => (k.replace('_', "-"), Some(v.to_string())),
                None => (long.replace('_', "-"), None),
            };

            let value: String = if takes_value(&key) {
                match inline {
                    Some(v) => v,
                    None => {
                        let v = args
                            .get(i)
                            .cloned()
                            .ok_or_else(|| format!("option --{key} needs a value"))?;
                        i += 1;
                        v
                    }
                }
            } else {
                String::new()
            };

            match key.as_str() {
                "system" => o.system = true,
                "group" => o.group = true,
                "quiet" => o.quiet = true,
                "disabled-password" | "disabled-login" => o.disabled_password = true,
                "no-create-home" => o.no_create_home = true,
                "remove-home" | "remove-all-files" => o.remove_home = true,
                "only-if-empty" => o.only_if_empty = true,
                "help" => o.help = true,
                "home" => o.home = Some(value),
                "shell" => o.shell = Some(value),
                "gecos" | "comment" => o.gecos = Some(value),
                "uid" => o.uid = Some(parse_id(&value)?),
                "gid" => o.gid = Some(parse_id(&value)?),
                "ingroup" => o.ingroup = Some(value),
                _ => {}
            }
        } else if arg.starts_with('-') && arg.len() > 1 {
            for c in arg[1..].chars() {
                match c {
                    'q' => o.quiet = true,
                    'h' => o.help = true,
                    _ => {}
                }
            }
        } else {
            o.positional.push(arg.clone());
        }
    }

    Ok(o)
}

fn say(o: &Opts, message: String) {
    if !o.quiet {
        println!("{message}");
    }
}

fn io_err(e: std::io::Error) -> String {
    format!("cannot read or write the account files: {e}")
}

fn exists(path: &str) -> bool {
    Path::new(path).exists()
}

fn default_shell(system: bool) -> String {
    if system {
        if exists("/usr/sbin/nologin") {
            "/usr/sbin/nologin".to_string()
        } else {
            "/bin/false".to_string()
        }
    } else if exists("/bin/bash") {
        "/bin/bash".to_string()
    } else {
        "/bin/sh".to_string()
    }
}

/// Le GECOS ne peut contenir ni `:` ni saut de ligne.
fn clean_gecos(text: &str) -> String {
    text.replace([':', '\n'], " ")
}

fn add_user(o: &Opts) -> Result<(), String> {
    let name = &o.positional[0];
    if !accounts::valid_name(name) {
        return Err(format!("`{name}' is not a valid user name."));
    }

    let mut acc = Accounts::load().map_err(io_err)?;

    if acc.user(name).is_some() {
        if o.system {
            say(o, format!("The system user `{name}' already exists. Exiting."));
            return Ok(());
        }
        return Err(format!("The user `{name}' already exists."));
    }

    let uid = match o.uid {
        Some(uid) => {
            if acc.uid_in_use(uid) {
                return Err(format!("The UID {uid} is already in use."));
            }
            uid
        }
        None => acc
            .free_uid(o.system)
            .ok_or_else(|| "no free UID left in the range".to_string())?,
    };

    // Primary group: --ingroup / --gid (must exist), nogroup for a system user
    // without --group, otherwise a group named like the user (created if needed).
    let mut create_group: Option<u32> = None;
    let gid: u32 = if let Some(g) = &o.ingroup {
        acc.group_named(g)
            .map(|g| g.gid)
            .ok_or_else(|| format!("The group `{g}' does not exist."))?
    } else if let Some(g) = o.gid {
        acc.group_by_gid(g)
            .map(|g| g.gid)
            .ok_or_else(|| format!("The GID {g} does not exist."))?
    } else if o.system && !o.group && acc.group_named("nogroup").is_some() {
        acc.group_named("nogroup").map(|g| g.gid).unwrap_or(65534)
    } else if let Some(existing) = acc.group_named(name) {
        existing.gid
    } else {
        let gid = if !acc.gid_in_use(uid) {
            uid
        } else {
            acc.free_gid(o.system)
                .ok_or_else(|| "no free GID left in the range".to_string())?
        };
        create_group = Some(gid);
        gid
    };

    let home = o
        .home
        .clone()
        .unwrap_or_else(|| format!("/home/{name}"));
    let shell = o
        .shell
        .clone()
        .unwrap_or_else(|| default_shell(o.system));

    if let Some(new_gid) = create_group {
        acc.add_group(name, new_gid, &[]);
        say(o, format!("Adding new group `{name}' (GID {new_gid}) ..."));
    }

    let user = User {
        name: name.clone(),
        passwd: "x".to_string(),
        uid,
        gid,
        gecos: clean_gecos(o.gecos.as_deref().unwrap_or("")),
        home: home.clone(),
        shell,
    };
    // "*" : compte système sans mot de passe ; "!" : compte verrouillé.
    acc.add_user(&user, if o.system { "*" } else { "!" });
    acc.save().map_err(io_err)?;

    if o.system {
        say(o, format!("Adding system user `{name}' (UID {uid}) ..."));
    } else {
        say(o, format!("Adding user `{name}' (UID {uid}) ..."));
    }

    // Home: created for a normal user; for a system user only when --home is
    // given (a package asking for a home wants it to exist).
    let create_home = !o.no_create_home && (!o.system || o.home.is_some());
    if create_home && !Path::new(&home).exists() {
        let mode = if o.system { 0o755 } else { 0o700 };
        let skel = if o.system { None } else { Some(Path::new("/etc/skel")) };
        accounts::create_home(&home, uid, gid, mode, skel)
            .map_err(|e| format!("cannot create the home directory `{home}': {e}"))?;
        say(o, format!("Creating home directory `{home}' ..."));
    }

    // Normal user on a terminal: ask for the password like Debian's adduser.
    if !o.system && !o.disabled_password && std::io::stdin().is_terminal() {
        let _ = Command::new("passwd").arg(name).status();
    }

    Ok(())
}

fn add_group(o: &Opts) -> Result<(), String> {
    let name = &o.positional[0];
    if !accounts::valid_name(name) {
        return Err(format!("`{name}' is not a valid group name."));
    }

    let mut acc = Accounts::load().map_err(io_err)?;

    if acc.group_named(name).is_some() {
        if o.system {
            say(o, format!("The group `{name}' already exists as a system group. Exiting."));
            return Ok(());
        }
        return Err(format!("The group `{name}' already exists."));
    }

    let gid = match o.gid {
        Some(gid) => {
            if acc.gid_in_use(gid) {
                return Err(format!("The GID {gid} is already in use."));
            }
            gid
        }
        None => acc
            .free_gid(o.system)
            .ok_or_else(|| "no free GID left in the range".to_string())?,
    };

    acc.add_group(name, gid, &[]);
    acc.save().map_err(io_err)?;
    say(o, format!("Adding group `{name}' (GID {gid}) ..."));
    Ok(())
}

fn add_member(o: &Opts) -> Result<(), String> {
    let user = &o.positional[0];
    let group = &o.positional[1];

    let mut acc = Accounts::load().map_err(io_err)?;
    if acc.user(user).is_none() {
        return Err(format!("The user `{user}' does not exist."));
    }
    if acc.group_named(group).is_none() {
        return Err(format!("The group `{group}' does not exist."));
    }

    if acc.add_member(user, group) {
        acc.save().map_err(io_err)?;
        say(o, format!("Adding user `{user}' to group `{group}' ..."));
    } else {
        say(o, format!("The user `{user}' is already a member of `{group}'."));
    }
    Ok(())
}

fn del_user(o: &Opts) -> Result<(), String> {
    let name = &o.positional[0];
    let mut acc = Accounts::load().map_err(io_err)?;

    let Some(user) = acc.user(name) else {
        if o.system {
            return Ok(());
        }
        return Err(format!("The user `{name}' does not exist."));
    };

    if o.system && user.uid >= accounts::USER_ID_MIN {
        say(o, format!("The user `{name}' is not a system user. Exiting."));
        return Ok(());
    }
    if user.uid == 0 {
        return Err("WARNING: You are just about to delete the root account (uid 0). Not done.".to_string());
    }

    acc.remove_from_all_groups(name);
    acc.remove_user(name);

    // The user's own group goes too when it is empty and nobody else uses it.
    if let Some(group) = acc.group_by_gid(user.gid) {
        let used = acc.users().iter().any(|u| u.gid == group.gid);
        if group.name == *name && group.members.is_empty() && !used {
            acc.remove_group(&group.name);
        }
    }

    acc.save().map_err(io_err)?;
    say(o, format!("Removing user `{name}' ..."));

    if o.remove_home {
        accounts::remove_home(&user.home)
            .map_err(|e| format!("cannot remove the home directory `{}': {e}", user.home))?;
    }
    Ok(())
}

fn del_member(o: &Opts) -> Result<(), String> {
    let user = &o.positional[0];
    let group = &o.positional[1];

    let mut acc = Accounts::load().map_err(io_err)?;
    if acc.user(user).is_none() {
        return Err(format!("The user `{user}' does not exist."));
    }
    if acc.group_named(group).is_none() {
        return Err(format!("The group `{group}' does not exist."));
    }

    if acc.remove_member(user, group) {
        acc.save().map_err(io_err)?;
        say(o, format!("Removing user `{user}' from group `{group}' ..."));
    } else {
        say(o, format!("The user `{user}' is not a member of group `{group}'."));
    }
    Ok(())
}

fn del_group(o: &Opts) -> Result<(), String> {
    let name = &o.positional[0];
    let mut acc = Accounts::load().map_err(io_err)?;

    let Some(group) = acc.group_named(name) else {
        if o.system {
            return Ok(());
        }
        return Err(format!("The group `{name}' does not exist."));
    };

    if let Some(user) = acc.users().iter().find(|u| u.gid == group.gid) {
        return Err(format!(
            "`{}' still has `{}' as their primary group!",
            user.name, name
        ));
    }
    if o.only_if_empty && !group.members.is_empty() {
        say(o, format!("The group `{name}' is not empty. Not removed."));
        return Ok(());
    }

    acc.remove_group(name);
    acc.save().map_err(io_err)?;
    say(o, format!("Removing group `{name}' ..."));
    Ok(())
}

fn usage(program: &str) -> String {
    match program {
        "addgroup" => "usage: addgroup [--system] [--gid ID] group | addgroup user group".to_string(),
        "deluser" => "usage: deluser [--system] [--remove-home] user | deluser user group".to_string(),
        "delgroup" => "usage: delgroup [--system] [--only-if-empty] group".to_string(),
        _ => "usage: adduser [--system] [--group] [--home DIR] [--shell SHELL] [--gecos TEXT]\n\
              \x20              [--uid ID] [--gid ID] [--ingroup GROUP] [--no-create-home]\n\
              \x20              [--disabled-password] [--quiet] user\n\
              \x20      adduser user group"
            .to_string(),
    }
}

fn run(program: &str, o: &Opts) -> Result<(), String> {
    let count = o.positional.len();

    match program {
        "addgroup" => match count {
            1 => add_group(o),
            2 => add_member(o),
            _ => Err(usage(program)),
        },
        "delgroup" => match count {
            1 => del_group(o),
            _ => Err(usage(program)),
        },
        "deluser" => match (count, o.group) {
            (1, true) => del_group(o),
            (1, false) => del_user(o),
            (2, _) => del_member(o),
            _ => Err(usage(program)),
        },
        // adduser
        _ => match (count, o.group) {
            (1, true) => add_group(o),
            (1, false) => add_user(o),
            (2, _) => add_member(o),
            _ => Err(usage(program)),
        },
    }
}

#[path = "../../_shared/hook.rs"]
mod hook;

fn main() -> ExitCode {
    hook::install();
    let mut args: Vec<String> = std::env::args().collect();
    let program = if args.is_empty() {
        "adduser".to_string()
    } else {
        Path::new(&args.remove(0))
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "adduser".to_string())
    };

    let opts = match parse(&args) {
        Ok(opts) => opts,
        Err(e) => {
            eprintln!("{program}: {e}");
            return ExitCode::from(1);
        }
    };

    if opts.help {
        println!("{}", usage(&program));
        return ExitCode::SUCCESS;
    }

    if !accounts::is_root() {
        eprintln!("{program}: Only root may add or remove users and groups.");
        return ExitCode::from(1);
    }

    match run(&program, &opts) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{program}: {e}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_a_typical_maintainer_script_call() {
        let o = parse(&args(&[
            "--system",
            "--quiet",
            "--home",
            "/var/lib/postgresql",
            "--no-create-home",
            "--shell=/bin/bash",
            "--gecos",
            "PostgreSQL administrator",
            "--ingroup",
            "postgres",
            "--force-badname",
            "postgres",
        ]))
        .unwrap();

        assert!(o.system && o.quiet && o.no_create_home);
        assert_eq!(o.home.as_deref(), Some("/var/lib/postgresql"));
        assert_eq!(o.shell.as_deref(), Some("/bin/bash"));
        assert_eq!(o.gecos.as_deref(), Some("PostgreSQL administrator"));
        assert_eq!(o.ingroup.as_deref(), Some("postgres"));
        assert_eq!(o.positional, vec!["postgres"]);
    }

    #[test]
    fn parses_ids_and_underscore_options() {
        let o = parse(&args(&["--uid", "150", "--gid=150", "--add_extra_groups", "--group", "x"])).unwrap();
        assert_eq!(o.uid, Some(150));
        assert_eq!(o.gid, Some(150));
        assert!(o.group);
        assert_eq!(o.positional, vec!["x"]);
    }

    #[test]
    fn missing_value_and_bad_id_are_errors() {
        assert!(parse(&args(&["--home"])).is_err());
        assert!(parse(&args(&["--uid", "abc", "x"])).is_err());
    }

    #[test]
    fn two_positionals_mean_add_to_group() {
        let o = parse(&args(&["alice", "staff"])).unwrap();
        assert_eq!(o.positional.len(), 2);
    }

    #[test]
    fn gecos_is_sanitized() {
        assert_eq!(clean_gecos("A:B\nC"), "A B C");
    }
}
