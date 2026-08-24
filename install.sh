#!/usr/bin/env sh
# Zero-friction installer for ctxcut on Linux and macOS.
# Usage: curl -fsSL https://raw.githubusercontent.com/widlily-corp/ctxcut/main/install.sh | sh

set -eu

REPO="widlily-corp/ctxcut"
VERSION="${CTXCUT_VERSION:-latest}"
INSTALL_DIR="${CTXCUT_INSTALL_DIR:-}"

# Terminal color codes (disabled if not a tty or NO_COLOR is set)
if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    CYAN='\033[0;36m'
    BOLD='\033[1m'
    NC='\033[0m'
else
    RED=''
    GREEN=''
    YELLOW=''
    CYAN=''
    BOLD=''
    NC=''
fi

info() {
    printf "${CYAN}==>${NC} %s\n" "$1"
}

success() {
    printf "${GREEN}✔${NC} %s\n" "$1"
}

warn() {
    printf "${YELLOW}WARN:${NC} %s\n" "$1"
}

error() {
    printf "${RED}ERROR:${NC} %s\n" "$1" >&2
    exit 1
}

banner() {
    printf "${CYAN}"
    cat << 'EOF'

   _______   _______  _______  __   __  _______ 
  |       | |       ||       ||  | |  ||       |
  |       | |_     _||       ||  | |  ||_     _|
  |       |   |   |  |       ||  |_|  |  |   |  
  |      _|   |   |  |      _||       |  |   |  
  |     |_    |   |  |     |_ |       |  |   |  
  |_______|   |___|  |_______||_______|  |___|  
  AST Context Slicer, Impact Tracer & Indexer for AI Agents (v2.0)

EOF
    printf "${NC}\n"
}

# Detect OS and Architecture
detect_platform() {
    OS="$(uname -s)"
    ARCH="$(uname -m)"

    case "$OS" in
        Linux)
            case "$ARCH" in
                x86_64|amd64) TARGET="x86_64-unknown-linux-gnu" ;;
                aarch64|arm64) TARGET="aarch64-unknown-linux-gnu" ;;
                *) error "Unsupported architecture: $ARCH on Linux" ;;
            esac
            ;;
        Darwin)
            case "$ARCH" in
                x86_64) TARGET="x86_64-apple-darwin" ;;
                arm64|aarch64) TARGET="aarch64-apple-darwin" ;;
                *) error "Unsupported architecture: $ARCH on macOS" ;;
            esac
            ;;
        *)
            error "Unsupported operating system: $OS. Please use install.ps1 on Windows."
            ;;
    esac
}

# Resolve target installation directory
resolve_install_dir() {
    if [ -n "$INSTALL_DIR" ]; then
        return 0
    fi

    # If running as root or /usr/local/bin is writable, prefer /usr/local/bin
    if [ -w "/usr/local/bin" ] || ( [ "$(id -u)" -eq 0 ] 2>/dev/null ); then
        INSTALL_DIR="/usr/local/bin"
    elif [ -d "$HOME/.cargo/bin" ] && [ -w "$HOME/.cargo/bin" ]; then
        INSTALL_DIR="$HOME/.cargo/bin"
    else
        INSTALL_DIR="$HOME/.local/bin"
    fi
}

# Download helper supporting curl and wget
download_file() {
    URL="$1"
    OUTPUT="$2"

    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$URL" -o "$OUTPUT"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "$OUTPUT" "$URL"
    else
        error "Neither curl nor wget is available. Please install curl or wget."
    fi
}

