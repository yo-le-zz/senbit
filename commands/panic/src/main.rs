#[path = "../../_shared/kernel.rs"]
mod kernel;

fn main() {
    kpanic!("Manual panic from user !")
}