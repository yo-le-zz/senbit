use anyhow::{
    bail,
    Result,
};

use crate::system::handler::common::prepare_shutdown;

pub fn handle() -> Result<()> {
    prepare_shutdown()?;

    let result =
        unsafe {
            libc::reboot(
                libc::RB_HALT_SYSTEM,
            )
        };

    if result != 0 {
        bail!(
            "halt failed: {}",
            std::io::Error::last_os_error()
        );
    }

    Ok(())
}