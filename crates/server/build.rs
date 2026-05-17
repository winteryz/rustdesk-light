fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        configure_macos_build();
    }
}

fn configure_macos_build() {
    println!("cargo:rerun-if-env-changed=RDL_SKIP_MACOS_ADHOC_SIGN");
    println!("cargo:rerun-if-env-changed=RDL_MACOS_CODESIGN_IDENTIFIER");

    if std::env::var_os("RDL_SKIP_MACOS_ADHOC_SIGN").is_some() {
        return;
    }
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set");
    spawn_macos_adhoc_signer(
        "rdl-server-cli",
        "local.rust-desk-light.server.cli",
        &manifest_dir,
    );
}

fn spawn_macos_adhoc_signer(binary_name: &str, default_identifier: &str, manifest_dir: &str) {
    let out_dir = match std::env::var_os("OUT_DIR") {
        Some(value) => std::path::PathBuf::from(value),
        None => return,
    };
    let Some(profile_dir) = cargo_profile_dir(&out_dir) else {
        return;
    };
    let binary = profile_dir.join(binary_name);
    let identifier = std::env::var("RDL_MACOS_CODESIGN_IDENTIFIER")
        .unwrap_or_else(|_| default_identifier.to_string());
    let watch_stamp = out_dir.join("macos-adhoc-codesign.watch");
    let log_path = out_dir.join("macos-adhoc-codesign.log");
    let script = std::path::Path::new(manifest_dir)
        .join("../..")
        .join("scripts/macos-adhoc-sign-after-link.sh");
    let _ = std::fs::write(&watch_stamp, "watch\n");
    println!("cargo:rerun-if-changed={}", script.display());
    println!("cargo:rerun-if-changed={}", watch_stamp.display());

    let _ = std::process::Command::new("/bin/sh")
        .arg(script)
        .arg(binary)
        .arg(identifier)
        .arg(watch_stamp)
        .arg(log_path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

fn cargo_profile_dir(out_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    for ancestor in out_dir.ancestors() {
        if ancestor.file_name().and_then(|name| name.to_str()) == Some("build") {
            return ancestor.parent().map(std::path::Path::to_path_buf);
        }
    }
    None
}
