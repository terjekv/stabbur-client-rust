#!/usr/bin/env bash
set -euo pipefail

image="${STABBUR_INTEGRATION_SERVER_IMAGE:-}"
if [[ "$image" != *@sha256:* ]]; then
  echo "STABBUR_INTEGRATION_SERVER_IMAGE must be pinned by digest" >&2
  exit 1
fi

container="stabbur-client-e2e-${RANDOM}"
request_file="$(mktemp)"
response_file="$(mktemp)"
trap 'docker rm --force "$container" >/dev/null 2>&1 || true; rm -f "$request_file" "$response_file"' EXIT

docker run --detach --name "$container" --publish 127.0.0.1::8080 "$image" >/dev/null
port="$(docker port "$container" 8080/tcp | sed 's/.*://')"
server_url="http://127.0.0.1:${port}"

for _ in $(seq 1 30); do
  if curl --fail --silent "${server_url}/healthz" >/dev/null; then
    break
  fi
  sleep 1
done
curl --fail --silent "${server_url}/healthz" >/dev/null

bootstrap_secret="$(docker exec "$container" sh -c 'cat /var/lib/stabbur/bootstrap.secret')"
password="integration-only-password-42"
chmod 0600 "$request_file" "$response_file"
jq --null-input \
  --arg secret "$bootstrap_secret" \
  --arg username live-admin \
  --arg password "$password" \
  '{secret:$secret, username:$username, password:$password}' >"$request_file"
curl --fail --silent \
  --header 'content-type: application/json' \
  --data-binary "@$request_file" \
  "${server_url}/api/v1/auth/bootstrap" >/dev/null
jq --null-input \
  --arg username live-admin \
  --arg password "$password" \
  '{username:$username, password:$password}' >"$request_file"
curl --fail --silent \
  --header 'content-type: application/json' \
  --data-binary "@$request_file" \
  "${server_url}/api/v1/auth/login" >"$response_file"

export STABBUR_E2E_SERVER_URL="$server_url"
export STABBUR_E2E_TOKEN
STABBUR_E2E_TOKEN="$(jq --raw-output .token "$response_file")"
cargo run -p e2e_client --locked
cargo test --workspace --all-features --locked
