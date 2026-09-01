#!/usr/bin/env bash
#
# Launcher for the demo with the Fluendo face anonymizer (flufaceanonymizer).
#
# The anonymizer plugin links against GStreamer >= 1.28.2 but must run against
# this project's custom GStreamer build. The Fluendo package ships its own
# glib 2.78 and GStreamer 1.28.2 libraries, which conflict with our build
# (our GStreamer needs the system glib >= 2.80). This script sets the correct
# library/plugin ordering so only the raven/CUDA/gfx libraries are pulled from
# the Fluendo bundle, while our GStreamer and the system glib win.
#
# Usage:
#   scripts/run-ai-filter.sh [app args...]
#
# Requires the anonymizer package to be installed (via `apt-get install`),
# which installs into /opt/fluendo/fluanonymizer.

set -euo pipefail

GST_PREFIX="${GST_PREFIX:-/opt/gstreamer/dev/ins}"
GST_LIB="$GST_PREFIX/lib/x86_64-linux-gnu"
RS_PLUGINS="${RS_PLUGINS:-/home/dnieto/workspace/gst-plugins-rs/target/release}"
FLU_DIR="${FLU_DIR:-/opt/fluendo/fluanonymizer}"

if [ ! -f "$FLU_DIR/lib/gstreamer-1.0/libgstfluanonymizer.so" ]; then
    echo "ERROR: flufaceanonymizer plugin not found at $FLU_DIR" >&2
    echo "       Install it first, e.g.:" >&2
    echo "         sudo apt-get install -y ./fluanonymizer_*.deb" >&2
    exit 1
fi

# Library ordering is critical:
#   1. our GStreamer libs (custom DSC/SEI build)
#   2. system glib >= 2.80 (Fluendo ships 2.78, which is too old for us)
#   3. Fluendo lib dir (raven engine, TBB, gfx, CUDA)
export LD_LIBRARY_PATH="$GST_LIB:$GST_PREFIX/lib:/usr/lib/x86_64-linux-gnu:$FLU_DIR/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

# Bin dir exposes the raven/slang tools (mirrors the vendor environment.sh).
export PATH="$FLU_DIR/bin:$PATH"

# Plugin path: our plugins first, then a dir containing ONLY the anonymizer
# plugin (the bundle's other 1.28.2 plugins are incompatible and would fail).
PLUGIN_LINK_DIR="${TMPDIR:-/tmp}/c2pa-dsc-fluanonymizer"
mkdir -p "$PLUGIN_LINK_DIR"
ln -sf "$FLU_DIR/lib/gstreamer-1.0/libgstfluanonymizer.so" "$PLUGIN_LINK_DIR/"
export GST_PLUGIN_PATH="$GST_LIB/gstreamer-1.0:$RS_PLUGINS:$PLUGIN_LINK_DIR${GST_PLUGIN_PATH:+:$GST_PLUGIN_PATH}"

cd "$(dirname "$0")/.."

# Strip a leading "--" so the script can be used exactly like `cargo run --`.
args=()
for arg in "$@"; do
    if [ "${#args[@]}" -eq 0 ] && [ "$arg" = "--" ]; then
        continue
    fi
    args+=("$arg")
done
set -- "${args[@]}"

if [ -n "${C2PA_DSC_BIN:-}" ]; then
    exec "$C2PA_DSC_BIN" "$@"
else
    exec cargo run -- "$@"
fi
