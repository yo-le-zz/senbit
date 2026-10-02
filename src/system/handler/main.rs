use anyhow::Result;
use crate::system::handler::{
    halt,
    poweroff,
    reboot,
    shutdown,
};

pub enum SystemEvent {
    Shutdown,
    Reboot,
    Halt,
    Poweroff,
}

pub fn handle(event: SystemEvent) -> Result<()> {
    match event {
        SystemEvent::Shutdown => shutdown::handle(),
        SystemEvent::Reboot => reboot::handle(),
        SystemEvent::Halt => halt::handle(),
        SystemEvent::Poweroff => poweroff::handle(),
    }
}