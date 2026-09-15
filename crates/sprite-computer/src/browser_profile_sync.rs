//! Sync Chromium user-data between the cloud-host volume and the sprite guest.

use std::path::Path;
use std::time::Duration;

use agent_core::ComputerError;

use crate::browser::{restart_browser_daemon, BROWSER_EXEC_CWD};
use crate::client::SpriteClient;
use crate::policy::NetworkPolicyConfig;

const GUEST_PROFILE_DIR: &str = "/var/elsewhere/browser/profile";
const GUEST_BUNDLE_PATH: &str = "/var/elsewhere/browser/.host-profile.tar.gz";

fn host_profile_has_data(host_profile_dir: &Path) -> bool {
    if !host_profile_dir.is_dir() {
        return false;
    }
    std::fs::read_dir(host_profile_dir)
        .ok()
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false)
}

fn tar_create_host_dir(host_profile_dir: &Path) -> Result<Vec<u8>, ComputerError> {
    let dir = host_profile_dir.to_string_lossy();
    let output = std::process::Command::new("tar")
        .args(["-czf", "-", "-C", dir.as_ref(), "."])
        .output()
        .map_err(|e| ComputerError::ExecutionFailed(format!("host tar create failed: {e}")))?;
    if !output.status.success() {
        return Err(ComputerError::ExecutionFailed(format!(
            "host tar create failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(output.stdout)
}

fn tar_extract_host_dir(host_profile_dir: &Path, bytes: &[u8]) -> Result<(), ComputerError> {
    std::fs::create_dir_all(host_profile_dir).map_err(|e| {
        ComputerError::ExecutionFailed(format!("host profile mkdir failed: {e}"))
    })?;
    let dir = host_profile_dir.to_string_lossy();
    let mut child = std::process::Command::new("tar")
        .args(["-xzf", "-", "-C", dir.as_ref()])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| ComputerError::ExecutionFailed(format!("host tar extract failed: {e}")))?;
    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().ok_or_else(|| {
            ComputerError::ExecutionFailed("host tar extract stdin unavailable".into())
        })?;
        stdin
            .write_all(bytes)
            .map_err(|e| ComputerError::ExecutionFailed(format!("host tar write failed: {e}")))?;
    }
    let status = child
        .wait()
        .map_err(|e| ComputerError::ExecutionFailed(format!("host tar wait failed: {e}")))?;
    if !status.success() {
        return Err(ComputerError::ExecutionFailed(
            "host tar extract exited with failure".into(),
        ));
    }
    Ok(())
}

/// Push host Chromium user-data into the guest before browser work.
pub async fn hydrate_from_host(
    client: &SpriteClient,
    baseline_policy: &NetworkPolicyConfig,
    exec_timeout: Duration,
    host_profile_dir: &Path,
) -> Result<(), ComputerError> {
    if !host_profile_has_data(host_profile_dir) {
        return Ok(());
    }
    let host_profile_dir = host_profile_dir.to_path_buf();
    let bundle = tokio::task::spawn_blocking(move || tar_create_host_dir(&host_profile_dir))
    .await
    .map_err(|e| ComputerError::ExecutionFailed(format!("host tar task failed: {e}")))??;

    restart_browser_daemon(client, baseline_policy).await?;

    client
        .fs_write(GUEST_BUNDLE_PATH, &bundle, true)
        .await
        .map_err(map_sprite_err)?;

    let script = format!(
        r#"set -e
mkdir -p "{GUEST_PROFILE_DIR}"
rm -rf "{GUEST_PROFILE_DIR}"/*
tar -xzf "{GUEST_BUNDLE_PATH}" -C "{GUEST_PROFILE_DIR}"
rm -f "{GUEST_BUNDLE_PATH}"
chmod -R 700 "{GUEST_PROFILE_DIR}" 2>/dev/null || true
"#
    );
    let (_, stderr, code) = client
        .exec_http(&script, BROWSER_EXEC_CWD, exec_timeout)
        .await
        .map_err(map_sprite_err)?;
    if code != 0 {
        return Err(ComputerError::GuestUnavailable(format!(
            "browser profile hydrate failed (exit {code}): {stderr}"
        )));
    }
    Ok(())
}

/// Pull guest Chromium user-data back to the host after browser work.
pub async fn persist_to_host(
    client: &SpriteClient,
    baseline_policy: &NetworkPolicyConfig,
    exec_timeout: Duration,
    host_profile_dir: &Path,
) -> Result<(), ComputerError> {
    restart_browser_daemon(client, baseline_policy).await?;

    let script = format!(
        r#"set -e
if [ ! -d "{GUEST_PROFILE_DIR}" ]; then
  exit 0
fi
tar -czf "{GUEST_BUNDLE_PATH}" -C "{GUEST_PROFILE_DIR}" .
"#
    );
    let (_, stderr, code) = client
        .exec_http(&script, BROWSER_EXEC_CWD, exec_timeout)
        .await
        .map_err(map_sprite_err)?;
    if code != 0 {
        return Err(ComputerError::GuestUnavailable(format!(
            "browser profile capture failed (exit {code}): {stderr}"
        )));
    }

    let bundle = client.fs_read(GUEST_BUNDLE_PATH).await.map_err(map_sprite_err)?;
    let _ = client
        .exec_http(
            &format!("rm -f \"{GUEST_BUNDLE_PATH}\""),
            BROWSER_EXEC_CWD,
            Duration::from_secs(15),
        )
        .await;

    if bundle.is_empty() {
        return Ok(());
    }

    let host_profile_dir = host_profile_dir.to_path_buf();
    tokio::task::spawn_blocking(move || {
        if host_profile_dir.exists() {
            std::fs::remove_dir_all(&host_profile_dir).map_err(|e| {
                ComputerError::ExecutionFailed(format!("host profile reset failed: {e}"))
            })?;
        }
        tar_extract_host_dir(&host_profile_dir, &bundle)
    })
    .await
    .map_err(|e| ComputerError::ExecutionFailed(format!("host tar task failed: {e}")))??;

    Ok(())
}

/// Remove guest profile bytes after a host-side reset.
pub async fn clear_guest_profile(
    client: &SpriteClient,
    baseline_policy: &NetworkPolicyConfig,
    exec_timeout: Duration,
) -> Result<(), ComputerError> {
    restart_browser_daemon(client, baseline_policy).await?;
    let script = format!(
        r#"set -e
rm -rf "{GUEST_PROFILE_DIR}"
mkdir -p "{GUEST_PROFILE_DIR}"
chmod 700 "{GUEST_PROFILE_DIR}"
rm -f "{GUEST_BUNDLE_PATH}"
"#
    );
    let (_, stderr, code) = client
        .exec_http(&script, BROWSER_EXEC_CWD, exec_timeout)
        .await
        .map_err(map_sprite_err)?;
    if code != 0 {
        return Err(ComputerError::GuestUnavailable(format!(
            "browser profile clear failed (exit {code}): {stderr}"
        )));
    }
    Ok(())
}

fn map_sprite_err(err: crate::types::SpriteError) -> ComputerError {
    match err {
        crate::types::SpriteError::NotFound => {
            ComputerError::GuestUnavailable("sprite filesystem unavailable".into())
        }
        crate::types::SpriteError::Timeout => {
            ComputerError::ExecutionFailed("browser profile sync timed out".into())
        }
        crate::types::SpriteError::Config(msg) => ComputerError::MalformedArguments(msg),
        crate::types::SpriteError::Network(msg) => ComputerError::GuestUnavailable(msg),
        crate::types::SpriteError::NotReady(msg) => ComputerError::GuestUnavailable(msg),
        crate::types::SpriteError::MalformedResponse(msg) => ComputerError::ExecutionFailed(msg),
        crate::types::SpriteError::Provider { status, message }
            if status == 403 || status == 400 =>
        {
            ComputerError::SandboxRejected(message)
        }
        crate::types::SpriteError::Provider { message, .. } => {
            ComputerError::GuestUnavailable(message)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn host_tar_roundtrip_preserves_marker_file() {
        let temp = std::env::temp_dir().join(format!(
            "elsewhere-browser-profile-test-{}",
            std::process::id()
        ));
        let profile = temp.join("profile");
        std::fs::create_dir_all(profile.join("Default")).unwrap();
        std::fs::File::create(profile.join("Default/.session-marker"))
            .unwrap()
            .write_all(b"signed-in")
            .unwrap();

        let bundle = tar_create_host_dir(&profile).unwrap();
        let restored = temp.join("restored");
        tar_extract_host_dir(&restored, &bundle).unwrap();
        let marker = std::fs::read_to_string(restored.join("Default/.session-marker")).unwrap();
        assert_eq!(marker, "signed-in");
        std::fs::remove_dir_all(temp).ok();
    }
}
