# Extractor

---

## Development

Run the service with **hot refresh**:

```bash
cargo watch -x run
```

Run the service with **logging enabled**:

```bash
RUST_LOG=info cargo watch -x run
```

---

## Release Build

Build the project in **release mode**:

```bash
cargo build --release
```

Run the **built binary** with logging:

```bash
RUST_LOG=info ./target/release/manager
```

---

## API Documentation

Swagger UI is available at:

```
http://localhost:8082/swagger-ui/
```
