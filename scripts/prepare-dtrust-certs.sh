#!/usr/bin/env bash
#
# Prepare D-Trust certificate files for the C2PA-DSC demo.
#
# Converts a D-Trust delivery (a password-protected .p12 plus the .cert files
# for the leaf / intermediate / root CAs) into the PEM layout the demo expects:
#
#   <out-dir>/ca.crt          -> D-TRUST root CA (trust anchor, public)
#   <out-dir>/provider.crt    -> leaf cert + intermediate CA (leaf first)
#   <out-dir>/provider.key    -> leaf private key (from the .p12)
#
# There is deliberately no ca.key: the demo only needs the root's public cert
# as a trust anchor, and src/cert.rs skips generation when these three files
# are present.
#
# Usage:
#   PIN=<your-pin> scripts/prepare-dtrust-certs.sh <input-dir> [output-dir]
#
#   - <input-dir>   directory holding the D-Trust .p12 and .cert files
#   - [output-dir]  destination (default /tmp/dtrust-certs)
#   - PIN           the p12 PIN (also accepts DTRUST_PIN)

set -euo pipefail

INPUT_DIR="${1:-.}"
OUT_DIR="${2:-/tmp/dtrust-certs}"
PIN="${PIN:-${DTRUST_PIN:-}}"

if [ -z "$PIN" ]; then
    echo "ERROR: PIN is required. Set PIN=<your-pin> or DTRUST_PIN=<your-pin>." >&2
    exit 1
fi

if [ ! -d "$INPUT_DIR" ]; then
    echo "ERROR: input directory not found: $INPUT_DIR" >&2
    exit 1
fi

# Locate the D-Trust files by naming convention.
P12_FILE="$(find "$INPUT_DIR" -maxdepth 1 -name '*.p12' -print -quit)"
ROOT_FILE="$(find "$INPUT_DIR" -maxdepth 1 -iname '*Root*.cert' -print -quit)"
INTERMEDIATE_FILE="$(find "$INPUT_DIR" -maxdepth 1 -iname '*D-TRUST*.cert' ! -iname '*Root*' -print -quit)"
LEAF_FILE="$(find "$INPUT_DIR" -maxdepth 1 -iname '*.cert' ! -iname '*D-TRUST*' -print -quit)"

for f in "$P12_FILE" "$ROOT_FILE" "$INTERMEDIATE_FILE" "$LEAF_FILE"; do
    if [ -z "$f" ] || [ ! -f "$f" ]; then
        echo "ERROR: could not locate all D-Trust files in $INPUT_DIR" >&2
        echo "       expected: one .p12, one *Root*.cert, one *D-TRUST* CA .cert," >&2
        echo "                 and one leaf .cert (not named D-TRUST)." >&2
        exit 1
    fi
done

mkdir -p "$OUT_DIR"

# D-Trust ships the .cert files as DER; the demo expects PEM everywhere
# (c2pa-rs trust anchors, CallbackSigner cert chain, and X509::from_pem).
# openssl x509 auto-detects the input format and emits PEM.
openssl x509 -in "$ROOT_FILE" -out "$OUT_DIR/ca.crt"

# Signer certificate chain: leaf first, then the intermediate CA. The demo's
# signer embeds this full chain in the C2PA manifest (pem::parse_many), while
# the DSC verifier reads only the first (leaf) cert via X509::from_pem.
openssl x509 -in "$LEAF_FILE" -out "$OUT_DIR/provider.crt"
openssl x509 -in "$INTERMEDIATE_FILE" >> "$OUT_DIR/provider.crt"

# Private key from the p12 (openssl pkcs12 emits PEM).
openssl pkcs12 -in "$P12_FILE" -nocerts -nodes \
    -passin "pass:$PIN" -out "$OUT_DIR/provider.key" 2>/dev/null
chmod 600 "$OUT_DIR/provider.key"

# Sanity check: confirm the leaf carries a C2PA-compatible EKU and the chain
# verifies leaf -> intermediate -> root.
EKU="$(openssl x509 -in "$OUT_DIR/provider.crt" -noout -text 2>/dev/null \
    | sed -n '/Extended Key Usage/,/^[[:space:]]*[^[:space:]]/p' \
    | grep -iE 'E-mail Protection|emailProtection|Document Signing|documentSigning' || true)"
if [ -z "$EKU" ]; then
    echo "WARNING: leaf cert does not appear to have a C2PA-compatible EKU" >&2
    echo "         (expected 'E-mail Protection' or 'Document Signing')." >&2
fi

if ! openssl verify -CAfile "$OUT_DIR/ca.crt" -untrusted "$OUT_DIR/provider.crt" "$LEAF_FILE" >/dev/null 2>&1; then
    echo "WARNING: certificate chain did not verify against the root CA." >&2
fi

echo "D-Trust certificates prepared in $OUT_DIR:"
echo "  ca.crt         (D-TRUST root, trust anchor)"
echo "  provider.crt   (leaf + intermediate)"
echo "  provider.key   (leaf private key, chmod 600)"
echo
echo "Run with: CERTS_DIR=$OUT_DIR docker compose up"
