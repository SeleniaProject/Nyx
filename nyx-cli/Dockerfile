# syntax=docker/dockerfile:1.4

FROM --platform=$BUILDPLATFORM rust:1.81-slim AS builder
ARG TARGET=x86_64-unknown-linux-musl
ENV CARGO_TERM_COLOR=always
RUN rustup update && rustup target add $TARGET
WORKDIR /workspace

# Cache deps
COPY Cargo.toml Cargo.lock ./
COPY nyx-cli/Cargo.toml nyx-cli/Cargo.toml
RUN mkdir -p nyx-cli/src && echo "fn main(){}" > nyx-cli/src/main.rs
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/workspace/target \
    cargo build -p nyx-cli --release --target $TARGET || true

# Build real binary
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/workspace/target \
    cargo build -p nyx-cli --release --target $TARGET

FROM gcr.io/distroless/cc-debian12
ARG TARGET=x86_64-unknown-linux-musl
COPY --from=builder /workspace/target/${TARGET}/release/nyx-cli /usr/bin/nyx-cli
USER 65532:65532
ENTRYPOINT ["/usr/bin/nyx-cli"]