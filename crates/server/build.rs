use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let ui_index = manifest_dir.join("../../ui/dist/index.html");
    if !ui_index.exists() {
        panic!(
            "Orbitry UI is not built. Run `npm run build` in the ui/ folder, then rebuild the server."
        );
    }
    println!(
        "cargo:rerun-if-changed={}",
        ui_index.parent().unwrap().display()
    );
    println!("cargo:rerun-if-changed=assets/orbitry.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/orbitry.ico");
        res.set("ProductName", "Orbitry");
        res.set(
            "FileDescription",
            "A local command center for every app in orbit.",
        );
        res.set("LegalCopyright", "Copyright (c) 2026 Piotr Francug");
        if let Err(err) = res.compile() {
            println!("cargo:warning=could not embed the exe icon: {err}");
        }
    }
}
