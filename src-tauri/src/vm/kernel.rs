use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
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
    tar_member: "opt/kata/share/kata-containers/vmlinux-6.12.36-160",
    sha256: "f533d8382f99e7f3fd8c6740e62f5bc2665bc7421a6286a04ec8ff3f052dcd0b",
};

#[cfg(target_arch = "x86_64")]
const KATA_KERNEL: KataKernelArtifact = KataKernelArtifact {
    tar_url: "https://github.com/kata-containers/kata-containers/releases/download/3.19.1/kata-static-3.19.1-amd64.tar.xz",
    tar_member: "opt/kata/share/kata-containers/vmlinux-6.12.36-160",
    sha256: "PLACEHOLDER_AMD64_SHA",
};

#[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
const KATA_KERNEL: KataKernelArtifact = KataKernelArtifact {
    tar_url: "",
    tar_member: "",
    sha256: "",
};

pub fn ensure_linux_image(layout: &VmLayout) -> Result<(), String> {
    if KATA_KERNEL.tar_url.is_empty() {
        return Err("unsupported host architecture for Linux VM kernel".into());
    }

    fs::create_dir_all(layout.artifacts_dir()).map_err(|e| e.to_string())?;

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

    let mut last_err: Option<String> = None;
    for attempt in 0..2 {
        if attempt > 0 {
            tracing::warn!("retrying Kata kernel download after corrupt or incomplete bundle");
            let _ = fs::remove_file(kata_tarball_path(layout));
        }
        let tarball = match ensure_kata_tarball(layout) {
            Ok(t) => t,
            Err(e) => {
                last_err = Some(e);
                continue;
            }
        };
        match extract_kernel_from_tarball(layout, &tarball) {
            Ok(()) => break,
            Err(e) => {
                last_err = Some(e);
                let _ = fs::remove_file(&tarball);
                if attempt == 1 {
                    return Err(last_err.unwrap_or_else(|| "kernel extract failed".into()));
                }
            }
        }
    }

    if !kernel_path.exists() {
        return Err(
            last_err.unwrap_or_else(|| "failed to download and extract Kata kernel image".into())
        );
    }

    let digest = sha256_file(&kernel_path)?;
    if digest != KATA_KERNEL.sha256 && !KATA_KERNEL.sha256.starts_with("PLACEHOLDER") {
        let _ = fs::remove_file(&kernel_path);
        return Err(format!(
            "kernel SHA-256 mismatch (expected {}, got {})",
            KATA_KERNEL.sha256, digest
        ));
    }

    validate_linux_image(&kernel_path)?;
    Ok(())
}

fn kata_tarball_path(layout: &VmLayout) -> PathBuf {
    layout
        .artifacts_dir()
        .join(format!("kata-static-{KATA_VERSION}.tar.xz"))
}

fn ensure_kata_tarball(layout: &VmLayout) -> Result<PathBuf, String> {
    let path = kata_tarball_path(layout);
    if path.exists() && path.metadata().map(|m| m.len()).unwrap_or(0) > 50 * 1024 * 1024 {
        if verify_tarball_integrity(&path).is_ok() {
            return Ok(path);
        }
        tracing::warn!(path = %path.display(), "removing corrupt cached Kata tarball");
    }

    tracing::info!(
        version = KATA_VERSION,
        url = KATA_KERNEL.tar_url,
        dest = %path.display(),
        "downloading pinned Kata static bundle (cached for future provisions)"
    );

    let partial = path.with_extension("tar.xz.partial");
    let output = Command::new("curl")
        .args([
            "-fL",
            "--retry",
            "3",
            "-C",
            "-",
            "-o",
            partial.to_str().unwrap_or(""),
            KATA_KERNEL.tar_url,
        ])
        .output()
        .map_err(|e| format!("curl failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Kata tarball download failed: {stderr}"));
    }

    fs::rename(&partial, &path).map_err(|e| e.to_string())?;
    verify_tarball_integrity(&path)?;
    Ok(path)
}

fn verify_tarball_integrity(path: &Path) -> Result<(), String> {
    let output = Command::new("xz")
        .args(["-t", path.to_str().unwrap_or("")])
        .output()
        .map_err(|e| format!("xz integrity check failed to run: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let _ = fs::remove_file(path);
    Err(format!(
        "Kata tarball failed integrity check (corrupt or incomplete download): {stderr}"
    ))
}

fn extract_kernel_from_tarball(layout: &VmLayout, tarball: &Path) -> Result<(), String> {
    let kernel_path = layout.kernel_path();
    let work = layout.artifacts_dir();
    let member = KATA_KERNEL.tar_member;
    let extracted = work.join(member);

    if extracted.parent().is_some() {
        let _ = fs::remove_dir_all(work.join("opt"));
    }

    tracing::info!(tarball = %tarball.display(), member, "extracting Linux image from Kata bundle");

    let output = Command::new("tar")
        .args([
            "-xJf",
            tarball.to_str().unwrap_or(""),
            "-C",
            work.to_str().unwrap_or(""),
            member,
        ])
        .output()
        .map_err(|e| format!("tar failed to run: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "failed to extract {member} from {tarball:?}:\nstdout:{stdout}\nstderr:{stderr}"
        ));
    }

    if !extracted.exists() {
        return Err(format!(
            "expected kernel at {} after tar extract",
            extracted.display()
        ));
    }

    fs::rename(&extracted, &kernel_path).map_err(|e| e.to_string())?;
    let _ = fs::remove_dir_all(work.join("opt"));
    Ok(())
}

pub fn validate_linux_image(path: &Path) -> Result<(), String> {
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() < 1024 * 1024 {
        return Err(format!(
            "kernel file too small ({} bytes); expected multi-megabyte ARM64 Image",
            meta.len()
        ));
    }

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
    "console=hvc0 root=/dev/vda rw rootfstype=ext4 rootwait init=/sbin/gptbot-init"
}
