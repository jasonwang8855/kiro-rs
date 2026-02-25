#!/bin/sh
set -e

CONFIG_DIR="/app/config"

if [ ! -f "$CONFIG_DIR/config.json" ]; then
  echo "config.json not found, creating from example..."
  cp /app/defaults/config.example.json "$CONFIG_DIR/config.json"
fi

if [ ! -f "$CONFIG_DIR/credentials.json" ]; then
  echo "credentials.json not found, creating from example..."
  cp /app/defaults/credentials.example.json "$CONFIG_DIR/credentials.json"
fi

exec "$@"
