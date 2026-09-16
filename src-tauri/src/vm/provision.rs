use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;

use super::kernel::{ensure_linux_image, kernel_command_line};
use super::paths::VmLayout;
use super::protocol::VmConfigFile;

const GUEST_AGENT_PORT: u32 = 1024;

pub fn host_arch() -> &'static str {
    #[cfg(target_arch = "aarch64")]
    {
        "arm64"
    }
    #[cfg(target_arch = "x86_64")]
    {
        "x86_64"
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        "unknown"
    }
}

pub fn guest_arch() -> &'static str {
    host_arch()
}

pub fn alpine_arch_slug() -> &'static str {
    match guest_arch() {
        "arm64" => "aarch64",
        "x86_64" => "x86_64",
        _ => "aarch64",
    }
}

pub fn provision_vm(layout: &VmLayout) -> Result<(), String> {
    layout.ensure_directories().map_err(|e| e.to_string())?;

    ensure_linux_image(layout)?;
    ensure_disk_image(layout)?;
    write_vm_config(layout)?;

    Ok(())
}

fn ensure_disk_image(layout: &VmLayout) -> Result<(), String> {
    let disk_path = layout.disk_path();
    let force_rebuild = std::env::var("GPTBOT_FORCE_DISK_REBUILD").ok().as_deref() == Some("1");

    if disk_path.exists() && validate_ext4_disk(&disk_path)? && !force_rebuild {
        return Ok(());
    }

    if disk_path.exists() {
        tracing::warn!(
            path = %disk_path.display(),
            force = force_rebuild,
            "removing guest disk for rebuild or invalid ext4 root"
        );
        fs::remove_file(&disk_path).map_err(|e| e.to_string())?;
    }

    let repo_script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/build-guest-disk.sh");

    if repo_script.exists() {
        let output = Command::new("bash")
            .arg(&repo_script)
            .arg(&disk_path)
            .arg(alpine_arch_slug())
            .output()
            .map_err(|e| format!("failed to run build-guest-disk.sh: {e}"))?;

        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "build-guest-disk.sh failed:\nstdout:{stdout}\nstderr:{stderr}"
            ));
        }
        if validate_ext4_disk(&disk_path)? {
            return Ok(());
        }
    }

    Err(
        "Guest disk was not built. Build gptbot-guest-agent and run scripts/build-guest-disk.sh."
            .into(),
    )
}

fn validate_ext4_disk(path: &Path) -> Result<bool, String> {
    if !path.exists() {
        return Ok(false);
    }
    let mut header = [0u8; 2048];
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    use std::io::Read;
    let read = file.read(&mut header).map_err(|e| e.to_string())?;
    if read < 0x43A {
        return Ok(false);
    }
    Ok(header[0x438] == 0x53 && header[0x439] == 0xEF)
}

fn write_vm_config(layout: &VmLayout) -> Result<(), String> {
    let config = VmConfigFile {
        id: "default".into(),
        host_arch: host_arch().into(),
        guest_arch: guest_arch().into(),
        cpu_count: 2,
        memory_mib: 2048,
        kernel_path: layout.kernel_path().to_string_lossy().into(),
        initrd_path: None,
        disk_path: layout.disk_path().to_string_lossy().into(),
        kernel_command_line: kernel_command_line().into(),
        guest_agent_port: GUEST_AGENT_PORT,
        console_log_path: layout.console_log_path().to_string_lossy().into(),
    };

    let json = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    let path = layout.vm_config_path();
    let mut file = fs::File::create(path).map_err(|e| e.to_string())?;
    file.write_all(json.as_bytes()).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn locate_guest_agent_binary() -> Result<std::path::PathBuf, String> {
    if let Ok(path) = std::env::var("GPTBOT_GUEST_AGENT_PATH") {
        let p = std::path::PathBuf::from(path);
        if p.exists() {
            return Ok(p);
        }
    }

    let target = match guest_arch() {
        "arm64" => "aarch64-unknown-linux-musl",
        _ => "x86_64-unknown-linux-musl",
    };

    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let release_path = manifest
        .join("../guest-agent/target")
        .join(target)
        .join("release/gptbot-guest-agent");
    if release_path.exists() {
        return Ok(release_path);
    }

    Err(
        "gptbot-guest-agent binary not found. Cross-compile with: cargo build -p gptbot-guest-agent --release --target <musl>"
            .into(),
    )
}
