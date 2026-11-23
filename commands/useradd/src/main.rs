//! useradd / groupadd / userdel / groupdel / usermod, compatibles avec les
//! options courantes de shadow-utils utilisées par les scripts de mainteneur.
//!
//! Un seul binaire : le comportement dépend du nom d'appel (argv[0]) ;
//! groupadd, userdel, groupdel et usermod sont des liens créés par `devtool`
//! (`aliases` dans Cargo.toml). Codes de sortie de shadow-utils : 2 syntaxe,
//! 3 argument invalide, 4 UID/GID déjà utilisé, 6 groupe ou utilisateur
//! inexistant, 8 groupe primaire d'un utilisateur, 9 nom déjà utilisé.
//!
//! Les options inconnues sont ignorées.

#[path = "../../_shared/accounts.rs"]
mod accounts;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::ExitCode;

use accounts::{Accounts, User};

type Fail = (u8, String);

#[derive(Debug, Default)]
struct Opts {
    flags: BTreeSet<char>,
    values: BTreeMap<char, String>,
    positional: Vec<String>,
    help: bool,
}

impl Opts {
    fn flag(&self, c: char) -> bool {
        self.flags.contains(&c)
    }

    fn value(&self, c: char) -> Option<&str> {
        self.values.get(&c).map(|v| v.as_str())
    }
}

/// Options courtes qui consomment une valeur, selon la commande.
fn takes_value(program: &str, c: char) -> bool {
    match c {
        'c' | 'd' | 'e' | 'g' | 'G' | 'k' | 'K' | 'p' | 's' | 'u' | 'b' | 'R' | 'P' | 'Z' => true,
        // useradd/usermod -f DAYS ; userdel/groupadd -f est un drapeau.
        'f' => matches!(program, "useradd" | "usermod"),
        // usermod -l NEW_LOGIN ; useradd -l est un drapeau.
        'l' => program == "usermod",
        _ => false,
    }
}

/// Option longue -> option courte équivalente.
fn long_to_short(name: &str) -> Option<char> {
    Some(match name {
        "system" | "remove" => 'r',
        "create-home" => 'm',
        "no-create-home" => 'M',
        "home-dir" | "home" => 'd',
        "shell" => 's',
        "gid" => 'g',
        "groups" => 'G',
        "uid" => 'u',
        "comment" => 'c',
        "user-group" => 'U',
        "no-user-group" => 'N',
        "non-unique" => 'o',
        "skel" => 'k',
        "password" => 'p',
        "expiredate" => 'e',
        "inactive" | "force" => 'f',
        "append" => 'a',
        "lock" => 'L',
        "unlock" => 'U',
        "login" | "new-name" => 'l',
        "move-home" => 'm',
        _ => return None,
    })
}

fn parse(program: &str, args: &[String]) -> Result<Opts, String> {
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
                Some((k, v)) => (k, Some(v.to_string())),
                None => (long, None),
            };
            if key == "help" {
                o.help = true;
                continue;
            }
            let Some(c) = long_to_short(key) else {
                continue; // inconnue : ignorée
            };
            if takes_value(program, c) {
                let value = match inline {
                    Some(v) => v,
                    None => {
                        let v = args
                            .get(i)
                            .cloned()
                            .ok_or_else(|| format!("option --{key} needs an argument"))?;
                        i += 1;
                        v
                    }
                };
                o.values.insert(c, value);
            } else {
                o.flags.insert(c);
            }
        } else if arg.starts_with('-') && arg.len() > 1 {
            let chars: Vec<char> = arg[1..].chars().collect();
            let mut k = 0;
            while k < chars.len() {
                let c = chars[k];
                k += 1;
                if takes_value(program, c) {
                    let rest: String = chars[k..].iter().collect();
                    let value = if !rest.is_empty() {
                        rest
                    } else {
                        let v = args
                            .get(i)
                            .cloned()
                            .ok_or_else(|| format!("option -{c} needs an argument"))?;
                        i += 1;
                        v
                    };
                    o.values.insert(c, value);
                    break;
                } else if c == 'h' {
                    o.help = true;
                } else {
                    o.flags.insert(c);
                }
            }
        } else {
            o.positional.push(arg.clone());
        }
    }

    Ok(o)
}

