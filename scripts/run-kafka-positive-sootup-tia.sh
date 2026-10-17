#!/usr/bin/env bash
set -euo pipefail

# Runs the existing controlled Kafka change with the independent SootUp adapter.
repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

exec env \
  JAVA_CALLGRAPH_PROVIDER=sootup \
  JAVA_CALLGRAPH_ADAPTER_JAR="${JAVA_CALLGRAPH_ADAPTER_JAR:-$repo_root/sootup-callgraph/target/sootup-callgraph-all.jar}" \
  TIA_POSITIVE_OUTPUT_DIR="${TIA_POSITIVE_OUTPUT_DIR:-/tmp/sar-kafka-positive-sootup-tia}" \
  "$repo_root/scripts/run-kafka-positive-tia.sh"
