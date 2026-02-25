#!/bin/sh
set -e

CONFIG_DIR="/app/config"

if [ ! -f "$CONFIG_DIR/config.json" ]; then
  echo "config.json not found, creating default..."
  cat > "$CONFIG_DIR/config.json" <<'CONF'
{
  "host": "0.0.0.0",
  "port": 8990,
  "apiKey": "sk-kiro-rs-your-api-key",
  "region": "us-east-1",
  "adminApiKey": "sk-admin-your-secret-key",
  "adminUsername": "admin",
  "adminPassword": "admin"
}
CONF
fi

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
