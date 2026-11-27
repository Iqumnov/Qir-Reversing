#!/usr/bin/env bash
# ==============================================================================
# Launch Cutter GUI with QIR Plugin Integration
# Author: Matvei Igumnov <matvei@iqumnov.com>
# ==============================================================================

set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CUTTER_DIST="${PROJECT_DIR}/bin/cutter_dist"
CUTTER_EXEC="${CUTTER_DIST}/AppRun"

export WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-wayland-0}"
export DISPLAY="${DISPLAY:-:1}"
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
export QT_QPA_PLATFORM="${QT_QPA_PLATFORM:-xcb}"

if [ -z "${XAUTHORITY:-}" ] && [ -d "${XDG_RUNTIME_DIR}" ]; then
    AUTH_FILE="$(ls -1t "${XDG_RUNTIME_DIR}/xauth_"* 2>/dev/null | head -n 1 || true)"
    [ -n "${AUTH_FILE}" ] && export XAUTHORITY="${AUTH_FILE}"
fi

export LD_LIBRARY_PATH="${CUTTER_DIST}/usr/lib${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"
export QT_PLUGIN_PATH="${CUTTER_DIST}/usr/plugins"

# Disable DBus Global Menu delegation so top menu bar (File, Edit, View, Help) is always drawn inside Cutter's window
export QT_QPA_PLATFORMTHEME=""
export QT_XCB_NATIVE_MENUBAR=0
export QT_AUTO_SCREEN_SCALE_FACTOR=1

# Ensure full qir_plugin package is deployed cleanly to user directory (avoid stale single-file imports)
rm -f "${CUTTER_DIST}/usr/share/rizin/cutter/plugins/python/cutter_qir_dock.py" 2>/dev/null || true
PLUGIN_DIR="${HOME}/.local/share/rizin/cutter/plugins/python"
mkdir -p "${PLUGIN_DIR}"
rm -f "${PLUGIN_DIR}/cutter_qir_dock.py"
rm -rf "${PLUGIN_DIR}/qir_plugin"
cp -r "${PROJECT_DIR}/qir_plugin" "${PLUGIN_DIR}/"
rm -rf "${PLUGIN_DIR}/qir_plugin/__pycache__"

if [ -f "${PROJECT_DIR}/target/release/librz_qir.so" ]; then
    RZ_PLUG_DIR="${HOME}/.local/share/rizin/plugins"
    mkdir -p "${RZ_PLUG_DIR}"
    cp "${PROJECT_DIR}/target/release/librz_qir.so" "${RZ_PLUG_DIR}/librz_qir.so"
fi

DEMO_FILE="${1:-}"
if [ -z "${DEMO_FILE}" ]; then
    if [ -f "${PROJECT_DIR}/samples/bell_state.bc" ]; then
        DEMO_FILE="${PROJECT_DIR}/samples/bell_state.bc"
    else
        DEMO_FILE="${PROJECT_DIR}/demo_programs/lr2_quantum_demo.bc"
    fi
fi
QSS_FILE="${PROJECT_DIR}/bin/tactical_cutter.qss"

echo "[*] Launching Cutter GUI for: ${DEMO_FILE}"
if [ -f "${QSS_FILE}" ]; then
    exec "${CUTTER_EXEC}" -stylesheet "${QSS_FILE}" "${DEMO_FILE}"
else
    exec "${CUTTER_EXEC}" "${DEMO_FILE}"
fi
