use agent_core::ComputerError;
use std::time::Duration;

use crate::client::SpriteClient;
use crate::policy::{
    browser_workload_network_policy, network_policy_matches, NetworkPolicyConfig,
};

pub const BROWSER_ROOT: &str = "/var/elsewhere/browser";
pub const BROWSER_CLIENT: &str = "/var/elsewhere/browser/client.mjs";
pub const BROWSER_DAEMON: &str = "/var/elsewhere/browser/daemon.mjs";
pub const BROWSER_COMMON: &str = "/var/elsewhere/browser/browser-common.mjs";
pub const BROWSER_REQUEST: &str = "/var/elsewhere/browser/.last-request.json";
pub const BROWSER_BOOTSTRAP_MARKER: &str = "/var/elsewhere/browser/.bootstrapped";
pub const BROWSER_DAEMON_PID: &str = "/var/elsewhere/browser/daemon.pid";

const CLIENT_SOURCE: &str = include_str!("../guest/browser-client.mjs");
const DAEMON_SOURCE: &str = include_str!("../guest/browser-daemon.mjs");
const COMMON_SOURCE: &str = include_str!("../guest/browser-common.mjs");
const BOOTSTRAP_VERSION: &str = include_str!("../guest/browser-bootstrap-version.txt");

const PACKAGE_JSON: &str = r#"{"name":"elsewhere-browser","private":true,"type":"module"}"#;

/// Install headless Chromium tooling and browser daemon assets (idempotent).
pub async fn ensure_browser_guest(
    client: &SpriteClient,
    baseline_policy: &NetworkPolicyConfig,
    _exec_timeout: Duration,
) -> Result<(), ComputerError> {
    client
        .fs_write(BROWSER_CLIENT, CLIENT_SOURCE.as_bytes(), true)
        .await
        .map_err(map_err)?;
    client
        .fs_write(BROWSER_DAEMON, DAEMON_SOURCE.as_bytes(), true)
        .await
        .map_err(map_err)?;
    client
        .fs_write(BROWSER_COMMON, COMMON_SOURCE.as_bytes(), true)
        .await
        .map_err(map_err)?;
    client
        .fs_write(
            &format!("{BROWSER_ROOT}/package.json"),
            PACKAGE_JSON.as_bytes(),
            true,
        )
        .await
        .map_err(map_err)?;
    client
        .fs_write(
            &format!("{BROWSER_ROOT}/bootstrap-version"),
            BOOTSTRAP_VERSION.trim().as_bytes(),
            true,
        )
        .await
        .map_err(map_err)?;

    let version_ok = client
        .fs_read(&format!("{BROWSER_ROOT}/bootstrap-version"))
        .await
        .map_err(map_err)?;
    let bootstrapped = client.fs_read(BROWSER_BOOTSTRAP_MARKER).await.is_ok();
    if bootstrapped && version_ok == BOOTSTRAP_VERSION.trim().as_bytes() {
        let (_, _, code) = client
            .exec_http(
                "test -f /var/elsewhere/browser/node_modules/playwright-core/package.json \
                 && test -f /var/elsewhere/browser/.bootstrapped \
                 && ls /var/elsewhere/browser/browsers/chromium-* >/dev/null 2>&1 \
                 && test -f /var/elsewhere/browser/.deps-ready",
                "/workspace",
                Duration::from_secs(15),
            )
            .await
            .map_err(map_err)?;
        if code == 0 {
            return Ok(());
        }
    }

    let bootstrap = r#"
set -e
BROWSER_DIR="/var/elsewhere/browser"
export PATH="/usr/local/bin:/usr/bin:/bin"
export PLAYWRIGHT_BROWSERS_PATH="$BROWSER_DIR/browsers"
mkdir -p "$BROWSER_DIR"
chmod 700 "$BROWSER_DIR"
if ! command -v node >/dev/null 2>&1; then
  NODE_DIR="$BROWSER_DIR/node-runtime"
  if [ ! -x "$NODE_DIR/bin/node" ]; then
    mkdir -p "$NODE_DIR"
    ARCH="$(uname -m)"
    case "$ARCH" in
      x86_64) NODE_ARCH="linux-x64" ;;
      aarch64|arm64) NODE_ARCH="linux-arm64" ;;
      *) echo "unsupported node arch: $ARCH" >&2; exit 1 ;;
    esac
    curl -fsSL "https://nodejs.org/dist/v20.18.0/node-v20.18.0-${NODE_ARCH}.tar.gz" \
      | tar -xz -C "$NODE_DIR" --strip-components=1
  fi
  export PATH="$NODE_DIR/bin:$PATH"
fi
if ! command -v node >/dev/null 2>&1; then
  echo "nodejs unavailable after bootstrap" >&2
  exit 1
