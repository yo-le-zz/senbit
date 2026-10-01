use anyhow::{
    bail,
    Context,
    Result,
};

use crypt_sha512::{
    verify,
    Password,
};

use std::env;

use std::fs::{
    self,
    OpenOptions,
};

use std::io::{
    self,
    BufRead,
    Write,
};

use std::os::fd::AsRawFd;

use std::os::unix::fs::{
    chown,
    MetadataExt,
    PermissionsExt,
};

use std::path::Path;

use std::process::Command;

use std::time::{
    SystemTime,
    UNIX_EPOCH,
};

const TIMESTAMP_DIR: &str = "/run/sudo";
const TIMESTAMP_TIMEOUT: u64 = 300; // 5 minutes

fn current_uid() -> Result<u32> {
    let uid = unsafe {
        libc::getuid()
    };

    Ok(uid)
}

fn current_euid() -> Result<u32> {
    let uid = unsafe {
        libc::geteuid()
    };

    Ok(uid)
}

fn timestamp_path(uid: u32) -> String {
    format!(
        "{}/{}",
        TIMESTAMP_DIR,
        uid
    )
}

fn current_timestamp() -> Result<u64> {
    Ok(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs()
    )
}

fn timestamp_valid(
    uid: u32,
) -> Result<bool> {
    let timestamp_file =
        timestamp_path(uid);

    let path =
        Path::new(
            &timestamp_file,
        );

    let metadata =
        match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,

            Err(_) => {
                return Ok(false);
            }
        };

    if metadata.file_type().is_symlink() {
        return Ok(false);
    }

    if metadata.uid() != 0
        || metadata.gid() != 0
    {
        return Ok(false);
    }

    if metadata.permissions().mode() & 0o077 != 0 {
        return Ok(false);
    }

    let content =
        fs::read_to_string(path)?;

    let timestamp =
        match content.trim().parse::<u64>() {
            Ok(timestamp) => timestamp,

            Err(_) => {
                return Ok(false);
            }
        };

    let now =
        current_timestamp()?;

    Ok(
        now >= timestamp
            && now - timestamp
                < TIMESTAMP_TIMEOUT
    )
}

fn update_timestamp(
    uid: u32,
) -> Result<()> {
    let dir =
        Path::new(
            TIMESTAMP_DIR,
        );

    /*
     * Create the timestamp directory if needed.
     *
     * sudo is SUID root, so it is allowed to create
     * and own this directory.
     */
    if !dir.exists() {
        fs::create_dir_all(dir)
            .context(
                "Failed to create sudo timestamp directory",
            )?;
    }

    /*
     * Never trust a symlink here.
     */
    let metadata =
        fs::symlink_metadata(dir)?;

    if metadata.file_type().is_symlink() {
        bail!(
            "sudo timestamp directory is a symlink"
        );
    }

    /*
     * sudo runs with effective UID 0.
     *
     * Force the directory ownership to root.
     */
    chown(
        dir,
        Some(0),
        Some(0),
    )?;

    fs::set_permissions(
        dir,
        fs::Permissions::from_mode(0o700),
    )?;

    let timestamp_file =
        timestamp_path(uid);

    let path =
        Path::new(
            &timestamp_file,
        );

    /*
     * Refuse to overwrite a symlink.
     */
    if path.exists() {
        let metadata =
            fs::symlink_metadata(path)?;

        if metadata.file_type().is_symlink() {
            bail!(
                "sudo timestamp file is a symlink"
            );
        }
    }

    let timestamp =
        current_timestamp()?;

    fs::write(
        path,
        timestamp.to_string(),
    )
    .context(
        "Failed to write sudo timestamp",
    )?;

    /*
     * Timestamp files are private and root-owned.
     */
    chown(
        path,
        Some(0),
        Some(0),
    )?;

    fs::set_permissions(
        path,
        fs::Permissions::from_mode(0o600),
    )?;

    Ok(())
}

fn invalidate_timestamp(
    uid: u32,
) -> Result<()> {
    let path =
        timestamp_path(uid);

    match fs::remove_file(&path) {
        Ok(()) => {}

        Err(error)
            if error.kind()
                == io::ErrorKind::NotFound => {}

        Err(error) => {
            return Err(
                error.into()
            );
        }
    }

    Ok(())
}

