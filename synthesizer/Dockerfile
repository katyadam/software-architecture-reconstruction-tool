# ----------- Builder Stage -----------
FROM rust:1.90-alpine AS builder

# Install needed tools for compiling
RUN apk add --no-cache musl-dev pkgconfig openssl-dev curl

WORKDIR /usr/src/app

# Copy Workspace Cargo.toml
# COPY Cargo.toml Cargo.lock ./

# Copy all needed crates
COPY synthesizer ./synthesizer
COPY ./models ./models

WORKDIR /usr/src/app/synthesizer
# Build for musl (static binary)
RUN cargo build --release

# ----------- Runtime Stage -----------
FROM alpine:3.20 AS runtime

# Install runtime dependencies (if needed, e.g. certs)
RUN apk add --no-cache ca-certificates

WORKDIR /usr/local/bin

# Copy the binary from builder
COPY --from=builder /usr/src/app/synthesizer/target/release/synthesizer .

# Set logging env variable
ENV RUST_LOG=info

# Run the binary
CMD ["./synthesizer"]