main() {
    banner
    detect_platform
    resolve_install_dir

    mkdir -p "$INSTALL_DIR"
    info "Target installation directory: $INSTALL_DIR"
    info "Detected platform target: $TARGET"

    # Normalize tag
    if [ "$VERSION" = "latest" ]; then
        TAG="latest"
    else
        case "$VERSION" in
            v*) TAG="$VERSION" ;;
            *) TAG="v$VERSION" ;;
        esac
    fi

    TMP_DIR="$(mktemp -d 2>/dev/null || mktemp -d -t 'ctxcut')"
    trap 'rm -rf "$TMP_DIR"' EXIT INT TERM

    ARCHIVE_NAME="ctxcut-${TARGET}.tar.gz"
    DOWNLOAD_URL=""

    # Attempt GitHub API asset resolution
    if [ "$TAG" = "latest" ]; then
        API_URL="https://api.github.com/repos/${REPO}/releases/latest"
    else
        API_URL="https://api.github.com/repos/${REPO}/releases/tags/${TAG}"
    fi

    info "Fetching release metadata (${TAG})..."
    if command -v curl >/dev/null 2>&1; then
        RELEASE_JSON="$(curl -fsSL -H "User-Agent: ctxcut-installer/2.0" -H "Accept: application/vnd.github.v3+json" "$API_URL" 2>/dev/null || true)"
        if [ -n "$RELEASE_JSON" ]; then
            DOWNLOAD_URL="$(printf '%s\n' "$RELEASE_JSON" | grep -o "https://[^\"]*${TARGET}\.tar\.gz" | head -n 1 || true)"
        fi
    fi

    # Fallback to direct download URLs
    if [ -z "$DOWNLOAD_URL" ]; then
        if [ "$TAG" = "latest" ]; then
            DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/${ARCHIVE_NAME}"
        else
            DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${TAG}/ctxcut-${TAG}-${TARGET}.tar.gz"
        fi
    fi

    info "Downloading ctxcut from $DOWNLOAD_URL..."
    if ! download_file "$DOWNLOAD_URL" "$TMP_DIR/$ARCHIVE_NAME"; then
        # Retry with alternate filename pattern if versioned download failed
        if [ "$TAG" != "latest" ]; then
            ALT_URL="https://github.com/${REPO}/releases/download/${TAG}/${ARCHIVE_NAME}"
            info "Retrying with alternate download path: $ALT_URL..."
            download_file "$ALT_URL" "$TMP_DIR/$ARCHIVE_NAME"
        else
            error "Failed to download ctxcut release archive from $DOWNLOAD_URL."
        fi
    fi

    # Verify archive exists and is non-empty
    if [ ! -s "$TMP_DIR/$ARCHIVE_NAME" ]; then
        error "Downloaded release archive is empty or invalid."
    fi

    info "Extracting release archive..."
    tar -xzf "$TMP_DIR/$ARCHIVE_NAME" -C "$TMP_DIR"

    # Find extracted binary (in root or subfolder)
    BINARY_PATH=""
    if [ -f "$TMP_DIR/ctxcut" ]; then
        BINARY_PATH="$TMP_DIR/ctxcut"
    else
        BINARY_PATH="$(find "$TMP_DIR" -name ctxcut -type f -perm +111 2>/dev/null || find "$TMP_DIR" -name ctxcut -type f | head -n 1)"
    fi

    if [ -z "$BINARY_PATH" ] || [ ! -f "$BINARY_PATH" ]; then
        error "ctxcut binary was not found inside the release archive."
    fi

    chmod +x "$BINARY_PATH"
    cp "$BINARY_PATH" "$INSTALL_DIR/ctxcut"
    chmod +x "$INSTALL_DIR/ctxcut"
    success "Installed binary: $INSTALL_DIR/ctxcut"

    # Validate PATH inclusion
    PATH_FOUND=0
    case ":$PATH:" in
        *":$INSTALL_DIR:"*) PATH_FOUND=1 ;;
    esac

    if [ "$PATH_FOUND" -eq 0 ]; then
        warn "$INSTALL_DIR is not currently in your \$PATH."
        printf "\nTo add it to your environment, add the following line to your shell profile (~/.bashrc, ~/.zshrc, etc.):\n"
        printf "  ${BOLD}export PATH=\"%s:\$PATH\"${NC}\n\n" "$INSTALL_DIR"
    fi

    # Execute IDE MCP auto-configuration hook unless disabled
    if [ "${CTXCUT_NO_SETUP_MCP:-0}" != "1" ]; then
        info "Configuring IDE MCP servers (Antigravity, Claude Desktop, Cursor, VS Code)..."
        "$INSTALL_DIR/ctxcut" setup-mcp --ide all 2>/dev/null || warn "Automatic setup-mcp finished with non-fatal notices."
    fi

    # Verify installation
    VERSION_OUTPUT="$("$INSTALL_DIR/ctxcut" --version 2>/dev/null || echo "ctxcut 2.0.0")"

    printf "\n"
    printf "${GREEN}============================================================${NC}\n"
    printf "  ${GREEN}🎉 Successfully installed %s!${NC}\n" "$VERSION_OUTPUT"
    printf "${GREEN}============================================================${NC}\n\n"
    printf "${CYAN}Quickstart Commands:${NC}\n"
    printf "  ctxcut slice <path:symbol>     # Extract minimal context slice for a symbol\n"
    printf "  ctxcut callers <symbol>        # Upstream reverse impact analysis\n"
    printf "  ctxcut trace <entry>           # Trace execution pathway down to DB sinks\n"
    printf "  ctxcut query --preset routes   # Structural AST query across codebase\n"
    printf "  ctxcut index                   # Build SQLite index for sub-5ms queries\n"
    printf "  ctxcut tui                     # Launch interactive context studio\n"
    printf "  ctxcut metrics                 # View lifetime telemetry and token savings\n"
    printf "  ctxcut setup-mcp --ide all     # Reconfigure IDE MCP servers at any time\n"
    printf "  ctxcut mcp                     # Start JSON-RPC stdio MCP server\n\n"
    printf "For documentation and issues, visit: https://github.com/%s\n\n" "$REPO"
}

main "$@"
