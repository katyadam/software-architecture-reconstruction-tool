# ----------- Builder Stage -----------
FROM rust:1.90-alpine AS builder

# Install needed tools for compiling
RUN apk add --no-cache musl-dev pkgconfig openssl-dev

WORKDIR /usr/src/app

# Pre-cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release && rm -rf src

# Copy the source
COPY . .

# Build for musl (static binary)
RUN cargo build --release

# ----------- Runtime Stage -----------
FROM alpine:3.20 AS runtime

# Install runtime dependencies (if needed, e.g. certs)
RUN apk add --no-cache ca-certificates

WORKDIR /usr/local/bin

# Copy the binary from builder
COPY --from=builder /usr/src/app/target/release/python-extractor .

# Set logging env variable
ENV RUST_LOG=info

# Run the binary
CMD ["./python-extractor"]
