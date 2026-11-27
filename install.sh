#!/usr/bin/env bash
# ==============================================================================
# QIR Reversing Universal Standalone Toolchain & Plugin Installer
# Automated OS/Arch detection, binary release fetcher, and Rizin/Cutter setup
# Usage: curl -sSL https://raw.githubusercontent.com/Iqumnov/Qir-Reversing/main/install.sh | bash
# Repository: https://github.com/Iqumnov/Qir-Reversing
# Author: Matvei Igumnov <matvei@iqumnov.com>
# License: Apache-2.0
# ==============================================================================

set -euo pipefail

REPO="Iqumnov/Qir-Reversing"
VERSION="${QIR_VERSION:-latest}"

# ANSI Colors (Monochrome Tactical)
BOLD="\033[1m"
DIM="\033[2m"
WHITE="\033[97m"
GREEN="\033[32m"
YELLOW="\033[33m"
RED="\033[31m"
RESET="\033[0m"

echo -e "${BOLD}${WHITE}================================================================${RESET}"
echo -e "${BOLD}${WHITE}  qir-reversing // Toolchain & Plugin Installer (${VERSION})${RESET}"
echo -e "${DIM}  Automated Multi-Target Detection & Full Environment Setup${RESET}"
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

# 3. Determine Installation Destination Directory for Binaries
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

# Resolve candidate archive names
ARCHIVE_CANDIDATES=()
if [ "${PLATFORM}" = "linux" ]; then
    if [ "${TARGET_ARCH}" = "x86_64" ]; then
        ARCHIVE_CANDIDATES=(
            "qir-rev-linux-x86_64.tar.gz"
        )
    fi
elif [ "${PLATFORM}" = "macos" ]; then
    if [ "${TARGET_ARCH}" = "arm64" ]; then
        ARCHIVE_CANDIDATES=("qir-rev-macos-arm64.tar.gz")
    elif [ "${TARGET_ARCH}" = "x86_64" ]; then
        ARCHIVE_CANDIDATES=("qir-rev-macos-x86_64.tar.gz")
    fi
fi

