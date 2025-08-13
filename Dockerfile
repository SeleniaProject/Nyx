# syntax=docker/dockerfile:1.4

# -------- build stage --------
FROM --platform=$BUILDPLATFORM rust:1.77-slim AS builder
ARG TARGETPLATFORM
ARG BUILDPLATFORM
ARG TARGET=x86_64-unknown-linux-musl
ENV CARGO_TERM_COLOR=always
RUN rustup target add $TARGET
WORKDIR /workspace

# Copy manifests separately for better layer caching
COPY Cargo.toml Cargo.lock ./
COPY build-protoc/Cargo.toml build-protoc/Cargo.toml
COPY nyx-cli/Cargo.toml nyx-cli/Cargo.toml
COPY nyx-conformance/Cargo.toml nyx-conformance/Cargo.toml
COPY nyx-control/Cargo.toml nyx-control/Cargo.toml
COPY nyx-core/Cargo.toml nyx-core/Cargo.toml
COPY nyx-crypto/Cargo.toml nyx-crypto/Cargo.toml
COPY nyx-daemon/Cargo.toml nyx-daemon/Cargo.toml
COPY nyx-fec/Cargo.toml nyx-fec/Cargo.toml
COPY nyx-mix/Cargo.toml nyx-mix/Cargo.toml
COPY nyx-sdk/Cargo.toml nyx-sdk/Cargo.toml
COPY nyx-stream/Cargo.toml nyx-stream/Cargo.toml
COPY nyx-telemetry/Cargo.toml nyx-telemetry/Cargo.toml
COPY nyx-transport/Cargo.toml nyx-transport/Cargo.toml

# Create minimal dummy src to warm cargo cache
RUN mkdir -p src && echo "fn main(){}" > src/main.rs

# Build deps with cache mounts
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/workspace/target \
    cargo build --release --target $TARGET || true

# Now copy full source and build workspace (excluding wasm)
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/workspace/target \
    cargo build --release --workspace --exclude nyx-sdk-wasm --target $TARGET

# -------- runtime stage --------
FROM gcr.io/distroless/cc-debian12@sha256:ce5c00e38acfc34b4e2cbbded4086985a45fc5517fcdba833ca6123e3aa8b6e1
ARG TARGET=x86_64-unknown-linux-musl

# Copy daemon binary; crate name produces binary `nyx-daemon`
COPY --from=builder /workspace/target/${TARGET}/release/nyx-daemon /usr/bin/nyx-daemon

# Run as non-root by default; Kubernetes may override UID/GID
USER 65532:65532

ENTRYPOINT ["/usr/bin/nyx-daemon"]