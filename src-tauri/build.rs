fn main() {
    tauri_build::build();

    #[cfg(target_os = "macos")]
    build_vmm();
}

#[cfg(target_os = "macos")]
fn build_vmm() {
    use std::path::PathBuf;
    use std::process::Command;

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let package_path = manifest_dir.join("../macos/gptbot-vmm");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let dest = out_dir.join("gptbot-vmm");

    println!(
        "cargo:rerun-if-changed={}",
        package_path.join("Package.swift").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        package_path.join("Sources").display()
    );

    let status = Command::new("swift")
        .args([
            "build",
            "-c",
            "release",
            "--package-path",
            package_path.to_str().unwrap(),
        ])
        .status();

    match status {
        Ok(s) if s.success() => {
            let built = package_path.join(".build/release/gptbot-vmm");
            if built.exists() {
                std::fs::copy(&built, &dest).expect("copy gptbot-vmm to OUT_DIR");
                let entitlements = manifest_dir.join("entitlements.plist");
                if entitlements.exists() {
                    let _ = std::process::Command::new("codesign")
                        .args([
                            "-f",
                            "-s",
                            "-",
                            "--entitlements",
                            entitlements.to_str().unwrap(),
                            "--generate-entitlement-der",
                            dest.to_str().unwrap(),
                        ])
                        .status();
                }
                println!("cargo:warning=Built gptbot-vmm at {}", dest.display());
            }
        }
        Ok(s) => {
            println!(
                "cargo:warning=swift build for gptbot-vmm failed (exit {:?}); dev builds can use macos/gptbot-vmm/.build/release/gptbot-vmm",
                s.code()
            );
        }
        Err(e) => {
            println!("cargo:warning=could not run swift: {e}");
        }
    }
}
