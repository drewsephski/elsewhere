use agent_core::ComputerError;
use std::time::Duration;

use crate::client::SpriteClient;

pub const BROWSER_DIR: &str = "/workspace/.elsewhere/browser";
pub const BROWSER_CLI: &str = "/workspace/.elsewhere/browser/cli.mjs";
pub const BROWSER_REQUEST: &str = "/workspace/.elsewhere/browser/.last-request.json";
pub const BROWSER_BOOTSTRAP_MARKER: &str = "/workspace/.elsewhere/browser/.bootstrapped";

const CLI_SOURCE: &str = include_str!("../guest/browser-cli.mjs");

const PACKAGE_JSON: &str = r#"{"name":"elsewhere-browser","private":true,"type":"module"}"#;

/// Install headless Chromium tooling inside the Sprite guest (idempotent).
pub async fn ensure_browser_guest(
    client: &SpriteClient,
    _exec_timeout: Duration,
) -> Result<(), ComputerError> {
    client
        .fs_write(BROWSER_CLI, CLI_SOURCE.as_bytes(), true)
        .await
        .map_err(map_err)?;
    client
        .fs_write(
            &format!("{BROWSER_DIR}/package.json"),
            PACKAGE_JSON.as_bytes(),
            true,
        )
        .await
        .map_err(map_err)?;

    if client
        .fs_read(BROWSER_BOOTSTRAP_MARKER)
        .await
        .is_ok()
    {
        let (_, _, code) = client
            .exec_http(
                "test -f /workspace/.elsewhere/browser/node_modules/playwright-core/package.json \
                 && test -f /workspace/.elsewhere/browser/.bootstrapped \
                 && ls /workspace/.elsewhere/browser/browsers/chromium-* >/dev/null 2>&1 \
                 && test -f /workspace/.elsewhere/browser/.deps-ready",
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
BROWSER_DIR="/workspace/.elsewhere/browser"
export PATH="/usr/local/bin:/usr/bin:/bin"
export PLAYWRIGHT_BROWSERS_PATH="$BROWSER_DIR/browsers"
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
touch /workspace/.elsewhere/browser/.deps-ready
touch /workspace/.elsewhere/browser/.bootstrapped
"#;

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
}

pub fn map_browser_exec_error(stdout: &str, stderr: &str, exit_code: i32) -> ComputerError {
    if exit_code == 0 {
        return ComputerError::ExecutionFailed("browser CLI returned success without output".into());
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
    fn guest_cli_source_is_non_empty() {
        assert!(CLI_SOURCE.contains("playwright-core"));
        assert!(CLI_SOURCE.contains("\"snapshot\""));
    }
}
