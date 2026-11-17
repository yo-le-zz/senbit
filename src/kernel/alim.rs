// src/utils/alim.rs

use libc::{LINUX_REBOOT_CMD_POWER_OFF, LINUX_REBOOT_CMD_RESTART, reboot};

pub fn do_reboot() -> ! {
    unsafe {
        reboot(LINUX_REBOOT_CMD_RESTART);
    }
    // si on revient ici, le syscall a échoué
    std::process::exit(1);
}

pub fn do_power_off() -> ! {
    unsafe {
        reboot(LINUX_REBOOT_CMD_POWER_OFF);
    }
    std::process::exit(1);
}