fi
if command -v apk >/dev/null 2>&1; then
  apk add --no-cache curl chromium 2>/dev/null || true
fi
if [ ! -f "$BROWSER_DIR/node_modules/playwright-core/package.json" ]; then
  cd "$BROWSER_DIR" && npm install playwright-core@1.49.1 --no-fund --no-audit --loglevel=error
fi
if ! ls "$PLAYWRIGHT_BROWSERS_PATH"/chromium-* >/dev/null 2>&1; then
  cd "$BROWSER_DIR" && node node_modules/playwright-core/cli.js install chromium
fi
cd "$BROWSER_DIR" && node node_modules/playwright-core/cli.js install-deps chromium 2>/dev/null \
  || node node_modules/playwright-core/cli.js install-deps 2>/dev/null \
  || true
touch "$BROWSER_DIR/.deps-ready"
touch "$BROWSER_DIR/.bootstrapped"
"#;

    with_temporary_egress(client, baseline_policy, async {
        let (stdout, stderr, code) = client
            .exec_http(bootstrap, "/workspace", Duration::from_secs(300))
            .await
            .map_err(map_err)?;
        if code != 0 {
            return Err(ComputerError::GuestUnavailable(format!(
                "browser bootstrap failed (exit {code}): {stderr} {stdout}"
            )));
        }
        Ok(())
    })
    .await?;

    Ok(())
}

pub async fn ensure_browser_daemon(
    client: &SpriteClient,
    baseline_policy: &NetworkPolicyConfig,
    exec_timeout: Duration,
) -> Result<(), ComputerError> {
    let health_cmd = format!(
        "export PLAYWRIGHT_BROWSERS_PATH={BROWSER_ROOT}/browsers; \
         export PATH={BROWSER_ROOT}/node-runtime/bin:$PATH; \
         node {BROWSER_CLIENT} health"
    );
    if daemon_health(client, &health_cmd, exec_timeout).await? {
        return Ok(());
    }

    let start = format!(
        r#"set -e
BROWSER_DIR="{BROWSER_ROOT}"
export PLAYWRIGHT_BROWSERS_PATH="$BROWSER_DIR/browsers"
export PATH="$BROWSER_DIR/node-runtime/bin:$PATH"
mkdir -p "$BROWSER_DIR"
chmod 700 "$BROWSER_DIR"
if [ -f "{BROWSER_DAEMON_PID}" ] && kill -0 "$(cat {BROWSER_DAEMON_PID})" 2>/dev/null; then
  exit 0
fi
nohup node {BROWSER_DAEMON} >> "$BROWSER_DIR/daemon.log" 2>&1 &
echo $! > {BROWSER_DAEMON_PID}
sleep 1
"#
    );

    with_temporary_egress(client, baseline_policy, async {
        let (_, stderr, code) = client
            .exec_http(&start, "/workspace", Duration::from_secs(30))
            .await
            .map_err(map_err)?;
        if code != 0 {
            return Err(ComputerError::GuestUnavailable(format!(
                "browser daemon start failed (exit {code}): {stderr}"
            )));
        }
        Ok(())
    })
    .await?;

    for attempt in 0..8 {
        if daemon_health(client, &health_cmd, Duration::from_secs(10)).await? {
            return Ok(());
        }
        if attempt < 7 {
            let wait_ms = 250 * (attempt + 1);
            let (_, _, _) = client
                .exec_http(
                    &format!("sleep {}", wait_ms as f64 / 1000.0),
                    "/workspace",
                    Duration::from_secs(5),
                )
                .await
                .map_err(map_err)?;
        }
    }

    Err(ComputerError::GuestUnavailable(
        "browser daemon did not become healthy".into(),
    ))
}

async fn daemon_health(
    client: &SpriteClient,
    health_cmd: &str,
    timeout: Duration,
) -> Result<bool, ComputerError> {
    let (stdout, _, code) = client
        .exec_http(health_cmd, "/workspace", timeout)
        .await
        .map_err(map_err)?;
    Ok(code == 0 && stdout.contains("\"ok\":true"))
}

