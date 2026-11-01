#!/bin/sh
set -eu

: "${MC_BIND:=0.0.0.0}"
: "${MC_PORT:=25565}"
: "${MC_DATA_DIR:=/data}"
: "${MC_VIEW_DISTANCE:=2}"
: "${MC_DIFFICULTY:=easy}"
: "${MC_ONLINE_MODE:=true}"
: "${MC_RESOURCE_PACK_REQUIRED:=false}"
: "${MC_MAX_PLAYERS:=20}"
: "${MC_OP_PERMISSION_LEVEL:=4}"
: "${MC_ENFORCE_WHITELIST:=false}"

case "$MC_ENFORCE_WHITELIST" in
    true) set -- --enforce-whitelist "$@" ;;
    false) ;;
    *) echo "MC_ENFORCE_WHITELIST must be true or false" >&2; exit 1 ;;
esac

case "$MC_RESOURCE_PACK_REQUIRED" in
    true) set -- --require-resource-pack "$@" ;;
    false) ;;
    *) echo "MC_RESOURCE_PACK_REQUIRED must be true or false" >&2; exit 1 ;;
esac
if [ -n "${MC_RESOURCE_PACK_URL:-}" ]; then
    set -- --resource-pack-url "$MC_RESOURCE_PACK_URL" "$@"
fi
if [ -n "${MC_RESOURCE_PACK_SHA1:-}" ]; then
    set -- --resource-pack-sha1 "$MC_RESOURCE_PACK_SHA1" "$@"
fi
if [ -n "${MC_RESOURCE_PACK_ID:-}" ]; then
    set -- --resource-pack-id "$MC_RESOURCE_PACK_ID" "$@"
fi
if [ -n "${MC_RESOURCE_PACK_PROMPT:-}" ]; then
    set -- --resource-pack-prompt "$MC_RESOURCE_PACK_PROMPT" "$@"
fi

case "$MC_ONLINE_MODE" in
    true) set -- --online-mode "$@" ;;
    false) set -- --allow-offline --allow-public-offline "$@" ;;
    *) echo "MC_ONLINE_MODE must be true or false" >&2; exit 1 ;;
esac

case "$MC_PORT" in
    ''|*[!0-9]*) echo "MC_PORT must be an integer from 1 to 65535" >&2; exit 1 ;;
esac
if [ "${#MC_PORT}" -gt 5 ] || [ "$MC_PORT" -lt 1 ] || [ "$MC_PORT" -gt 65535 ]; then
    echo "MC_PORT must be an integer from 1 to 65535" >&2
    exit 1
fi
case "$MC_DATA_DIR" in
    /*) ;;
    *) echo "MC_DATA_DIR must be an absolute directory path" >&2; exit 1 ;;
esac
mkdir -p -- "$MC_DATA_DIR"

# exec leaves no shell between tini and the server's graceful signal handler.
# Offline access requires the explicit MC_ONLINE_MODE=false setting.
exec /app/mc121-rust \
    --bind "$MC_BIND" \
    --port "$MC_PORT" \
    --fixtures /app/fixtures/configuration.json \
    --world "$MC_DATA_DIR/world.json" \
    --server-directory "${MC_SERVER_DIRECTORY:-$MC_DATA_DIR}" \
    --max-players "$MC_MAX_PLAYERS" \
    --op-permission-level "$MC_OP_PERMISSION_LEVEL" \
    --view-distance "$MC_VIEW_DISTANCE" \
    --difficulty "$MC_DIFFICULTY" \
    "$@"
