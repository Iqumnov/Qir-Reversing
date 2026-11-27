#!/usr/bin/env bash
# ==============================================================================
# Master Demonstration Script: LLVM QIR Reverse Engineering Suite
# Author: Matvei Igumnov <matvei@iqumnov.com>
# ==============================================================================

set -euo pipefail
PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VENV_PYTHON="${PROJECT_DIR}/venv/bin/python3"
if [ ! -x "${VENV_PYTHON}" ]; then
    VENV_PYTHON="python3"
fi
RIZIN_BIN="${PROJECT_DIR}/bin/cutter_dist/usr/bin/rizin"
[ ! -x "${RIZIN_BIN}" ] && RIZIN_BIN="${PROJECT_DIR}/bin/bin/rizin"
CUTTER_APPIMAGE="${PROJECT_DIR}/bin/Cutter.AppImage"

export PYTHONPATH="${PROJECT_DIR}:${PROJECT_DIR}/qir_plugin${PYTHONPATH:+:${PYTHONPATH}}"

echo -e "\033[1;36m┌────────────────────────────────────────────────────────────────────────┐\033[0m"
echo -e "\033[1;36m│   LLVM QUANTUM INTERMEDIATE REPRESENTATION (QIR) REVERSING SUITE       │\033[0m"
echo -e "\033[1;36m│   Author: Matvei Igumnov <matvei@iqumnov.com>                          │\033[0m"
echo -e "\033[1;36m└────────────────────────────────────────────────────────────────────────┘\033[0m\n"

echo -e "\033[1;32m[STAGE 1] Compiling Probe Plugin for Rizin (C architecture module)...\033[0m"
DUMMY_DIR="${PROJECT_DIR}/plugins/dummy"
if [ -d "${PROJECT_DIR}/bin/include/librz" ] || pkg-config --exists rz_asm 2>/dev/null; then
    make -C "${DUMMY_DIR}" clean
    make -C "${DUMMY_DIR}"
    echo -e "\033[1;37m[✓] Rizin Probe Plugin compiled:\033[0m ${DUMMY_DIR}/librz_dummy_qir.so\n"
else
    echo -e "\033[1;33m[*] Skipping C probe compilation (Rizin dev headers not present in bin/include/librz).\033[0m\n"
fi

echo -e "\033[1;32m[STAGE 2] Checking Quantum Bytecode Samples (LR2 Constructs & Obfuscation)...\033[0m"
if [ ! -f "${PROJECT_DIR}/demo_programs/lr2_quantum_demo.bc" ] || [ ! -f "${PROJECT_DIR}/demo_programs/lr2_obfuscated.bc" ]; then
    if [ -f "${PROJECT_DIR}/demo_programs/generate_demos.py" ]; then
        "${VENV_PYTHON}" "${PROJECT_DIR}/demo_programs/generate_demos.py"
    else
        "${VENV_PYTHON}" "${PROJECT_DIR}/samples/generate_samples.py"
    fi
fi
echo ""

echo -e "\033[1;32m[STAGE 3] Executing Automated Unit Tests & Validation...\033[0m"
"${VENV_PYTHON}" -m unittest discover -s "${PROJECT_DIR}/tests/" -v
echo ""

echo -e "\033[1;32m[STAGE 4] Disassembling Clean Quantum Bytecode...\033[0m"
DEMO_CLEAN="${PROJECT_DIR}/demo_programs/lr2_quantum_demo.bc"
[ ! -f "${DEMO_CLEAN}" ] && DEMO_CLEAN="${PROJECT_DIR}/samples/bell_state.bc"
"${VENV_PYTHON}" "${PROJECT_DIR}/qir_plugin/qir_disasm.py" "${DEMO_CLEAN}"
echo ""

echo -e "\033[1;32m[STAGE 5] Analyzing & Deobfuscating Quantum Program...\033[0m"
DEMO_OBF="${PROJECT_DIR}/demo_programs/lr2_obfuscated.bc"
[ ! -f "${DEMO_OBF}" ] && DEMO_OBF="${PROJECT_DIR}/samples/obfuscated_circuit.bc"
"${VENV_PYTHON}" "${PROJECT_DIR}/qir_plugin/qir_deobfuscator.py" "${DEMO_OBF}"
echo ""

echo -e "\033[1;32m[STAGE 6] Verifying Rizin & Cutter Toolchain Availability...\033[0m"
if [ -x "${RIZIN_BIN}" ]; then
    echo -e "  [✓] Standalone Rizin Core Binary : ${RIZIN_BIN}"
    LD_LIBRARY_PATH="${PROJECT_DIR}/bin/cutter_dist/usr/lib${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}" "${RIZIN_BIN}" -v | head -n 1
fi

if [ -f "${CUTTER_APPIMAGE}" ]; then
    echo -e "  [✓] Standalone Cutter GUI Binary: ${CUTTER_APPIMAGE}"
fi

echo -e "\n\033[1;36m════════════════════════════════════════════════════════════════════════\033[0m"
echo -e "\033[1;32m[✓] ALL STAGES COMPLETED SUCCESSFULLY.\033[0m"
echo -e "To launch interactive Rizin QIR analysis shell:"
echo -e "  ${VENV_PYTHON} ${PROJECT_DIR}/qir_plugin/rz_qir_plugin.py ${PROJECT_DIR}/demo_programs/lr2_quantum_demo.bc"
echo -e "To launch Cutter GUI with QIR plugin:"
echo -e "  ${PROJECT_DIR}/launch_cutter.sh [file.bc]"
echo -e "To launch standalone Quantum GUI Inspector:"
echo -e "  ${PROJECT_DIR}/launch_gui.sh"
echo -e "\033[1;36m════════════════════════════════════════════════════════════════════════\033[0m"

