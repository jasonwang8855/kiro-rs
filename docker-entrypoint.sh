#!/bin/sh
set -e

CONFIG_DIR="/app/config"

if [ ! -f "$CONFIG_DIR/credentials.json" ]; then
  echo "credentials.json not found, creating default..."
  cat > "$CONFIG_DIR/credentials.json" <<'CRED'
{
  "refreshToken": "your-refresh-token",
  "expiresAt": "2099-01-01T00:00:00.000Z",
  "authMethod": "social"
}
CRED
fi

exec "$@"
