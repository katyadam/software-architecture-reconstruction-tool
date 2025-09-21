# --------- Build Stage ----------
FROM debian:bookworm-slim AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libssl-dev \
    libpq-dev \
    curl \
    rustc \
    cargo \
    ca-certificates \
 && rm -rf /var/lib/apt/lists/*

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /usr/src/app

# Copy and build your Rust app
COPY . .

# Install Diesel CLI
# This builds Diesel on the same base image, ensuring glibc compatibility.
RUN cargo install diesel_cli --no-default-features --features postgres

# Build your binary
RUN cargo build --release

# --------- Runtime Stage ----------
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    postgresql-client \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/local/bin

# Copy built artifacts
COPY --from=builder /usr/src/app/target/release/manager .
COPY --from=builder /root/.cargo/bin/diesel .
COPY --from=builder /usr/src/app/migrations ./migrations
COPY --from=builder /usr/src/app/.env ./.env

ENV RUST_LOG=info

# Run migrations and start the app
CMD ["sh", "-c", "diesel migration run && ./manager"]