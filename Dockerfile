# syntax=docker/dockerfile:1
FROM rust:1.99.1-bookworm AS builder

WORKDIR /build
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
ENV OPENSSL_NO_VENDOR=1

COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
COPY fixtures/ ./fixtures/
# These generated data files are compile-time inputs, not analysis tools.
COPY tools/crafting_metadata.json \
     tools/container_oracle_equipment.json \
     tools/entity_tracking_facts.json \
     tools/entity_move_facts.json \
     tools/storage_registries.json \
     tools/storage_blockentity_defaults.json \
     tools/storage_blockentity_factories.json \
     tools/player_physics_facts.json \
     tools/item_entity_physics_facts.json ./tools/

RUN cargo build --locked --release --bin mc121-rust \
    && strip target/release/mc121-rust

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 libgcc-s1 tini \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /app/fixtures /app/deploy /data
WORKDIR /app
LABEL org.opencontainers.image.title="Cinderstone" \
      org.opencontainers.image.source="https://github.com/Ferigone/Cinderstone"

COPY --from=builder /build/target/release/mc121-rust /app/mc121-rust
COPY --from=builder /build/fixtures/ /app/fixtures/
COPY --chmod=755 deploy/start.sh /app/deploy/start.sh
COPY LICENSE THIRD_PARTY_NOTICES.md /app/
COPY licenses/ /app/licenses/
COPY third_party/licenses/ /app/third_party/licenses/

ENV MC_BIND=0.0.0.0 \
    MC_PORT=25565 \
    MC_DATA_DIR=/data \
    MC_VIEW_DISTANCE=2 \
    MC_DIFFICULTY=easy
EXPOSE 25565/tcp
# Persistent /data is mounted by Railway service configuration.
STOPSIGNAL SIGTERM
ENTRYPOINT ["/usr/bin/tini", "-g", "--", "/app/deploy/start.sh"]
CMD []
