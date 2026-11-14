#!/usr/bin/env bash
# ==============================================================================
# Master Demonstration Script: LLVM QIR Reverse Engineering Suite
# Student: Igumnov (Group 231-351)
# ==============================================================================

set -e
PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VENV_PYTHON="${PROJECT_DIR}/venv/bin/python3"
RIZIN_BIN="${PROJECT_DIR}/bin/bin/rizin"
CUTTER_APPIMAGE="${PROJECT_DIR}/bin/Cutter.AppImage"

export PYTHONPATH="${PROJECT_DIR}:${PROJECT_DIR}/qir_plugin"

echo -e "\033[1;36m┌────────────────────────────────────────────────────────────────────────┐\033[0m"
echo -e "\033[1;36m│   LLVM QUANTUM INTERMEDIATE REPRESENTATION (QIR) REVERSING SUITE       │\033[0m"
echo -e "\033[1;36m│   Student: Igumnov | Group: 231-351 | Course: Reverse Engineering      │\033[0m"
echo -e "\033[1;36m└────────────────────────────────────────────────────────────────────────┘\033[0m\n"

echo -e "\033[1;32m[STAGE 1] Compiling Dummy Probe Plugin for Rizin (C architecture module)...\033[0m"
make -C "${PROJECT_DIR}/dummy_module" clean
make -C "${PROJECT_DIR}/dummy_module"
echo -e "\033[1;37m[✓] Dummy Rizin Plugin compiled:\033[0m ${PROJECT_DIR}/dummy_module/librz_dummy_qir.so\n"

echo -e "\033[1;32m[STAGE 2] Generating Quantum Bytecode Demos (LR2 Constructs & Obfuscation)...\033[0m"
${VENV_PYTHON} "${PROJECT_DIR}/demo_programs/generate_demos.py"
echo ""

echo -e "\033[1;32m[STAGE 3] Executing Automated Unit Tests & Validation...\033[0m"
${VENV_PYTHON} -m unittest discover -s "${PROJECT_DIR}/tests/" -v
echo ""

echo -e "\033[1;32m[STAGE 4] Disassembling Clean LR2 Quantum Bytecode (lr2_quantum_demo.bc)...\033[0m"
${VENV_PYTHON} "${PROJECT_DIR}/qir_plugin/qir_disasm.py" "${PROJECT_DIR}/demo_programs/lr2_quantum_demo.bc"
echo ""

echo -e "\033[1;32m[STAGE 5] Analyzing & Deobfuscating Quantum Program (lr2_obfuscated.bc)...\033[0m"
${VENV_PYTHON} "${PROJECT_DIR}/qir_plugin/qir_deobfuscator.py" "${PROJECT_DIR}/demo_programs/lr2_obfuscated.bc"
echo ""

echo -e "\033[1;32m[STAGE 6] Verifying Rizin & Cutter Toolchain Availability...\033[0m"
if [ -f "${RIZIN_BIN}" ]; then
    echo -e "  [✓] Standalone Rizin Core Binary : ${RIZIN_BIN}"
    "${RIZIN_BIN}" -v | head -n 1
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

