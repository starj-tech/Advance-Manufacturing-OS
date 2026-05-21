#!/usr/bin/env bash
# Generate self-signed certificates for the local dev MQTT
# broker. Run once before `docker compose -f docker-compose.dev.yml up`.
#
# DEV-ONLY. Never use these certs against a production
# Mosquitto. The CA private key is generated alongside; in
# production the CA key lives in a hardware token.

set -euo pipefail

CERT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$CERT_DIR"

# CA
openssl genrsa -out ca.key 4096
openssl req -x509 -new -key ca.key -sha256 -days 365 \
  -subj "/CN=aether-dev-ca/O=aether-os-dev" \
  -out ca.crt

# Server
openssl genrsa -out server.key 4096
openssl req -new -key server.key \
  -subj "/CN=localhost/O=aether-os-dev" \
  -out server.csr
openssl x509 -req -in server.csr \
  -CA ca.crt -CAkey ca.key -CAcreateserial \
  -out server.crt -days 365 -sha256

# Tighten perms — Mosquitto refuses world-readable key files.
chmod 600 ca.key server.key
chmod 644 ca.crt server.crt

echo "Dev certs generated in $CERT_DIR"
echo "  ca.crt + server.crt + server.key — mounted by docker-compose.dev.yml"
