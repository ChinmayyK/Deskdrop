#!/usr/bin/env bash
# build-macos.sh — Build the Deskdrop.app bundle for macOS
#
# Requirements:
#   - Rust toolchain (cargo)
#   - Xcode command-line tools (xcode-select --install)
#   - Apple Developer ID (for notarization — optional for local builds)
#
# Usage:
#   ./build-macos.sh [--release] [--notarize]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CORE_DIR="${REPO_ROOT}/cliprelay-core"
MACOS_DIR="${REPO_ROOT}/platforms/macos"
SOURCE_DIR_NAME="ClipRelay"
PRODUCT_NAME="Deskdrop"
BUILD_TYPE="${1:---release}"
APP_BUNDLE="${MACOS_DIR}/build/${PRODUCT_NAME}.app"
TARGET_DIR="${REPO_ROOT}/target/release"
ICON_SRC="${REPO_ROOT}/platforms/macos/${SOURCE_DIR_NAME}/Resources/AppIconSource.png"
STATUS_ICON_SRC="${MACOS_DIR}/${SOURCE_DIR_NAME}/Resources/StatusBarSource.png"

log() { echo "▶ $*"; }

# ── 1. Build Rust library + daemon ────────────────────────────────────────────

log "Building Rust core (${BUILD_TYPE})..."
cd "${REPO_ROOT}"
cargo build --release -p cliprelay-core --features compress --lib --bin cliprelay-daemon

DYLIB_SRC="${TARGET_DIR}/libcliprelay_core.dylib"
DAEMON_SRC="${TARGET_DIR}/cliprelay-daemon"

# ── 2. Create .app bundle skeleton ───────────────────────────────────────────

log "Creating ${PRODUCT_NAME}.app bundle..."
rm -rf "${APP_BUNDLE}"
mkdir -p "${APP_BUNDLE}/Contents/"{MacOS,Frameworks,Resources}

# Copy dylib.
cp "${DYLIB_SRC}" "${APP_BUNDLE}/Contents/Frameworks/libcliprelay_core.dylib"
cp "${DAEMON_SRC}" "${APP_BUNDLE}/Contents/MacOS/cliprelay-daemon"
chmod +x "${APP_BUNDLE}/Contents/MacOS/cliprelay-daemon"

# Fix dylib install name.
install_name_tool \
    -id "@rpath/libcliprelay_core.dylib" \
    "${APP_BUNDLE}/Contents/Frameworks/libcliprelay_core.dylib"

# ── 3. Compile Swift app ─────────────────────────────────────────────────────

log "Compiling Swift sources..."
SWIFT_FILES=()
while IFS= read -r file; do
    # Include all files found in the source directory
    SWIFT_FILES+=("${file}")
done < <(find "${MACOS_DIR}/${SOURCE_DIR_NAME}" -name '*.swift' | sort)

SDK_PATH="$(xcrun --sdk macosx --show-sdk-path)"
MACOS_TARGET="arm64-apple-macos13.0"

swiftc \
    "${SWIFT_FILES[@]}" \
    -import-objc-header "${MACOS_DIR}/${SOURCE_DIR_NAME}/ClipRelayBridge.h" \
    -sdk "${SDK_PATH}" \
    -target "${MACOS_TARGET}" \
    -framework AppKit \
    -framework SwiftUI \
    -framework Carbon \
    -framework UserNotifications \
    -F "${APP_BUNDLE}/Contents/Frameworks" \
    -L "${APP_BUNDLE}/Contents/Frameworks" \
    -lcliprelay_core \
    -Xlinker -rpath -Xlinker @executable_path/../Frameworks \
    -o "${APP_BUNDLE}/Contents/MacOS/${PRODUCT_NAME}"

# ── 4. Copy resources ─────────────────────────────────────────────────────────

cp "${MACOS_DIR}/${SOURCE_DIR_NAME}/Info.plist" "${APP_BUNDLE}/Contents/Info.plist"
cp "${STATUS_ICON_SRC}" "${APP_BUNDLE}/Contents/Resources/StatusBarIcon.png"

# Generate AppIcon.icns from the bundled source PNG.
if [[ -f "${ICON_SRC}" ]]; then
    log "Generating app icon..."
    ICON_TMP_DIR="$(mktemp -d /tmp/deskdrop-icon.XXXXXX)"
    ICONSET_DIR="${ICON_TMP_DIR}/AppIcon.iconset"
    mkdir -p "${ICONSET_DIR}"
    for size in 16 32 128 256 512; do
        sips -s format png -z "${size}" "${size}" "${ICON_SRC}" --out "${ICONSET_DIR}/icon_${size}x${size}.png" >/dev/null
        retina_size=$((size * 2))
        sips -s format png -z "${retina_size}" "${retina_size}" "${ICON_SRC}" --out "${ICONSET_DIR}/icon_${size}x${size}@2x.png" >/dev/null
    done
    iconutil -c icns "${ICONSET_DIR}" -o "${APP_BUNDLE}/Contents/Resources/AppIcon.icns"
    rm -rf "${ICON_TMP_DIR}"
fi

# ── 5. Code sign ─────────────────────────────────────────────────────────────

IDENTITY="${CODESIGN_IDENTITY:-"-"}"   # "-" = ad-hoc for local builds
log "Code signing with identity: ${IDENTITY}"

codesign \
    --force \
    --deep \
    --sign "${IDENTITY}" \
    --entitlements "${MACOS_DIR}/${SOURCE_DIR_NAME}/ClipRelay.entitlements" \
    --options runtime \
    "${APP_BUNDLE}"

# ── 6. Verify ────────────────────────────────────────────────────────────────

log "Verifying bundle..."
codesign --verify --deep --strict "${APP_BUNDLE}"
spctl --assess --type exec "${APP_BUNDLE}" 2>/dev/null || \
    log "(spctl: unsigned build — expected for ad-hoc signing)"

log "✅ Built: ${APP_BUNDLE}"

# ── 7. Optional: create DMG ──────────────────────────────────────────────────

if command -v create-dmg &>/dev/null; then
    log "Creating DMG..."
    create-dmg \
        --volname "Deskdrop" \
        --window-size 600 400 \
        --icon-size 128 \
        --app-drop-link 400 200 \
        "${MACOS_DIR}/build/Deskdrop.dmg" \
        "${APP_BUNDLE}"
    log "✅ DMG: ${MACOS_DIR}/build/Deskdrop.dmg"
fi
