#!/bin/sh
set -eu

: "${MC_BIND:=0.0.0.0}"
: "${MC_PORT:=25565}"
: "${MC_DATA_DIR:=/data}"
: "${MC_VIEW_DISTANCE:=2}"
: "${MC_DIFFICULTY:=easy}"

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
# Public deployment always verifies Java account sessions; no offline fallback.
exec /app/mc121-rust \
    --online-mode \
    --bind "$MC_BIND" \
    --port "$MC_PORT" \
    --fixtures /app/fixtures/configuration.json \
    --world "$MC_DATA_DIR/world.json" \
    --view-distance "$MC_VIEW_DISTANCE" \
    --difficulty "$MC_DIFFICULTY" \
    "$@"
