pub const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn build_version() -> Option<&'static str> {
    option_env!("RDL_BUILD_VERSION").filter(|value| !value.is_empty())
}

pub fn display_version() -> String {
    build_version()
        .map(str::to_string)
        .unwrap_or_else(|| format!("v{PACKAGE_VERSION}"))
}

pub fn app_version(app_name: &str) -> String {
    format!("{app_name} {}", display_version())
}