pub async fn invoke_browser_daemon(
    client: &SpriteClient,
    baseline_policy: &NetworkPolicyConfig,
    payload: &str,
    exec_timeout: Duration,
) -> Result<String, ComputerError> {
    client
        .fs_write(BROWSER_REQUEST, payload.as_bytes(), true)
        .await
        .map_err(map_err)?;

    let command = format!(
        "export PLAYWRIGHT_BROWSERS_PATH={BROWSER_ROOT}/browsers; \
         export PATH={BROWSER_ROOT}/node-runtime/bin:$PATH; \
         node {BROWSER_CLIENT} --request {BROWSER_REQUEST}"
    );

    let mut last_err = None;
    for attempt in 0..2 {
        ensure_browser_daemon(client, baseline_policy, exec_timeout).await?;
        let stdout = with_temporary_egress(client, baseline_policy, async {
            let (stdout, stderr, exit_code) = client
                .exec_http(&command, "/workspace", exec_timeout)
                .await
                .map_err(map_err)?;
            if exit_code != 0 {
                return Err(map_browser_exec_error(&stdout, &stderr, exit_code));
            }
            Ok(stdout)
        })
        .await;

        match stdout {
            Ok(out) => return Ok(out),
            Err(err) => {
                let retryable = err.to_string().contains("ERR_CONNECTION_REFUSED")
                    || err.to_string().contains("ERR_NAME_NOT_RESOLVED")
                    || err.to_string().contains("browser daemon did not become healthy");
                if attempt == 0 && retryable {
                    let _ = restart_browser_daemon(client, baseline_policy).await;
                    last_err = Some(err);
                    continue;
                }
                return Err(err);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| {
        ComputerError::GuestUnavailable("browser invoke failed after retry".into())
    }))
}

async fn restart_browser_daemon(
    client: &SpriteClient,
    baseline_policy: &NetworkPolicyConfig,
) -> Result<(), ComputerError> {
    let script = format!(
        r#"set -e
if [ -f "{BROWSER_DAEMON_PID}" ]; then
  kill "$(cat {BROWSER_DAEMON_PID})" 2>/dev/null || true
  rm -f "{BROWSER_DAEMON_PID}"
fi
rm -f /var/elsewhere/browser/daemon.sock
"#
    );
    with_temporary_egress(client, baseline_policy, async {
        let (_, _, code) = client
            .exec_http(&script, "/workspace", Duration::from_secs(15))
            .await
            .map_err(map_err)?;
        if code != 0 {
            return Err(ComputerError::GuestUnavailable(
                "failed to restart browser daemon".into(),
            ));
        }
        Ok(())
    })
    .await?;
    Ok(())
}

async fn restore_and_verify_baseline_network_policy(
    client: &SpriteClient,
    baseline: &NetworkPolicyConfig,
) -> Result<(), ComputerError> {
    client
        .set_network_policy(baseline)
        .await
        .map_err(map_err)?;
    // Fly Sprites network policy updates are asynchronous; give deny a moment to apply.
    tokio::time::sleep(Duration::from_millis(150)).await;
    let actual = client.get_network_policy().await.map_err(map_err)?;
    if !network_policy_matches(&actual, baseline) {
        return Err(ComputerError::GuestUnavailable(
            "sprite network policy is not default-deny after browser egress; computer is not safe for exec"
                .into(),
        ));
    }
    Ok(())
}

pub async fn with_temporary_egress<T, F>(
    client: &SpriteClient,
    baseline: &NetworkPolicyConfig,
    f: F,
) -> Result<T, ComputerError>
where
    F: std::future::Future<Output = Result<T, ComputerError>>,
{
    client
        .set_network_policy(&browser_workload_network_policy())
        .await
        .map_err(map_err)?;
    // Fly Sprites network policy updates are asynchronous; give egress a moment to apply.
    tokio::time::sleep(Duration::from_millis(400)).await;
    let result = f.await;
    let restore = restore_and_verify_baseline_network_policy(client, baseline).await;
    match (result, restore) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(inner), Ok(())) => Err(inner),
        (_, Err(restore_err)) => Err(restore_err),
    }
}

pub fn map_browser_exec_error(stdout: &str, stderr: &str, exit_code: i32) -> ComputerError {
    if exit_code == 0 {
        return ComputerError::ExecutionFailed("browser client returned success without output".into());
    }
    let detail = if stderr.trim().is_empty() {
        stdout.trim()
    } else {
        stderr.trim()
    };
    ComputerError::ExecutionFailed(format!("browser: {detail}"))
}

fn map_err(err: crate::types::SpriteError) -> ComputerError {
    match err {
        crate::types::SpriteError::NotFound => ComputerError::NotProvisioned,
        crate::types::SpriteError::Timeout => {
            ComputerError::ExecutionFailed("browser command timed out".into())
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

    #[test]
    fn guest_sources_are_non_empty() {
        assert!(CLIENT_SOURCE.contains("callDaemon"));
        assert!(DAEMON_SOURCE.contains("buildSnapshot"));
        assert!(COMMON_SOURCE.contains("assertPublicHttpUrl"));
        assert!(COMMON_SOURCE.contains("downloadHttpWithRedirects"));
        assert!(DAEMON_SOURCE.contains("runExclusive"));
    }
}
