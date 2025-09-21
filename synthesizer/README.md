# Development

```bash
# With Hot Refresh
cargo watch -x run

# Or With Logging
RUST_LOG=info cargo watch -x run
```

# Release

```bash
# Build
cargo build --release

# Start Built Binary with Logging
RUST_LOG=info ./target/release/synthesizer
```

# Swagger UI

One can access Swagger UI at: http://localhost:8080/swagger-ui/

# Run Neo4J Docker Instances

```bash
docker compose up -d
```