if [ "${#ARCHIVE_CANDIDATES[@]}" -gt 0 ] && [ "${QIR_FORCE_BUILD:-0}" != "1" ]; then
    for ARCHIVE_NAME in "${ARCHIVE_CANDIDATES[@]}"; do
        if [ "${VERSION}" = "latest" ]; then
            TARBALL_URL="https://github.com/${REPO}/releases/latest/download/${ARCHIVE_NAME}"
        else
            TARBALL_URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE_NAME}"
        fi
        echo -e "${GREEN}[+]${RESET} Checking release asset: ${DIM}${ARCHIVE_NAME}${RESET}"

        if curl -sSLf "${TARBALL_URL}" -o "${TEMP_DIR}/${ARCHIVE_NAME}" 2>/dev/null; then
            echo -e "${GREEN}[+]${RESET} Extracting payload..."
            if ! tar -xzf "${TEMP_DIR}/${ARCHIVE_NAME}" -C "${TEMP_DIR}" 2>/dev/null; then
                echo -e "${YELLOW}[!] Failed to extract ${ARCHIVE_NAME}; skipping asset.${RESET}"
                continue
            fi
            
            # 4.1 Locate and Install CLI & MCP Executables
            CLI_BIN="$(find "${TEMP_DIR}" -type f \( -name "qir-rev" -o -name "qir" \) | head -n 1 || true)"
            MCP_BIN="$(find "${TEMP_DIR}" -type f \( -name "qir-rev-mcp" -o -name "qir-mcp" \) | head -n 1 || true)"

            if [ -n "${CLI_BIN}" ] && [ -f "${CLI_BIN}" ]; then
                mkdir -p "${INSTALL_DIR}"
                install -m 755 "${CLI_BIN}" "${INSTALL_DIR}/qir-rev"
                # Create convenient aliases
                ln -sf "${INSTALL_DIR}/qir-rev" "${INSTALL_DIR}/qrev"
                ln -sf "${INSTALL_DIR}/qir-rev" "${INSTALL_DIR}/qir"

                if [ -n "${MCP_BIN}" ] && [ -f "${MCP_BIN}" ]; then
                    install -m 755 "${MCP_BIN}" "${INSTALL_DIR}/qir-rev-mcp"
                    ln -sf "${INSTALL_DIR}/qir-rev-mcp" "${INSTALL_DIR}/qir-mcp"
                fi

                # 4.2 Locate and Install Native Rizin Plugin (librz_qir.so / librz_qir.dylib)
                PLUGIN_LIB="$(find "${TEMP_DIR}" -type f \( -name "librz_qir.so" -o -name "librz_qir.dylib" \) | head -n 1 || true)"
                if [ -n "${PLUGIN_LIB}" ] && [ -f "${PLUGIN_LIB}" ]; then
                    RIZIN_PLUGIN_DIR="$HOME/.local/share/rizin/plugins"
                    mkdir -p "${RIZIN_PLUGIN_DIR}" 2>/dev/null || true
                    PLUGIN_EXT="${PLUGIN_LIB##*.}"
                    cp "${PLUGIN_LIB}" "${RIZIN_PLUGIN_DIR}/librz_qir.${PLUGIN_EXT}"
                    echo -e "${GREEN}[✓]${RESET} Rizin Native Plugin installed to: ${DIM}${RIZIN_PLUGIN_DIR}/librz_qir.${PLUGIN_EXT}${RESET}"
                fi

                # 4.3 Locate and Install Cutter GUI Python Plugin (qir_plugin)
                PLUGIN_DIR_SRC="$(find "${TEMP_DIR}" -type d -name "qir_plugin" | head -n 1 || true)"
                if [ -n "${PLUGIN_DIR_SRC}" ] && [ -d "${PLUGIN_DIR_SRC}" ]; then
                    if [ "${PLATFORM}" = "macos" ]; then
                        CUTTER_PLUGIN_DIR="$HOME/Library/Application Support/rizin/cutter/plugins/python"
                    else
                        CUTTER_PLUGIN_DIR="$HOME/.local/share/rizin/cutter/plugins/python"
                    fi
                    mkdir -p "${CUTTER_PLUGIN_DIR}" 2>/dev/null || true
                    rm -rf "${CUTTER_PLUGIN_DIR}/qir_plugin"
                    cp -r "${PLUGIN_DIR_SRC}" "${CUTTER_PLUGIN_DIR}/"
                    echo -e "${GREEN}[✓]${RESET} Cutter GUI Plugin installed to: ${DIM}${CUTTER_PLUGIN_DIR}/qir_plugin${RESET}"
                fi

                DOWNLOAD_SUCCESS=1
                echo -e "${GREEN}[✓]${RESET} Binaries & plugins deployed successfully to ${BOLD}${INSTALL_DIR}${RESET}"
                break
            fi
        fi
    done
fi