/// Erreur avec son code de sortie (le type du code est fixé ici : un littéral
/// dans une fermeture passée à `?` pourrait être déduit en i32).
fn fail(code: u8, message: impl Into<String>) -> Fail {
    (code, message.into())
}

fn io_err(e: std::io::Error) -> Fail {
    (1, format!("cannot update the account files: {e}"))
}

fn parse_num(text: &str) -> Result<u32, Fail> {
    text.parse::<u32>()
        .map_err(|_| fail(3, format!("invalid numeric argument '{text}'")))
}

/// Groupe désigné par un nom ou un GID.
fn find_group(acc: &Accounts, spec: &str) -> Result<accounts::Group, Fail> {
    let found = match spec.parse::<u32>() {
        Ok(gid) => acc.group_by_gid(gid).or_else(|| acc.group_named(spec)),
        Err(_) => acc.group_named(spec),
    };
    found.ok_or_else(|| fail(6, format!("group '{spec}' does not exist")))
}

/// Remplace le hash (2e champ) de l'entrée shadow de `name`.
fn set_shadow_hash(acc: &mut Accounts, name: &str, hash: &str) {
    if let Some(i) = acc.shadow.find(name) {
        let mut fields: Vec<String> = acc.shadow.lines[i].split(':').map(|f| f.to_string()).collect();
        while fields.len() < 9 {
            fields.push(String::new());
        }
        fields[1] = hash.to_string();
        acc.shadow.lines[i] = fields.join(":");
    }
}

fn shadow_hash(acc: &Accounts, name: &str) -> Option<String> {
    let i = acc.shadow.find(name)?;
    acc.shadow.lines[i].split(':').nth(1).map(|h| h.to_string())
}

fn groups_list(acc: &Accounts, list: &str) -> Result<Vec<String>, Fail> {
    let mut names = Vec::new();
    for spec in list.split(',').filter(|s| !s.is_empty()) {
        names.push(find_group(acc, spec)?.name);
    }
    Ok(names)
}

fn useradd(o: &Opts) -> Result<(), Fail> {
    if o.positional.len() != 1 {
        return Err((2, usage("useradd")));
    }
    let name = &o.positional[0];
    if !accounts::valid_name(name) {
        return Err((3, format!("invalid user name '{name}'")));
    }

    let mut acc = Accounts::load().map_err(io_err)?;
    if acc.user(name).is_some() {
        return Err((9, format!("user '{name}' already exists")));
    }

    let system = o.flag('r');

    let uid = match o.value('u') {
        Some(text) => {
            let uid = parse_num(text)?;
            if acc.uid_in_use(uid) && !o.flag('o') {
                return Err((4, format!("UID {uid} is not unique")));
            }
            uid
        }
        None => acc
            .free_uid(system)
            .ok_or_else(|| fail(1, "no free UID left in the range"))?,
    };

    // Primary group.
    let mut create_group: Option<u32> = None;
    let gid = if let Some(spec) = o.value('g') {
        find_group(&acc, spec)?.gid
    } else if o.flag('N') {
        find_group(&acc, "users")?.gid
    } else if let Some(existing) = acc.group_named(name) {
        existing.gid
    } else {
        let gid = if !acc.gid_in_use(uid) {
            uid
        } else {
            acc.free_gid(system)
                .ok_or_else(|| fail(1, "no free GID left in the range"))?
        };
        create_group = Some(gid);
        gid
    };

    let supplementary = match o.value('G') {
        Some(list) => groups_list(&acc, list)?,
        None => Vec::new(),
    };

    let home = o
        .value('d')
        .map(|h| h.to_string())
        .unwrap_or_else(|| format!("/home/{name}"));
    let shell = o.value('s').unwrap_or("/bin/sh").to_string();
    let gecos = o.value('c').unwrap_or("").replace([':', '\n'], " ");

    if let Some(new_gid) = create_group {
        acc.add_group(name, new_gid, &[]);
    }
    let user = User {
        name: name.clone(),
        passwd: "x".to_string(),
        uid,
        gid,
        gecos,
        home: home.clone(),
        shell,
    };
    acc.add_user(&user, o.value('p').unwrap_or("!"));
    for group in &supplementary {
        acc.add_member(name, group);
    }
    acc.save().map_err(io_err)?;

    if o.flag('m') && !Path::new(&home).exists() {
        let skel = o.value('k').unwrap_or("/etc/skel");
        let skel = if system { None } else { Some(Path::new(skel)) };
        accounts::create_home(&home, uid, gid, 0o755, skel)
            .map_err(|e| fail(12, format!("cannot create directory {home}: {e}")))?;
    }
    Ok(())
}

