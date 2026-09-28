# Build context is the repo root (core/ + server/): `server/deploy.sh` streams
# exactly those two directories from a commit, so the image is that commit.
FROM rust:1-bookworm AS build
WORKDIR /src
COPY core core
COPY server server
RUN cargo build --release --locked --manifest-path server/Cargo.toml

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates libssl3 \
 && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/server/target/release/ribbit-server /usr/local/bin/ribbit-server
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/ribbit-server"]
