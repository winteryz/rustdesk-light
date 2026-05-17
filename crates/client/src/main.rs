#[cfg(debug_assertions)]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        {
            eprintln!($($arg)*);
        }
    };
}

#[cfg(not(debug_assertions))]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        {
            let _ = format_args!($($arg)*);
        }
    };
}

mod app;
mod commands;
mod execute;
mod live_control;
mod remote_management;
mod runtime;
mod session;
mod support;
mod system_info;
mod user_interaction;
#[cfg(feature = "gui")]
mod windowing;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    app::run()
}
