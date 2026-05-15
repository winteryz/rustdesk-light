use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=RDL_BUILD_VERSION");
    println!("cargo:rerun-if-env-changed=RDL_BUILD_GIT_TAG");

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set");
    let workspace_root = Path::new(&manifest_dir).join("../..");
    watch_git_metadata(&workspace_root);

    let version = std::env::var("RDL_BUILD_VERSION")
        .ok()
        .or_else(|| std::env::var("RDL_BUILD_GIT_TAG").ok())
        .or_else(|| git_output(&workspace_root, &["describe", "--tags", "--exact-match"]));

    emit_env("RDL_BUILD_VERSION", version.as_deref());
}

fn watch_git_metadata(workspace_root: &Path) {
    let git_path = workspace_root.join(".git");
    println!("cargo:rerun-if-changed={}", git_path.display());

    let head_path = git_path.join("HEAD");
    println!("cargo:rerun-if-changed={}", head_path.display());

    let Some(head_ref) = std::fs::read_to_string(&head_path).ok().and_then(|head| {
        head.strip_prefix("ref: ")
            .map(|value| value.trim().to_string())
    }) else {
        return;
    };

    println!(
        "cargo:rerun-if-changed={}",
        git_path.join(head_ref).display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        git_path.join("packed-refs").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        git_path.join("refs/tags").display()
    );
}

fn git_output(workspace_root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(workspace_root)
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    String::from_utf8(output.stdout)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn emit_env(name: &str, value: Option<&str>) {
    if let Some(value) = value {
        println!("cargo:rustc-env={name}={value}");
    }
}