fn read_root_shadow() -> Result<String> {
    let content =
        fs::read_to_string(
            "/etc/shadow",
        )
        .context(
            "Failed to read /etc/shadow",
        )?;

    for line in content.lines() {
        if let Some(hash) =
            line.strip_prefix("root:")
        {
            let hash =
                hash
                    .split(':')
                    .next()
                    .unwrap_or("");

            if hash.is_empty()
                || hash == "!"
                || hash == "*"
            {
                bail!(
                    "Root account has no usable password"
                );
            }

            return Ok(
                hash.to_string()
            );
        }
    }

    bail!(
        "Root account not found in /etc/shadow"
    );
}

fn read_password() -> Result<String> {
    let tty =
        OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")
            .context(
                "Failed to open /dev/tty",
            )?;

    let fd =
        std::os::fd::AsRawFd::as_raw_fd(
            &tty,
        );

    let mut original =
        unsafe {
            std::mem::zeroed::<libc::termios>()
        };

    if unsafe {
        libc::tcgetattr(
            fd,
            &mut original,
        )
    } != 0 {
        bail!(
            "Failed to read terminal settings"
        );
    }

    let mut hidden =
        original;

    hidden.c_lflag
        &= !libc::ECHO;

    if unsafe {
        libc::tcsetattr(
            fd,
            libc::TCSANOW,
            &hidden,
        )
    } != 0 {
        bail!(
            "Failed to disable password echo"
        );
    }

    let mut tty =
        std::io::BufReader::new(
            tty,
        );

    print!(
        "[sudo] password for root: "
    );

    io::stdout()
        .flush()?;

    let mut password =
        String::new();

    let result =
        tty.read_line(
            &mut password,
        );

    /*
     * Always restore terminal echo,
     * even if reading the password fails.
     */
    unsafe {
        libc::tcsetattr(
            fd,
            libc::TCSANOW,
            &original,
        );
    }

    println!();

    result?;

    while password.ends_with(
        &['\n', '\r'],
    ) {
        password.pop();
    }

    Ok(password)
}

fn authenticate_root() -> Result<()> {
    let stored_hash =
        read_root_shadow()?;

    let password =
        read_password()?;

    let valid =
        verify(
            Password::from(password),
            &stored_hash,
        )
        .map_err(|_| {
            anyhow::anyhow!(
                "Invalid root password"
            )
        })?;

    if !valid {
        bail!(
            "Sorry, try again."
        );
    }

    Ok(())
}

fn execute_as_root(
    args: &[String],
) -> Result<()> {
    if args.is_empty() {
        bail!(
            "sudo: no command specified"
        );
    }

    unsafe {
        if libc::setgroups(
            0,
            std::ptr::null(),
        ) != 0 {
            bail!(
                "sudo: failed to clear supplementary groups"
            );
        }

        if libc::setgid(0) != 0 {
            bail!(
                "sudo: failed to set gid to root"
            );
        }

        if libc::setuid(0) != 0 {
            bail!(
                "sudo: failed to set uid to root"
            );
        }
    }

    let status =
        Command::new(&args[0])
            .args(&args[1..])
            .status()
            .with_context(|| {
                format!(
                    "Failed to execute '{}'",
                    args[0]
                )
            })?;

    match status.code() {
        Some(code) => {
            std::process::exit(code);
        }

        None => {
            bail!(
                "Command terminated by signal"
            );
        }
    }
}

fn print_help() {
    println!(
        "Usage: sudo [OPTIONS] COMMAND [ARGS...]"
    );

    println!();
    println!("Options:");
    println!("  -v, --validate    Authenticate and refresh timestamp");
    println!("  -k, --reset-timestamp");
    println!("                    Invalidate sudo timestamp");
    println!("  -h, --help        Show this help");
    println!("  -V, --version     Show version");
}

fn print_version() {
    println!(
        "sudo (Senbit) 0.1.0"
    );
}

fn main() -> Result<()> {
    let args: Vec<String> =
        env::args()
            .skip(1)
            .collect();

    if args.is_empty() {
        print_help();
        return Ok(());
    }

    match args[0].as_str() {
        "-h" | "--help" => {
            print_help();
            return Ok(());
        }

        "-V" | "--version" => {
            print_version();
            return Ok(());
        }

        "-k" | "--reset-timestamp" => {
            let uid =
                current_uid()?;

            invalidate_timestamp(
                uid,
            )?;

            return Ok(());
        }

        "-v" | "--validate" => {
            let uid =
                current_uid()?;

            if !timestamp_valid(uid)? {
                authenticate_root()?;
            }

            update_timestamp(
                uid,
            )?;

            return Ok(());
        }

        _ => {}
    }

    let uid =
        current_uid()?;

    if !timestamp_valid(uid)? {
        authenticate_root()?;

        update_timestamp(
            uid,
        )?;
    }
    
    execute_as_root(
        &args,
    )
}