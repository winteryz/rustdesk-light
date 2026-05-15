fn main() {
    #[cfg(target_os = "macos")]
    {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set");
        let plist = std::path::Path::new(&manifest_dir).join("macos/Info.plist");
        println!(
            "cargo:rustc-link-arg-bin=rdl-client=-Wl,-sectcreate,__TEXT,__info_plist,{}",
            plist.display()
        );
        println!("cargo:rerun-if-changed={}", plist.display());
    }
}