# 5. Fallback: Compile from Source via Cargo if binary download failed or skipped
if [ "${DOWNLOAD_SUCCESS}" -eq 0 ]; then
    echo -e "${YELLOW}[*]${RESET} Pre-compiled binary unavailable or skipped. Compiling via Cargo..."
    if command -v cargo &>/dev/null; then
        echo -e "${GREEN}[+]${RESET} Rust toolchain detected: $(cargo --version)"
        cargo install --git "https://github.com/${REPO}" qir-cli --force
        cargo install --git "https://github.com/${REPO}" qir-mcp --force
        CARGO_BIN_DIR="${CARGO_HOME:-$HOME/.cargo}/bin"
        EXE_EXT=""
        if [ "${PLATFORM}" = "windows" ]; then
            EXE_EXT=".exe"
        fi
        mkdir -p "${INSTALL_DIR}"
        if [ "${CARGO_BIN_DIR}" != "${INSTALL_DIR}" ]; then
            [ -f "${CARGO_BIN_DIR}/qir-rev${EXE_EXT}" ] && ln -sf "${CARGO_BIN_DIR}/qir-rev${EXE_EXT}" "${INSTALL_DIR}/qir-rev${EXE_EXT}"
            [ -f "${CARGO_BIN_DIR}/qir-rev-mcp${EXE_EXT}" ] && ln -sf "${CARGO_BIN_DIR}/qir-rev-mcp${EXE_EXT}" "${INSTALL_DIR}/qir-rev-mcp${EXE_EXT}"
        fi
        if [ -f "${INSTALL_DIR}/qir-rev${EXE_EXT}" ] || [ -L "${INSTALL_DIR}/qir-rev${EXE_EXT}" ]; then
            ln -sf "${INSTALL_DIR}/qir-rev${EXE_EXT}" "${INSTALL_DIR}/qrev${EXE_EXT}"
            ln -sf "${INSTALL_DIR}/qir-rev${EXE_EXT}" "${INSTALL_DIR}/qir${EXE_EXT}"
        fi
        if [ -f "${INSTALL_DIR}/qir-rev-mcp${EXE_EXT}" ] || [ -L "${INSTALL_DIR}/qir-rev-mcp${EXE_EXT}" ]; then
            ln -sf "${INSTALL_DIR}/qir-rev-mcp${EXE_EXT}" "${INSTALL_DIR}/qir-mcp${EXE_EXT}"
        fi
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
echo -e "\n${BOLD}${WHITE}================================================================${RESET}"
echo -e "${BOLD}${WHITE}  DEPLOYMENT VERIFICATION SUMMARY                               ${RESET}"
echo -e "${BOLD}${WHITE}================================================================${RESET}"
if command -v "${INSTALL_DIR}/qir-rev" &>/dev/null || [ -f "${INSTALL_DIR}/qir-rev" ]; then
    echo -e "${GREEN}[✓] CLI Binary:${RESET}       ${BOLD}qir-rev${RESET} (alias: ${BOLD}qrev${RESET}) at ${INSTALL_DIR}/qir-rev"
fi
if command -v "${INSTALL_DIR}/qir-rev-mcp" &>/dev/null || [ -f "${INSTALL_DIR}/qir-rev-mcp" ]; then
    echo -e "${GREEN}[✓] MCP Server:${RESET}       ${BOLD}qir-rev-mcp${RESET} at ${INSTALL_DIR}/qir-rev-mcp"
fi
if [ -f "$HOME/.local/share/rizin/plugins/librz_qir.so" ]; then
    echo -e "${GREEN}[✓] Rizin Plugin:${RESET}     $HOME/.local/share/rizin/plugins/librz_qir.so"
elif [ -f "$HOME/.local/share/rizin/plugins/librz_qir.dylib" ]; then
    echo -e "${GREEN}[✓] Rizin Plugin:${RESET}     $HOME/.local/share/rizin/plugins/librz_qir.dylib"
fi
if [ -d "$HOME/.local/share/rizin/cutter/plugins/python/qir_plugin" ]; then
    echo -e "${GREEN}[✓] Cutter GUI Dock:${RESET}  $HOME/.local/share/rizin/cutter/plugins/python/qir_plugin"
elif [ -d "$HOME/Library/Application Support/rizin/cutter/plugins/python/qir_plugin" ]; then
    echo -e "${GREEN}[✓] Cutter GUI Dock:${RESET}  $HOME/Library/Application Support/rizin/cutter/plugins/python/qir_plugin"
fi

# 8. MCP Configuration Example
echo -e "\n${BOLD}${WHITE}AI AGENT CONFIGURATION (Claude Desktop / Cursor / MCP Hosts):${RESET}"
cat << 'EOF'
{
  "mcpServers": {
    "qir-reversing": {
      "command": "qir-rev-mcp"
    }
  }
}
EOF

echo -e "\n${BOLD}${GREEN}[✓] Installation completed successfully.${RESET}\n"
