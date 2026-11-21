#!/usr/bin/env bash
# ==============================================================================
# qir-reversing Universal Standalone Toolchain Installer
# Automated OS/Arch detection, binary release fetcher, and Cargo fallback
# Usage: curl -sSL https://raw.githubusercontent.com/Iqumnov/Qir-Reversing/main/install.sh | bash
# Repository: https://github.com/Iqumnov/Qir-Reversing
# Author: Matvei Igumnov <matvei@iqumnov.com>
# License: Apache-2.0
# ==============================================================================

set -euo pipefail

REPO="Iqumnov/Qir-Reversing"
VERSION="${QIR_VERSION:-v0.1.0}"

# ANSI Colors (Monochrome Tactical)
BOLD="\033[1m"
DIM="\033[2m"
WHITE="\033[97m"
GREEN="\033[32m"
YELLOW="\033[33m"
RED="\033[31m"
RESET="\033[0m"

echo -e "${BOLD}${WHITE}================================================================${RESET}"
echo -e "${BOLD}${WHITE}  qir-reversing // Universal Toolchain Installer (${VERSION})${RESET}"
echo -e "${DIM}  Automated OS/Arch Detection & Standalone Binary Setup${RESET}"
echo -e "${BOLD}${WHITE}================================================================${RESET}\n"

# 1. Detect Operating System
OS="$(uname -s)"
case "${OS}" in
    Linux*)  PLATFORM="linux" ;;
    Darwin*) PLATFORM="macos" ;;
    MINGW*|MSYS*|CYGWIN*) PLATFORM="windows" ;;
    *)
        echo -e "${RED}[!] Unsupported operating system: ${OS}${RESET}"
        exit 1
        ;;
esac

# 2. Detect CPU Architecture
ARCH="$(uname -m)"
case "${ARCH}" in
    x86_64|amd64)   TARGET_ARCH="x86_64" ;;
    arm64|aarch64)  TARGET_ARCH="arm64" ;;
    *)
        echo -e "${YELLOW}[!] Architecture ${ARCH} may require compiling from source via Cargo.${RESET}"
        TARGET_ARCH="${ARCH}"
        ;;
esac

echo -e "${GREEN}[+]${RESET} Detected Platform: ${BOLD}${PLATFORM}-${TARGET_ARCH}${RESET}"

# 3. Determine Installation Destination Directory
if [ -n "${QIR_INSTALL_DIR:-}" ]; then
    INSTALL_DIR="${QIR_INSTALL_DIR}"
elif [ -d "$HOME/.cargo/bin" ] && [[ ":$PATH:" == *":$HOME/.cargo/bin:"* ]]; then
    INSTALL_DIR="$HOME/.cargo/bin"
elif [ -d "$HOME/.local/bin" ]; then
    INSTALL_DIR="$HOME/.local/bin"
else
    INSTALL_DIR="$HOME/.local/bin"
    mkdir -p "${INSTALL_DIR}"
fi

echo -e "${GREEN}[+]${RESET} Installation target directory: ${DIM}${INSTALL_DIR}${RESET}"

# 4. Attempt Pre-compiled Standalone Binary Download
DOWNLOAD_SUCCESS=0
TEMP_DIR="$(mktemp -d)"
cleanup() { rm -rf "${TEMP_DIR}"; }
trap cleanup EXIT

# Resolve archive name based on platform and architecture
if [ "${PLATFORM}" = "linux" ]; then
    ARCHIVE_NAME="qir-${VERSION}-linux-x86_64.tar.gz"
elif [ "${PLATFORM}" = "macos" ]; then
    if [ "${TARGET_ARCH}" = "arm64" ]; then
        ARCHIVE_NAME="qir-macos-arm64.tar.gz"
    else
        ARCHIVE_NAME="qir-macos-x86_64.tar.gz"
    fi
fi

