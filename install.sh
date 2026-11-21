#!/usr/bin/env bash
# ==============================================================================
# qir-reversing Local Toolchain Installer
# Automated installer for qir (CLI), qir-mcp (AI Agent Server), and qir-gui
# Repository: https://github.com/Iqumnov/Qir-Reversing
# Author: Matvei Igumnov <matvei@iqumnov.com>
# License: Apache-2.0
# ==============================================================================

set -euo pipefail

REPO_URL="https://github.com/Iqumnov/Qir-Reversing"
INSTALL_PREFIX="${CARGO_HOME:-$HOME/.cargo}/bin"

# ANSI Colors (Monochrome Tactical)
BOLD="\033[1m"
DIM="\033[2m"
WHITE="\033[97m"
GREEN="\033[32m"
RED="\033[31m"
RESET="\033[0m"

echo -e "${BOLD}${WHITE}================================================================${RESET}"
echo -e "${BOLD}${WHITE}  qir-reversing // Quantum Reverse Engineering Platform${RESET}"
echo -e "${DIM}  Automated Toolchain & AI Agent Server Installer${RESET}"
echo -e "${BOLD}${WHITE}================================================================${RESET}\n"

# 1. Check for Rust & Cargo
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}[!] Cargo / Rust toolchain not detected in PATH.${RESET}"
    echo -e "    qir-reversing requires Rust to compile the high-throughput native binaries."
    echo -e "    Please install Rust via rustup: ${BOLD}curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh${RESET}"
    exit 1
fi

echo -e "${GREEN}[+]${RESET} Rust toolchain detected: $(cargo --version)"

# 2. Determine installation source (local repo or remote git)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ -f "$SCRIPT_DIR/Cargo.toml" ] && [ -d "$SCRIPT_DIR/core" ]; then
    echo -e "${GREEN}[+]${RESET} Installing from local repository source at: ${DIM}$SCRIPT_DIR${RESET}"
    cargo install --path "$SCRIPT_DIR/cli" --force
    cargo install --path "$SCRIPT_DIR/mcp" --force
else
    echo -e "${GREEN}[+]${RESET} Installing latest release from GitHub: ${DIM}$REPO_URL${RESET}"
    cargo install --git "$REPO_URL" qir-cli qir-mcp --force
fi

# 3. Verify installed binaries
echo ""
if command -v qir &> /dev/null; then
    echo -e "${GREEN}[✓] qir (CLI):${RESET} $(qir --help | head -n 1)"
else
    echo -e "${GREEN}[✓] qir (CLI):${RESET} Installed to $INSTALL_PREFIX/qir"
    echo -e "    ${DIM}Ensure $INSTALL_PREFIX is in your \$PATH.${RESET}"
fi

if command -v qir-mcp &> /dev/null; then
    echo -e "${GREEN}[✓] qir-mcp (AI Agent MCP Server):${RESET} Ready for Cursor, Claude Desktop, Antigravity"
else
    echo -e "${GREEN}[✓] qir-mcp (AI Agent MCP Server):${RESET} Installed to $INSTALL_PREFIX/qir-mcp"
fi

# 4. AI Agent Configuration Example
echo -e "\n${BOLD}${WHITE}AI AGENT MCP CONFIGURATION (Claude Desktop / Cursor / Antigravity):${RESET}"
echo -e "${DIM}Add the following to your mcpServers configuration:${RESET}"
cat << 'EOF'
{
  "mcpServers": {
    "qir-reversing": {
      "command": "qir-mcp"
    }
  }
}
EOF

echo -e "\n${BOLD}${GREEN}[✓] Installation completed successfully.${RESET}\n"