fn groupadd(o: &Opts) -> Result<(), Fail> {
    if o.positional.len() != 1 {
        return Err((2, usage("groupadd")));
    }
    let name = &o.positional[0];
    if !accounts::valid_name(name) {
        return Err((3, format!("invalid group name '{name}'")));
    }

    let mut acc = Accounts::load().map_err(io_err)?;
    if acc.group_named(name).is_some() {
        if o.flag('f') {
            return Ok(());
        }
        return Err((9, format!("group '{name}' already exists")));
    }

    let system = o.flag('r');
    let gid = match o.value('g') {
        Some(text) => {
            let gid = parse_num(text)?;
            if acc.gid_in_use(gid) && !o.flag('o') {
                if o.flag('f') {
                    // -f : choisir un autre GID plutôt que d'échouer.
                    acc.free_gid(system)
                        .ok_or_else(|| fail(1, "no free GID left in the range"))?
                } else {
                    return Err((4, format!("GID '{gid}' already exists")));
                }
            } else {
                gid
            }
        }
        None => acc
            .free_gid(system)
            .ok_or_else(|| fail(1, "no free GID left in the range"))?,
    };

    acc.add_group(name, gid, &[]);
    acc.save().map_err(io_err)
}

fn userdel(o: &Opts) -> Result<(), Fail> {
    if o.positional.len() != 1 {
        return Err((2, usage("userdel")));
    }
    let name = &o.positional[0];
    let mut acc = Accounts::load().map_err(io_err)?;

    let Some(user) = acc.user(name) else {
        return Err((6, format!("user '{name}' does not exist")));
    };
    if user.uid == 0 {
        return Err((1, "refusing to remove the root account".to_string()));
    }

    acc.remove_from_all_groups(name);
    acc.remove_user(name);

    if let Some(group) = acc.group_by_gid(user.gid) {
        let used = acc.users().iter().any(|u| u.gid == group.gid);
        if group.name == *name && group.members.is_empty() && !used {
            acc.remove_group(&group.name);
        }
    }
    acc.save().map_err(io_err)?;

    if o.flag('r') {
        accounts::remove_home(&user.home)
            .map_err(|e| fail(12, format!("cannot remove {}: {e}", user.home)))?;
        let _ = std::fs::remove_file(format!("/var/mail/{name}"));
    }
    Ok(())
}

fn groupdel(o: &Opts) -> Result<(), Fail> {
    if o.positional.len() != 1 {
        return Err((2, usage("groupdel")));
    }
    let name = &o.positional[0];
    let mut acc = Accounts::load().map_err(io_err)?;

    let Some(group) = acc.group_named(name) else {
        return Err((6, format!("group '{name}' does not exist")));
    };
    if acc.users().iter().any(|u| u.gid == group.gid) {
        return Err((8, format!("cannot remove the primary group of a user '{name}'")));
    }

    acc.remove_group(name);
    acc.save().map_err(io_err)
}

