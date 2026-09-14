#!/usr/bin/env bash
set -euo pipefail

# Usage: migrate-hosted.sh [info|run]
# Default: info

ROOT="$(cd "$(dirname "$0")/../../../.." && pwd)"
ENV_FILE="${ROOT}/.env.hosted"
CA_FILE="${ROOT}/infra/certs/supabase-prod-ca-2021.crt"
CMD="${1:-info}"

if [[ ! -f "$ENV_FILE" ]]; then
  echo "Missing $ENV_FILE (local staging; not in git)" >&2
  exit 1
fi

if [[ ! -f "$CA_FILE" ]]; then
  echo "Missing CA cert: $CA_FILE" >&2
  exit 1
fi

if ! command -v sqlx >/dev/null 2>&1; then
  echo "Install sqlx CLI: cargo install sqlx-cli --no-default-features --features rustls,postgres" >&2
  exit 1
fi

export DATABASE_URL="$(python3 - <<PY
from pathlib import Path
import re
import urllib.parse

root = Path("${ROOT}")
line = next(
    l for l in root.joinpath(".env.hosted").read_text().splitlines()
    if l.startswith("DATABASE_URL=")
)
url = line.split("=", 1)[1].strip()
ca = str(root / "infra/certs/supabase-prod-ca-2021.crt")
url = re.sub(
    r"sslrootcert=[^&]+",
    "sslrootcert=" + urllib.parse.quote(ca, safe=""),
    url,
)
print(url)
PY
)"

cd "${ROOT}/services/cloud-host"

case "$CMD" in
  info) sqlx migrate info ;;
  run) sqlx migrate run ;;
  *)
    echo "Usage: $0 [info|run]" >&2
    exit 1
    ;;
esac
