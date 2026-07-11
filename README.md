# SAR Tool

A high-performance **Software Architecture Reconstruction (SAR)** tool built in Rust.

SAR Tool provides a unified workflow for analyzing distributed architecture systems. It crawls project directories, dispatches files for extraction via `tree-sitter`, and synthesizes the data into structured JSON models (views).

---

## Disclaimer
First, this repository is currently anonymized. Second, `main` branch currently includes multi-pass SAR. Single-pass SAR is on the branch `single-pass`. Another version is the combination of multi-pass with LLM in order to achieve the most precise reconstruction. The combination is located on the branch `sage-rework`.

## Overview

SAR Tool reconstructs architectural views and persists them within a **Neo4j** graph database, mapping them to Git-based repository metadata to ensure strict version consistency.

### Key Capabilities:

- **Automated Extraction:** Uses `tree-sitter` for fast, precise code parsing.
- **Multi-View Synthesis:** Generates JSON models for different architectural perspectives.
- **Graph Storage:** Maps reconstructed views to repository metadata in Neo4j.
- **Distributed Ready:** Designed to handle complex, distributed architectural styles.

---

## Installation & Setup

### Prerequisites

- **Rust & Cargo** (Edition 2024) – [Install via rustup](https://rustup.rs/)
- **Docker & Docker Compose** – Required for database storage and full-stack deployment.

---

## Usage: Command Line Interface (CLI)

The CLI is the preferred lightweight method when you only need to generate SAR views as JSON files without a full database stack.

### 1. Clone the Repository

```bash
git clone https://github.com/katyadam/software-architecture-reconstruction-tool.git
cd software-architecture-reconstruction-tool
```

### 2. Basic Execution

```bash
cargo run -p cli --release -- \
  -p ./path/to/target-project \
  -c ./configuration.json \
  -f ./constants.json \
  -o ./output-directory
```

### Example: Benchmarking "Train-Ticket"

To run the benchmark, the target project must be a **sibling directory** to SAR tool.

1. **Clone Train-Ticket:**

```bash
# In the parent directory where SAR tool is located
git clone https://github.com/FudanSELab/train-ticket
```

2. **Run Analysis:**

```bash
# From within the software-architecture-reconstruction-tool directory
cargo run -p cli --release -- \
  -p ../train-ticket \
  -c ./config/configurations/local-train-ticket-config.json \
  -o ./output-directory
```

> **Note:** The output will be generated as JSON files in the specified `./output-directory`.

---

## Full-Stack Deployment

To access the full suite of features—including metadata storage and Neo4j visualization—use the Docker-based setups.

### Option A: Development Mode (Cargo + Docker)

_Best for testing full-stack capabilities with local code changes._

0. **Create .env**
   In synthesizer, extractor-runtime, manager and constant-scanner, create .env files from their respective .env.example files.

1. **Start Databases:**

```bash
# Start Neo4J (Architectural Views)
cd synthesizer && docker compose up -d && cd ..

# Start Postgres (Constants)
docker run -d --name constant-scanner-db -e POSTGRES_PASSWORD=password -e POSTGRES_USER=postgres -e POSTGRES_DB=constant-scanner-db -p 5433:5432 postgres

# Run diesel (ORM)
cd constant-scanner
cargo install diesel_cli --no-default-features --features postgres

## If you see 'rust-lld: error: unable to find library -lpq' error run:
dnf install postgresql-devel # or your OS equivalent

diesel setup
diesel migration run

cd ..

# Start Postgres (Metadata)
docker run -d --name manager-db -e POSTGRES_PASSWORD=password -e POSTGRES_USER=postgres -e POSTGRES_DB=manager-db -p 5432:5432 postgres

cd manager
diesel setup
diesel migration run
cd ..
```

2. **Launch Services:**

```bash
./run_info.sh
./run_client.sh
```

3. Create testing metadata:

- Navigate to: `http://localhost:8081/swagger-ui/#/`
- Create Project
- Create Codebase - pass created Project UUID
- Create Configuration - pass ./config/configurations/train-ticket-config.json, into the `configuration_data` field
- Create Commit - pass created Codebase UUID and Configuration UUID

4. **Upload Project:** Navigate to `http://localhost:3000/upload.html`, fill in the metadata, and upload the project folder.

---

### Option B: Production Mode (Full Docker Compose)

_Best for a stable, fully containerized environment._

```bash
# In the root directory
docker compose up -d
```

#### Service Map & Interfaces

Once running, you can access the following services:

| Service               | Endpoint         | Purpose                         |
| --------------------- | ---------------- | ------------------------------- |
| **Synthesizer**       | `localhost:8080` | View synthesis engine           |
| **Manager**           | `localhost:8081` | Project & Commit metadata       |
| **Extractor-Runtime** | `localhost:8082` | `tree-sitter` extraction worker |
| **Constant-Scanner**  | `localhost:8083` | Static constant analysis        |
| **Neo4j Interface**   | `localhost:7475` | Graph visualization & storage   |

**Neo4j Database Port Mapping:**

- **:7687** — Context Map DB
- **:7688** — SDG (System Dependency Graph) DB
- **:7689** — IMCG (Inter-Microservice Call Graph) DB
