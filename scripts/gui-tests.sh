#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Run the atvv-gui suite on a private headless Wayland compositor.
#
# A plain `cargo test -p atvv-gui` uses the desktop Wayland session, so every
# window-building test pops a real window. This starts a temporary
# `mutter --headless` display instead and points GTK at it. Extra arguments go
# to `cargo test`.

set -eu

display_name=atvv-gui-test
runtime_dir=$(mktemp -d "${TMPDIR:-/tmp}/atvv-gui-test.XXXXXX")
chmod 700 "$runtime_dir"
compositor_pid=

cleanup() {
    if [ -n "$compositor_pid" ]; then
        kill "$compositor_pid" 2>/dev/null || true
        wait "$compositor_pid" 2>/dev/null || true
    fi
    rm -rf "$runtime_dir"
}
trap cleanup EXIT HUP INT TERM

if ! command -v mutter >/dev/null 2>&1; then
    echo "scripts/gui-tests.sh: mutter is not installed" >&2
    exit 1
fi

XDG_RUNTIME_DIR="$runtime_dir" mutter --headless --wayland \
    --wayland-display="$display_name" --no-x11 --virtual-monitor 1280x1024 \
    >"$runtime_dir/mutter.log" 2>&1 &
compositor_pid=$!

waited=0
while [ ! -S "$runtime_dir/$display_name" ]; do
    if ! kill -0 "$compositor_pid" 2>/dev/null; then
        echo "scripts/gui-tests.sh: mutter exited before the Wayland socket" >&2
        sed 's/^/  /' "$runtime_dir/mutter.log" >&2
        exit 1
    fi
    if [ "$waited" -ge 100 ]; then
        echo "scripts/gui-tests.sh: timed out waiting for the Wayland socket" >&2
        exit 1
    fi
    sleep 0.1
    waited=$((waited + 1))
done

XDG_RUNTIME_DIR="$runtime_dir" \
WAYLAND_DISPLAY="$display_name" \
GDK_BACKEND=wayland \
DISPLAY= \
    cargo test -p atvv-gui "$@"
