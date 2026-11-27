#!/usr/bin/env bash
set -euo pipefail

echo "========================================================"
echo "  QIR Reversing Build Pipeline"
echo "  Target: Native Rust CLI, MCP Server, Rizin Plugin, Web"
echo "  Author: Matvei Igumnov <matvei@iqumnov.com>"
echo "========================================================"

export PATH="$HOME/.cargo/bin:$PATH"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${ROOT_DIR}"
TARGET_DIR="${CARGO_TARGET_DIR:-target}"
UNAME_S="$(uname -s || echo Linux)"
case "${UNAME_S}" in
    Darwin*) RIZIN_PLUGIN_NAME="librz_qir.dylib" ;;
    MINGW*|MSYS*|CYGWIN*) RIZIN_PLUGIN_NAME="rz_qir.dll" ;;
    *) RIZIN_PLUGIN_NAME="librz_qir.so" ;;
esac

echo ""
echo "[1/5] Running Mathematical Core Unit Tests..."
cargo test --workspace --all-features

echo ""
echo "[2/5] Building Release Binaries (CLI, MCP Server, Rizin Plugin)..."
cargo build --release --workspace
echo "[✓] Binaries generated at:"
echo "    - CLI:          ${TARGET_DIR}/release/qir-rev"
echo "    - MCP Server:   ${TARGET_DIR}/release/qir-rev-mcp"
echo "    - Rizin Plugin: ${TARGET_DIR}/release/${RIZIN_PLUGIN_NAME}"

echo ""
echo "[3/5] Compiling Pure Rust WebAssembly Module (wasm32-unknown-unknown)..."
cargo build --release --target wasm32-unknown-unknown --manifest-path core/Cargo.toml
mkdir -p web/wasm
cp "${TARGET_DIR}/wasm32-unknown-unknown/release/qir_core.wasm" web/wasm/qir_core.wasm
chmod 644 web/wasm/qir_core.wasm
echo "[✓] WebAssembly module generated at: web/wasm/qir_core.wasm"

echo ""
echo "[4/5] Verifying CLI, Python, WebAssembly & Lean Suites..."
"${TARGET_DIR}/release/qir-rev" disasm samples/bell_state.bc
"${TARGET_DIR}/release/qir-rev" deobf --qasm samples/obfuscated_circuit.ll
"${TARGET_DIR}/release/qir-rev" verify samples/bell_state.ll
"${TARGET_DIR}/release/qir-rev" verify samples/obfuscated_circuit.ll

if [ -x "${ROOT_DIR}/venv/bin/python" ]; then
    "${ROOT_DIR}/venv/bin/python" -m unittest discover -s tests -v
elif command -v python3 &>/dev/null; then
    python3 -m unittest discover -s tests -v
fi

if command -v node &>/dev/null; then
    node web/test_suite.js
fi

if command -v lake &>/dev/null; then
    (cd "${ROOT_DIR}/proofs" && lake build)
else
    echo "[*] lake not present, skipping Lean 4 proof build."
fi

echo ""
echo "[5/5] Web Dashboard & Cloudflare Edge Status:"
echo "    - WebAssembly binary: web/wasm/qir_core.wasm (Pure Rust)"
echo "    - Serve locally via:  node web/server.js (or python3 -m http.server 8088 -d web)"
echo "    - Cloudflare Pages / Workers ready in web/"
echo ""
echo "========================================================"
echo "  QIR Reversing Build Complete. Zero mocks, zero fake data."
echo "========================================================"
