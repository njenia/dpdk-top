#!/bin/bash
set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

REPO="njenia/dpdk-top"
BINARY_NAME="dpdk-top"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

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

install() {
    detect_platform

    VERSION="${VERSION:-latest}"
    if [ "$VERSION" = "latest" ]; then
        DOWNLOAD_URL="https://github.com/$REPO/releases/latest/download/${BINARY_NAME}-${PLATFORM}-${ARCH}.tar.gz"
    else
        DOWNLOAD_URL="https://github.com/$REPO/releases/download/${VERSION}/${BINARY_NAME}-${PLATFORM}-${ARCH}.tar.gz"
    fi

    echo -e "${GREEN}Installing ${BINARY_NAME}...${NC}"
    echo "Platform: ${PLATFORM}-${ARCH}"

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

    if [ ! -w "$INSTALL_DIR" ]; then
        sudo mv "$BINARY_NAME" "$INSTALL_DIR/"
        sudo chmod +x "$INSTALL_DIR/$BINARY_NAME"
    else
        mv "$BINARY_NAME" "$INSTALL_DIR/"
        chmod +x "$INSTALL_DIR/$BINARY_NAME"
    fi

    if command -v "$BINARY_NAME" &> /dev/null; then
        echo -e "${GREEN}✓ Installed ${BINARY_NAME} to ${INSTALL_DIR}/${BINARY_NAME}${NC}"
        echo "Run 'sudo dpdk-top' to get started."
    else
        echo -e "${YELLOW}Installed to ${INSTALL_DIR}/${BINARY_NAME}${NC}"
        echo "Make sure $INSTALL_DIR is in your PATH."
    fi
}

install
