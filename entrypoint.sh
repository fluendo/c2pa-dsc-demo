#!/bin/bash
set -e

BIN=/root/c2pa-dsc-live-demo/target/release/c2pa-dsc-live-demo

echo "=== c2pa-dsc-live-demo ==="
echo "Args: $@"
echo "=========================="

exec "$BIN" "$@"
