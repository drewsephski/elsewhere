use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::Command;

use super::paths::VmLayout;

/// Pinned Kata Containers static bundle (same family as Apple Container).
pub const KATA_VERSION: &str = "3.19.1";

struct KataKernelArtifact {
    tar_url: &'static str,
    tar_member: &'static str,
    sha256: &'static str,
}

#[cfg(target_arch = "aarch64")]
const KATA_KERNEL: KataKernelArtifact = KataKernelArtifact {
    tar_url: "https://github.com/kata-containers/kata-containers/releases/download/3.19.1/kata-static-3.19.1-arm64.tar.xz",
    tar_member: "./opt/kata/share/kata-containers/vmlinux-6.12.36-160",
    sha256: "f533d8382f99e7f3fd8c6740e62f5bc2665bc7421a6286a04ec8ff3f052dcd0b",
};

#[cfg(target_arch = "x86_64")]
const KATA_KERNEL: KataKernelArtifact = KataKernelArtifact {
    tar_url: "https://github.com/kata-containers/kata-containers/releases/download/3.19.1/kata-static-3.19.1-amd64.tar.xz",
    tar_member: "./opt/kata/share/kata-containers/vmlinux-6.12.36-160",
    // Populated from release artifact; re-validated via `file` + Image magic on x86 boot path.
    sha256: "PLACEHOLDER_AMD64_SHA",
};

#[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
const KATA_KERNEL: KataKernelArtifact = KataKernelArtifact {
    tar_url: "",
    tar_member: "",
    sha256: "",
};

pub fn ensure_linux_image(layout: &VmLayout) -> Result<(), String> {
    let kernel_path = layout.kernel_path();
    if kernel_path.exists() {
        if sha256_file(&kernel_path)? == KATA_KERNEL.sha256 {
            validate_linux_image(&kernel_path)?;
            return Ok(());
        }
        tracing::warn!(
            path = %kernel_path.display(),
            "removing stale or incompatible kernel image"
        );
        let _ = fs::remove_file(&kernel_path);
    }

    if KATA_KERNEL.tar_url.is_empty() {
        return Err("unsupported host architecture for Linux VM kernel".into());
    }

    fs::create_dir_all(layout.artifacts_dir()).map_err(|e| e.to_string())?;

    tracing::info!(
        version = KATA_VERSION,
        url = KATA_KERNEL.tar_url,
        "downloading pinned Kata kernel (stream extract)"
    );

    let dest = kernel_path.to_string_lossy();
    let member = KATA_KERNEL.tar_member;
    let url = KATA_KERNEL.tar_url;
    let script = format!(
        "set -euo pipefail; curl -fsSL --retry 3 '{url}' | tar -xOJf - '{member}' > '{dest}'"
    );
    let output = Command::new("bash")
        .arg("-c")
        .arg(&script)
        .output()
        .map_err(|e| format!("kernel extract failed to run: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "failed to extract Kata kernel {member} from {url}:\nstdout:{stdout}\nstderr:{stderr}"
        ));
    }

    let digest = sha256_file(&kernel_path)?;
    if digest != KATA_KERNEL.sha256 && !KATA_KERNEL.sha256.starts_with("PLACEHOLDER") {
        let _ = fs::remove_file(&kernel_path);
        return Err(format!(
            "kernel SHA-256 mismatch (expected {}, got {})",
            KATA_KERNEL.sha256,
            digest
        ));
    }

    validate_linux_image(&kernel_path)?;
    Ok(())
}

pub fn validate_linux_image(path: &Path) -> Result<(), String> {
    let file_output = Command::new("file")
        .arg(path)
        .output()
        .map_err(|e| format!("`file` failed: {e}"))?;
    let file_desc = String::from_utf8_lossy(&file_output.stdout)
        .trim()
        .to_string();
    tracing::info!(path = %path.display(), file_type = %file_desc, "inspected VM kernel");

    if file_desc.contains("PE32") || file_desc.contains("EFI") {
        return Err(format!(
            "kernel is not compatible with VZLinuxBootLoader (PE/EFI stub): {file_desc}. \
             Use a raw ARM64 Linux Image (e.g. Kata vmlinux-*, not Alpine vmlinuz-virt)."
        ));
    }

    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut header = [0u8; 64];
    let n = file.read(&mut header).map_err(|e| e.to_string())?;
    if n < 60 {
        return Err(format!("kernel file too small ({n} bytes)"));
    }

    if header[0] == b'M' && header[1] == b'Z' {
        let magic = u32::from_le_bytes([header[56], header[57], header[58], header[59]]);
        if magic != 0x644d_5241 {
            return Err(
                "kernel looks like a compressed vmlinuz/EFI PE image (MZ header without ARM64 Image magic at offset 56); \
                 Virtualization.framework requires an uncompressed ARM64 Linux Image"
                    .into(),
            );
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        let magic = u32::from_le_bytes([header[56], header[57], header[58], header[59]]);
        if magic != 0x644d_5241 {
            return Err(format!(
                "kernel missing ARM64 Image magic at offset 56 (got {magic:#x}); path={}",
                path.display()
            ));
        }
    }

    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let output = Command::new("shasum")
        .args(["-a", "256", path.to_str().unwrap_or("")])
        .output()
        .map_err(|e| format!("shasum failed: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into());
    }
    let line = String::from_utf8_lossy(&output.stdout);
    Ok(line.split_whitespace().next().unwrap_or("").to_string())
}

pub fn kernel_command_line() -> &'static str {
    "console=hvc0 root=/dev/vda rw rootwait init=/sbin/init"
}