fn usermod(o: &Opts) -> Result<(), Fail> {
    if o.positional.len() != 1 {
        return Err((2, usage("usermod")));
    }
    let name = o.positional[0].clone();
    let mut acc = Accounts::load().map_err(io_err)?;

    let Some(mut user) = acc.user(&name) else {
        return Err((6, format!("user '{name}' does not exist")));
    };

    if let Some(text) = o.value('u') {
        let uid = parse_num(text)?;
        if uid != user.uid && acc.uid_in_use(uid) && !o.flag('o') {
            return Err((4, format!("UID '{uid}' already exists")));
        }
        user.uid = uid;
    }
    if let Some(spec) = o.value('g') {
        user.gid = find_group(&acc, spec)?.gid;
    }
    if let Some(shell) = o.value('s') {
        user.shell = shell.to_string();
    }
    if let Some(comment) = o.value('c') {
        user.gecos = comment.replace([':', '\n'], " ");
    }

    let old_home = user.home.clone();
    let mut move_home: Option<(String, String)> = None;
    if let Some(home) = o.value('d') {
        user.home = home.to_string();
        if o.flag('m') && home != old_home {
            move_home = Some((old_home, home.to_string()));
        }
    }

    // Supplementary groups: -G replaces them, -a -G adds to them.
    if let Some(list) = o.value('G') {
        let wanted = groups_list(&acc, list)?;
        if !o.flag('a') {
            acc.remove_from_all_groups(&name);
        }
        for group in &wanted {
            acc.add_member(&name, group);
        }
    }

    // Password lock / unlock / replace.
    if o.flag('L') {
        if let Some(hash) = shadow_hash(&acc, &name) {
            if !hash.starts_with('!') {
                set_shadow_hash(&mut acc, &name, &format!("!{hash}"));
            }
        }
    }
    if o.flag('U') {
        if let Some(hash) = shadow_hash(&acc, &name) {
            if let Some(unlocked) = hash.strip_prefix('!') {
                // A bare "!" has no password to unlock: keep it locked.
                if !unlocked.is_empty() {
                    set_shadow_hash(&mut acc, &name, unlocked);
                }
            }
        }
    }
    if let Some(hash) = o.value('p') {
        set_shadow_hash(&mut acc, &name, hash);
    }

    // Rename (-l): passwd, shadow and every group membership.
    if let Some(new_name) = o.value('l') {
        if !accounts::valid_name(new_name) {
            return Err((3, format!("invalid user name '{new_name}'")));
        }
        if acc.user(new_name).is_some() {
            return Err((9, format!("user '{new_name}' already exists")));
        }
        let groups: Vec<String> = acc
            .groups()
            .iter()
            .filter(|g| g.members.iter().any(|m| *m == name))
            .map(|g| g.name.clone())
            .collect();
        for group in &groups {
            acc.remove_member(&name, group);
            acc.add_member(new_name, group);
        }
        if let Some(i) = acc.shadow.find(&name) {
            let rest = acc.shadow.lines[i]
                .split_once(':')
                .map(|(_, r)| r.to_string())
                .unwrap_or_default();
            acc.shadow.lines[i] = format!("{new_name}:{rest}");
        }
        // The passwd entry is rewritten below under the old name, then renamed.
        acc.set_user(&user);
        if let Some(i) = acc.passwd.find(&name) {
            user.name = new_name.to_string();
            acc.passwd.lines[i] = user.line();
        }
    } else {
        acc.set_user(&user);
    }

    acc.save().map_err(io_err)?;

    if let Some((from, to)) = move_home {
        if Path::new(&from).exists() && !Path::new(&to).exists() {
            if let Some(parent) = Path::new(&to).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::rename(&from, &to)
                .map_err(|e| fail(12, format!("cannot move {from} to {to}: {e}")))?;
        }
    }
    Ok(())
}

