#!/bin/bash
set -e  # Exit on error

# Start manager project
echo "=== Starting manager project ==="
cd manager
echo "Starting manager-db container..."
docker start manager-db || echo "manager-db container not found, skipping."
echo "Running manager service..."
RUST_LOG=debug cargo watch -x run &
cd ..

# Start extractor-runtime project
echo "=== Starting extractor runtime project ==="
cd extractor-runtime
RUST_LOG=debug cargo watch -x run &
cd ..

# Start extractor-runtime project
echo "=== Starting constant scanner project ==="
cd constant-scanner
RUST_LOG=debug cargo watch -x run &
cd ..

# Start synthesizer project
echo "=== Starting synthesizer project ==="
cd synthesizer
echo "Starting docker compose services..."
docker compose up -d
RUST_LOG=debug cargo watch -x run &
cd ..

docker start constant-scanner-db || echo "constant-scanner-db container not found, skipping."
# Wait for all background processes to finish
wait