if [ -n "${ARCHIVE_NAME:-}" ] && [ "${QIR_FORCE_BUILD:-0}" != "1" ]; then
    TARBALL_URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE_NAME}"
    echo -e "${GREEN}[+]${RESET} Fetching pre-compiled release: ${DIM}${TARBALL_URL}${RESET}"

    if curl -sSLf "${TARBALL_URL}" -o "${TEMP_DIR}/${ARCHIVE_NAME}" 2>/dev/null; then
        echo -e "${GREEN}[+]${RESET} Extracting binaries..."
        tar -xzf "${TEMP_DIR}/${ARCHIVE_NAME}" -C "${TEMP_DIR}"
        
        # Locate extracted binaries
        CLI_BIN="$(find "${TEMP_DIR}" -type f -name "qir" | head -n 1)"
        MCP_BIN="$(find "${TEMP_DIR}" -type f -name "qir-mcp" | head -n 1)"
        PLUGIN_LIB="$(find "${TEMP_DIR}" -type f -name "librz_qir.so" | head -n 1 || true)"

        if [ -n "${CLI_BIN}" ] && [ -f "${CLI_BIN}" ]; then
            mkdir -p "${INSTALL_DIR}"
            install -m 755 "${CLI_BIN}" "${INSTALL_DIR}/qir"
            [ -n "${MCP_BIN}" ] && install -m 755 "${MCP_BIN}" "${INSTALL_DIR}/qir-mcp"
            if [ -n "${PLUGIN_LIB}" ] && [ -f "${PLUGIN_LIB}" ]; then
                mkdir -p "$HOME/.local/share/rizin/plugins" 2>/dev/null || true
                cp "${PLUGIN_LIB}" "$HOME/.local/share/rizin/plugins/" 2>/dev/null || true
            fi
            DOWNLOAD_SUCCESS=1
            echo -e "${GREEN}[✓]${RESET} Standalone binaries installed successfully to ${BOLD}${INSTALL_DIR}${RESET}"
        fi
    fi
fi

# 5. Fallback: Compile from Source via Cargo if binary download failed or skipped
if [ "${DOWNLOAD_SUCCESS}" -eq 0 ]; then
    echo -e "${YELLOW}[*]${RESET} Pre-compiled binary not found or skipped. Falling back to Cargo build..."
    if command -v cargo &>/dev/null; then
        echo -e "${GREEN}[+]${RESET} Rust toolchain detected: $(cargo --version)"
        cargo install --git "https://github.com/${REPO}" qir-cli qir-mcp --force
        INSTALL_DIR="${CARGO_HOME:-$HOME/.cargo}/bin"
        echo -e "${GREEN}[✓]${RESET} Binaries compiled and installed via Cargo."
    else
        echo -e "${RED}[!] Error: Pre-compiled binary unavailable for ${PLATFORM}-${TARGET_ARCH} and Cargo is not installed.${RESET}"
        echo -e "    Please install Rust via: ${BOLD}curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh${RESET}"
        exit 1
    fi
fi

# 6. Verify PATH
echo ""
if [[ ":$PATH:" != *":${INSTALL_DIR}:"* ]]; then
    echo -e "${YELLOW}[!] Notice:${RESET} ${INSTALL_DIR} is not in your system \$PATH."
    echo -e "    Add it to your shell configuration file (~/.bashrc, ~/.zshrc):"
    echo -e "    ${BOLD}export PATH=\"${INSTALL_DIR}:\$PATH\"${RESET}"
fi

# 7. Verification Summary
if command -v "${INSTALL_DIR}/qir" &>/dev/null; then
    echo -e "${GREEN}[✓] qir (CLI):${RESET} Ready at ${INSTALL_DIR}/qir"
fi
if command -v "${INSTALL_DIR}/qir-mcp" &>/dev/null; then
    echo -e "${GREEN}[✓] qir-mcp (AI Agent MCP Server):${RESET} Ready at ${INSTALL_DIR}/qir-mcp"
fi

# 8. MCP Configuration Example
echo -e "\n${BOLD}${WHITE}AI AGENT CONFIGURATION (Claude Desktop / Cursor / Antigravity):${RESET}"
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
