ARG RUST_IMAGE=rust:bookworm
ARG RUNTIME_IMAGE=debian:bookworm-slim

FROM ${RUST_IMAGE} AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY config/ config/
COPY assets/ assets/

RUN cargo build -p rust-desk-light-server --release --locked && \
    cp target/release/rdl-server-cli /rdl-server-cli

FROM ${RUNTIME_IMAGE} AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /rdl-server-cli /usr/local/bin/rdl-server-cli

EXPOSE 5169/tcp
EXPOSE 5169/udp

ENV RDL_IP=0.0.0.0
ENV RDL_PORT=5169
ENV RDL_CONFIG_DIR=/etc/rust-desk-light
ENV RDL_DATA_DIR=/var/lib/rust-desk-light

VOLUME ["/etc/rust-desk-light", "/var/lib/rust-desk-light"]

COPY scripts/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD ["/usr/local/bin/rdl-server-cli"]
