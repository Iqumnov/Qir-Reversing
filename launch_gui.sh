#!/usr/bin/env bash
# ==============================================================================
# Launch Standalone Quantum Reverse Engineering GUI
# Student: Igumnov (Group 231-351)
# ==============================================================================

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export PYTHONPATH="${PROJECT_DIR}:${PROJECT_DIR}/qir_plugin"
export WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-wayland-0}"
export DISPLAY="${DISPLAY:-:1}"
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"

if [ -z "${XAUTHORITY}" ]; then
    AUTH_FILE=$(ls -1t "${XDG_RUNTIME_DIR}/xauth_"* 2>/dev/null | head -n 1)
    [ -n "${AUTH_FILE}" ] && export XAUTHORITY="${AUTH_FILE}"
fi

exec python3 "${PROJECT_DIR}/qir_plugin/qir_gui.py" "$@"

