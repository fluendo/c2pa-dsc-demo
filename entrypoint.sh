#!/bin/bash
set -e

BIN=/root/c2pa-dsc-live-demo/target/release/c2pa-dsc-live-demo

# If the Fluendo anonymizer bundle is mounted (native install exposed via
# -v /opt/fluendo/fluanonymizer:/opt/fluendo/fluanonymizer:ro), wire up the
# library/plugin environment so the flufaceanonymizer element can load.
# This mirrors scripts/run-ai-filter.sh but for the container (GStreamer is
# under /usr/local here).
if [ -d /opt/fluendo/fluanonymizer ]; then
    export LD_LIBRARY_PATH="/usr/local/lib/x86_64-linux-gnu:/usr/local/lib:/usr/lib/x86_64-linux-gnu:/opt/fluendo/fluanonymizer/lib:${LD_LIBRARY_PATH:-}"
    export PATH="/opt/fluendo/fluanonymizer/bin:${PATH:-}"

    # Expose only the anonymizer plugin (the bundle's other 1.28.2 plugins are
    # incompatible with our GStreamer build).
    PLUGIN_DIR=/tmp/fluanonymizer-plugin
    mkdir -p "$PLUGIN_DIR"
    ln -sf /opt/fluendo/fluanonymizer/lib/gstreamer-1.0/libgstfluanonymizer.so "$PLUGIN_DIR/"
    export GST_PLUGIN_PATH="/usr/local/lib/x86_64-linux-gnu/gstreamer-1.0:$PLUGIN_DIR:${GST_PLUGIN_PATH:-}"
fi

echo "=== c2pa-dsc-live-demo ==="
echo "Args: $@"
echo "=========================="

exec "$BIN" "$@"
