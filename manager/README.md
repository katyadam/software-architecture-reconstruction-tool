# Manager Service

This project is a Rust-based backend service that uses PostgreSQL as its database and Diesel for database migrations.  
It includes development, release, and database management instructions.

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
http://localhost:8081/swagger-ui/
```

---

## Running PostgreSQL with Docker

Start a PostgreSQL instance using Docker:

```bash
docker run -d \
  --name manager-db \
  -e POSTGRES_PASSWORD=password \
  -e POSTGRES_USER=postgres \
  -e POSTGRES_DB=manager-db \
  -p 5432:5432 \
  postgres
```

---

## Database Migrations

We use **Diesel** for managing database migrations.

### 1️⃣ Install Diesel CLI

```bash
cargo install diesel_cli --no-default-features --features postgres
```

> **Note:**  
> If you see the error:
>
> ```
> rust-lld: error: unable to find library -lpq
> ```
>
> you need to install PostgreSQL development libraries:

```bash
dnf install postgresql-devel
```

---

### 2️⃣ Setup Diesel

Initial setup creates a local `diesel.toml` file pointing to your migrations folder.

```bash
diesel setup
```

> **⚠ DO NOT** commit `diesel.toml` to version control — it contains local paths.

---

### 3️⃣ Run Migrations

Ensure you have a `.env` file created from `.env.example` before running:

```bash
diesel migration run
```

---

## 📝 Notes

- Ensure Docker is running before starting the database.
- Make sure `.env` is properly configured before running migrations or starting the application.
- For debugging, increase log levels by setting `RUST_LOG=debug`.

---