fn usage(program: &str) -> String {
    match program {
        "groupadd" => "usage: groupadd [-r] [-g GID] [-f] [-o] group".to_string(),
        "userdel" => "usage: userdel [-r] [-f] user".to_string(),
        "groupdel" => "usage: groupdel group".to_string(),
        "usermod" => "usage: usermod [-a] [-G groups] [-g group] [-s shell] [-d home [-m]] [-c comment]\n\
                      \x20              [-l new_name] [-u uid] [-L|-U] [-p hash] user"
            .to_string(),
        _ => "usage: useradd [-r] [-m] [-d home] [-s shell] [-g group] [-G groups] [-u uid]\n\
              \x20              [-c comment] [-N] [-o] [-k skel] [-p hash] user"
            .to_string(),
    }
}

#[path = "../../_shared/hook.rs"]
mod hook;

fn main() -> ExitCode {
    hook::install();
    let mut args: Vec<String> = std::env::args().collect();
    let program = if args.is_empty() {
        "useradd".to_string()
    } else {
        Path::new(&args.remove(0))
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "useradd".to_string())
    };

    let opts = match parse(&program, &args) {
        Ok(opts) => opts,
        Err(e) => {
            eprintln!("{program}: {e}");
            return ExitCode::from(2);
        }
    };

    if opts.help {
        println!("{}", usage(&program));
        return ExitCode::SUCCESS;
    }

    if !accounts::is_root() {
        eprintln!("{program}: Permission denied.");
        return ExitCode::from(1);
    }

    let result = match program.as_str() {
        "groupadd" => groupadd(&opts),
        "userdel" => userdel(&opts),
        "groupdel" => groupdel(&opts),
        "usermod" => usermod(&opts),
        _ => useradd(&opts),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err((code, message)) => {
            eprintln!("{program}: {message}");
            ExitCode::from(code)
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
    fn parses_short_bundles_and_values() {
        let o = parse(
            "useradd",
            &args(&["-r", "-m", "-d", "/var/lib/x", "-s/bin/false", "-G", "a,b", "x"]),
        )
        .unwrap();
        assert!(o.flag('r') && o.flag('m'));
        assert_eq!(o.value('d'), Some("/var/lib/x"));
        assert_eq!(o.value('s'), Some("/bin/false"));
        assert_eq!(o.value('G'), Some("a,b"));
        assert_eq!(o.positional, vec!["x"]);
    }

    #[test]
    fn parses_long_options() {
        let o = parse(
            "useradd",
            &args(&["--system", "--home-dir=/srv/x", "--shell", "/bin/sh", "--create-home", "x"]),
        )
        .unwrap();
        assert!(o.flag('r') && o.flag('m'));
        assert_eq!(o.value('d'), Some("/srv/x"));
        assert_eq!(o.value('s'), Some("/bin/sh"));
    }

    #[test]
    fn f_is_a_flag_for_groupadd_and_a_value_for_useradd() {
        let g = parse("groupadd", &args(&["-f", "-r", "g"])).unwrap();
        assert!(g.flag('f') && g.flag('r'));
        assert_eq!(g.positional, vec!["g"]);

        let u = parse("useradd", &args(&["-f", "30", "u"])).unwrap();
        assert_eq!(u.value('f'), Some("30"));
        assert_eq!(u.positional, vec!["u"]);
    }

    #[test]
    fn l_is_a_value_only_for_usermod() {
        let m = parse("usermod", &args(&["-l", "new", "old"])).unwrap();
        assert_eq!(m.value('l'), Some("new"));
        assert_eq!(m.positional, vec!["old"]);

        let u = parse("useradd", &args(&["-l", "x"])).unwrap();
        assert!(u.flag('l'));
        assert_eq!(u.positional, vec!["x"]);
    }

    #[test]
    fn missing_argument_is_an_error() {
        assert!(parse("useradd", &args(&["-d"])).is_err());
        assert!(parse("useradd", &args(&["--shell"])).is_err());
    }

    #[test]
    fn unknown_long_options_are_ignored() {
        let o = parse("useradd", &args(&["--badname", "x"])).unwrap();
        assert_eq!(o.positional, vec!["x"]);
    }
}
