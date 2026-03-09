#!/bin/bash
set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

REPO="njenia/dpdk-top"
BINARY_NAME="dpdk-top"

detect_platform() {
    OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
    ARCH="$(uname -m)"

    case "$ARCH" in
        x86_64)  ARCH="amd64" ;;
        arm64|aarch64) ARCH="arm64" ;;
        *)
            echo -e "${RED}Error: Unsupported architecture: $ARCH${NC}"
            echo "Build from source: cargo build --release"
            exit 1
            ;;
    esac

    case "$OS" in
        linux)  PLATFORM="linux" ;;
        darwin) PLATFORM="darwin" ;;
        *)
            echo -e "${RED}Error: Unsupported OS: $OS${NC}"
            echo "Build from source: cargo build --release"
            exit 1
            ;;
    esac
}

pick_install_dir() {
    if [ -n "$INSTALL_DIR" ]; then
        return
    fi

    if [ -w /usr/local/bin ]; then
        INSTALL_DIR="/usr/local/bin"
    elif command -v sudo &> /dev/null && sudo -n true 2>/dev/null; then
        INSTALL_DIR="/usr/local/bin"
        USE_SUDO=1
    else
        INSTALL_DIR="${HOME}/.local/bin"
        mkdir -p "$INSTALL_DIR"
    fi
}

install() {
    detect_platform
    pick_install_dir

    VERSION="${VERSION:-latest}"
    if [ "$VERSION" = "latest" ]; then
        DOWNLOAD_URL="https://github.com/$REPO/releases/latest/download/${BINARY_NAME}-${PLATFORM}-${ARCH}.tar.gz"
    else
        DOWNLOAD_URL="https://github.com/$REPO/releases/download/${VERSION}/${BINARY_NAME}-${PLATFORM}-${ARCH}.tar.gz"
    fi

    echo -e "${GREEN}Installing ${BINARY_NAME}...${NC}"
    echo "Platform: ${PLATFORM}-${ARCH}"
    echo "Target:   ${INSTALL_DIR}"

    TMP_DIR=$(mktemp -d)
    trap "rm -rf $TMP_DIR" EXIT

    if command -v curl &> /dev/null; then
        curl -fsSL "$DOWNLOAD_URL" -o "$TMP_DIR/${BINARY_NAME}.tar.gz"
    elif command -v wget &> /dev/null; then
        wget -q "$DOWNLOAD_URL" -O "$TMP_DIR/${BINARY_NAME}.tar.gz"
    else
        echo -e "${RED}Error: curl or wget is required${NC}"
        exit 1
    fi

    cd "$TMP_DIR"
    tar -xzf "${BINARY_NAME}.tar.gz"

    if [ "${USE_SUDO:-0}" = "1" ]; then
        sudo mv "$BINARY_NAME" "$INSTALL_DIR/"
        sudo chmod +x "$INSTALL_DIR/$BINARY_NAME"
    else
        mv "$BINARY_NAME" "$INSTALL_DIR/"
        chmod +x "$INSTALL_DIR/$BINARY_NAME"
    fi

    echo -e "${GREEN}✓ Installed ${BINARY_NAME} to ${INSTALL_DIR}/${BINARY_NAME}${NC}"

    if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
        echo ""
        echo -e "${YELLOW}Add to your PATH:${NC}"
        echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
        echo ""
        echo "Then run: sudo dpdk-top"
    else
        echo "Run 'sudo dpdk-top' to get started."
    fi
}

install
