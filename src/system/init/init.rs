use anyhow::{Result, Context};

use std::env::set_var;

use crate::filesystem::fs::mount;
use crate::system::init::local::hostname::get_hostname;
use crate::system::init::local::lang::get_lang;
use crate::system::init::local::timezone::get_timezone;

use crate::logln;

use std::ffi::CString;
use std::os::unix::fs::chroot;

use std::os::unix::fs::symlink;

pub fn mount_system(
    sys_disk: &str,
    root: &str,
) -> Result<()> {
    let device =
        if sys_disk.starts_with("/dev/") {
            sys_disk.to_string()
        } else {
            format!("/dev/{}", sys_disk)
        };

    mount(
        &device,
        root,
        Some("ext4"),
    )?;

    mount(
        "proc",
        &format!("{}/proc", root),
        Some("proc"),
    )?;

    mount(
        "sysfs",
        &format!("{}/sys", root),
        Some("sysfs"),
    )?;

    mount(
        "devtmpfs",
        &format!("{}/dev", root),
        Some("devtmpfs"),
    )?;

    mount(
        "tmpfs",
        &format!("{}/run", root),
        Some("tmpfs"),
    )?;

    let var_dir =
        format!("{}/var", root);

    let var_run =
        format!("{}/var/run", root);

    std::fs::create_dir_all(
        &var_dir
    )?;

    if !std::path::Path::new(
        &var_run
    ).exists() {
        symlink(
            "/run",
            &var_run
        )?;
    }

    Ok(())
}

pub fn fstab_is_ok(root: &str) -> bool {
    std::path::Path::new(root)
        .join("etc/fstab")
        .exists()
}

pub fn init_vars(root: &str) -> Result<()> {
    logln!("Reading hostname...");
    let hostname = get_hostname(root)
        .context("Failed to get hostname")?;

    logln!("Hostname: {}", hostname);

    logln!("Reading language...");
    let lang = get_lang(root)
        .context("Failed to get language")?;

    logln!("Language: {}", lang);

    logln!("Reading timezone...");
    let timezone = get_timezone(root)
        .context("Failed to get timezone")?;

    logln!("Timezone: {}", timezone);

    logln!("Setting system environment variables...");

    unsafe {
        set_var("HOSTNAME", &hostname);
        set_var("TERM", "linux");
        set_var(
            "PATH",
            "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
        );
        set_var("LANG", &lang);
        set_var("LC_ALL", &lang);
        set_var("TZ", &timezone);
        set_var("SHELL", "/bin/sh");
        set_var("TERMINFO", "/usr/share/terminfo");
        set_var("SENBIT_VERSION", env!("CARGO_PKG_VERSION"));
        set_var("SENBIT_ROOT", root);
        set_var("PWD", "/");
        set_var("OLDPWD", "/");
    }

    logln!("System environment initialized.");

    Ok(())
}

pub fn switch_root(new_root: &str) -> Result<()> {
    logln!("Switching root to {}...", new_root);

    std::env::set_current_dir(new_root)
        .with_context(|| format!("failed to chdir to {}", new_root))?;

    let dot = CString::new(".")?;
    let slash = CString::new("/")?;
    
    let ret = unsafe {
        libc::mount(
            dot.as_ptr(),
            slash.as_ptr(),
            std::ptr::null(),
            libc::MS_MOVE,
            std::ptr::null(),
        )
    };

    if ret != 0 {
        anyhow::bail!(
            "failed to move {} to /: {}",
            new_root,
            std::io::Error::last_os_error()
        );
    }

    chroot(".").context("failed to chroot")?;
    std::env::set_current_dir("/").context("failed to chdir to /")?;

    Ok(())
}