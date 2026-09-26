pub mod panic;
pub mod alim;

// Macro du kernel panic
#[macro_export]
macro_rules! kpanic {
    ($msg:expr) => {
        $crate::kernel::panic::kpanic_impl(&$msg.to_string());
    };
    ($fmt:expr, $($args:expr),+) => {
        $crate::kernel::panic::kpanic_impl(&format!($fmt, $($args),+));
    };
}