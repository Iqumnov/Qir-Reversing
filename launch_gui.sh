#!/usr/bin/env bash
# ==============================================================================
# Launch Standalone Quantum Reverse Engineering GUI
# Author: Matvei Igumnov <matvei@iqumnov.com>
# ==============================================================================

set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export PYTHONPATH="${PROJECT_DIR}:${PROJECT_DIR}/qir_plugin${PYTHONPATH:+:${PYTHONPATH}}"
export WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-wayland-0}"
export DISPLAY="${DISPLAY:-:1}"
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"

if [ -z "${XAUTHORITY:-}" ] && [ -d "${XDG_RUNTIME_DIR}" ]; then
    AUTH_FILE="$(ls -1t "${XDG_RUNTIME_DIR}/xauth_"* 2>/dev/null | head -n 1 || true)"
    [ -n "${AUTH_FILE}" ] && export XAUTHORITY="${AUTH_FILE}"
fi

PYTHON_BIN="${PROJECT_DIR}/venv/bin/python3"
if [ ! -x "${PYTHON_BIN}" ]; then
    PYTHON_BIN="python3"
fi

exec "${PYTHON_BIN}" "${PROJECT_DIR}/qir_plugin/qir_gui.py" "$@"

