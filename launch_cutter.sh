#!/usr/bin/env bash
# ==============================================================================
# Launch Cutter GUI with QIR Plugin Integration
# Author: Matvei Igumnov <matvei@iqumnov.com>
# ==============================================================================

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CUTTER_DIST="${PROJECT_DIR}/bin/cutter_dist"
CUTTER_EXEC="${CUTTER_DIST}/AppRun"

export WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-wayland-0}"
export DISPLAY="${DISPLAY:-:1}"
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
export QT_QPA_PLATFORM=xcb

if [ -z "${XAUTHORITY}" ]; then
    AUTH_FILE=$(ls -1t "${XDG_RUNTIME_DIR}/xauth_"* 2>/dev/null | head -n 1)
    [ -n "${AUTH_FILE}" ] && export XAUTHORITY="${AUTH_FILE}"
fi

export LD_LIBRARY_PATH="${CUTTER_DIST}/usr/lib:${LD_LIBRARY_PATH}"
export QT_PLUGIN_PATH="${CUTTER_DIST}/usr/plugins"

# Disable DBus Global Menu delegation so top menu bar (File, Edit, View, Help) is always drawn inside Cutter's window
export QT_QPA_PLATFORMTHEME=""
export QT_XCB_NATIVE_MENUBAR=0
export QT_AUTO_SCREEN_SCALE_FACTOR=1

# Ensure plugin is deployed cleanly to user directory
PLUGIN_DIR="${HOME}/.local/share/rizin/cutter/plugins/python"
mkdir -p "${PLUGIN_DIR}" 2>/dev/null || true
cp -u "${PROJECT_DIR}/qir_plugin/cutter_qir_dock.py" "${PLUGIN_DIR}/" 2>/dev/null || true

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
