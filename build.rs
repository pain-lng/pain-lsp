// Build script for Windows icon embedding

fn main() {
    #[cfg(target_os = "windows")]
    {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .map(std::path::PathBuf::from)
            .expect("CARGO_MANIFEST_DIR is always set by Cargo");

        let local_icon = manifest_dir.join("resources/icons/windows/lsp.ico");
        let shared_icon = manifest_dir
            .parent()
            .map(|root| root.join("pain-compiler/resources/icons/windows/lsp.ico"));

        let icon_path = if local_icon.exists() {
            local_icon
        } else if let Some(shared) = shared_icon.as_ref().filter(|path| path.exists()) {
            shared.clone()
        } else {
            local_icon
        };

        if icon_path.exists() {
            let mut res = winres::WindowsResource::new();
            res.set_icon(icon_path.to_str().unwrap());
            if let Err(e) = res.compile() {
                eprintln!("cargo:warning=Failed to embed Windows icon: {}", e);
                eprintln!("cargo:warning=This is a known issue with CVTRES on some Windows setups");
                eprintln!("cargo:warning=Build will continue without icon");
                eprintln!(
                    "cargo:warning=Icon file is available at: {}",
                    icon_path.display()
                );
            }
        } else {
            println!(
                "cargo:warning=Windows icon not found ({}), skipping embed",
                icon_path.display()
            );
        }
    }
}
