pub mod level;
pub mod record;
pub mod rotation;
pub mod writer;
pub mod logger;

pub use level::LogLevel;
pub use logger::init_logs;

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        $crate::system::log::logger::log(
            $crate::system::log::LogLevel::Error,
            module_path!(),
            format_args!($($arg)*),
        )
    };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        $crate::system::log::logger::log(
            $crate::system::log::LogLevel::Warn,
            module_path!(),
            format_args!($($arg)*),
        )
    };
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        $crate::system::log::logger::log(
            $crate::system::log::LogLevel::Info,
            module_path!(),
            format_args!($($arg)*),
        )
    };
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        $crate::system::log::logger::log(
            $crate::system::log::LogLevel::Debug,
            module_path!(),
            format_args!($($arg)*),
        )
    };
}

#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => {
        $crate::system::log::logger::log(
            $crate::system::log::LogLevel::Trace,
            module_path!(),
            format_args!($($arg)*),
        )
    };
}