#!/usr/bin/env bash
# build-android.sh — Build the Deskdrop Android APK
#
# Requirements:
#   - Rust + cargo-ndk:  cargo install cargo-ndk
#   - Android SDK + NDK (ANDROID_HOME and ANDROID_NDK_HOME set)
#   - JDK 17+
#
# Usage:
#   ./build-android.sh [--debug|--release] [--install]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CORE_DIR="${REPO_ROOT}/cliprelay-core"
ANDROID_DIR="${REPO_ROOT}/platforms/android"
SDK_ROOT_DEFAULT="${HOME}/Library/Android/sdk"
BREW_SDK_ROOT="/opt/homebrew/share/android-commandlinetools"
BREW_NDK_ROOT="/opt/homebrew/share/android-ndk"
ICON_SRC="${REPO_ROOT}/platforms/macos/ClipRelay/Resources/AppIconSource.png"
LOCAL_PROPERTIES_PATH="${ANDROID_DIR}/local.properties"
BRAND_DRAWABLE_DIR="${ANDROID_DIR}/app/src/main/res/drawable-nodpi"

BUILD_TYPE="release"
DO_INSTALL=false

for arg in "$@"; do
    case "$arg" in
        --debug)   BUILD_TYPE="debug" ;;
        --release) BUILD_TYPE="release" ;;
        --install) DO_INSTALL=true ;;
    esac
done

ABIS=("aarch64-linux-android" "armv7-linux-androideabi" "x86_64-linux-android")
JNI_DIR="${ANDROID_DIR}/app/src/main/jniLibs"

log() { echo "▶ $*"; }

generate_android_icons() {
    if [[ ! -f "${ICON_SRC}" ]]; then
        log "Skipping Android icon generation; source icon not found at ${ICON_SRC}"
        return
    fi

    if ! command -v sips >/dev/null 2>&1; then
        log "Skipping Android icon generation; sips is unavailable"
        return
    fi

    log "Generating Android launcher icons from ${ICON_SRC}..."

    while IFS=: read -r density size; do
        out_dir="${ANDROID_DIR}/app/src/main/res/mipmap-${density}"
        mkdir -p "${out_dir}"
        sips -s format png -z "${size}" "${size}" "${ICON_SRC}" --out "${out_dir}/ic_launcher.png" >/dev/null
        cp "${out_dir}/ic_launcher.png" "${out_dir}/ic_launcher_round.png"
    done <<'EOF'
mdpi:48
hdpi:72
xhdpi:96
xxhdpi:144
xxxhdpi:192
EOF
}

generate_android_brand_asset() {
    if [[ ! -f "${ICON_SRC}" ]]; then
        return
    fi

    mkdir -p "${BRAND_DRAWABLE_DIR}"
    if command -v sips >/dev/null 2>&1; then
        sips -s format png -z 512 512 "${ICON_SRC}" --out "${BRAND_DRAWABLE_DIR}/deskdrop_logo.png" >/dev/null
    else
        cp "${ICON_SRC}" "${BRAND_DRAWABLE_DIR}/deskdrop_logo.png"
    fi
}

write_local_properties() {
    if [[ -z "${ANDROID_HOME:-}" ]]; then
        log "Skipping local.properties generation; ANDROID_HOME is unset"
        return
    fi

    log "Writing Android SDK path to ${LOCAL_PROPERTIES_PATH}..."
    printf 'sdk.dir=%s\n' "${ANDROID_HOME//\\/\\\\}" > "${LOCAL_PROPERTIES_PATH}"
}

if [[ -z "${ANDROID_HOME:-}" && -d "${SDK_ROOT_DEFAULT}" ]]; then
    export ANDROID_HOME="${SDK_ROOT_DEFAULT}"
fi

if [[ -z "${ANDROID_HOME:-}" && -d "${BREW_SDK_ROOT}" ]]; then
    export ANDROID_HOME="${BREW_SDK_ROOT}"
fi

if [[ -z "${ANDROID_SDK_ROOT:-}" && -n "${ANDROID_HOME:-}" ]]; then
    export ANDROID_SDK_ROOT="${ANDROID_HOME}"
fi

if [[ -z "${ANDROID_NDK_HOME:-}" && -d "${BREW_NDK_ROOT}" ]]; then
    export ANDROID_NDK_HOME="${BREW_NDK_ROOT}"
fi

if [[ -z "${ANDROID_NDK_HOME:-}" && -n "${ANDROID_HOME:-}" && -d "${ANDROID_HOME}/ndk" ]]; then
    latest_ndk="$(find "${ANDROID_HOME}/ndk" -mindepth 1 -maxdepth 1 -type d | sort | tail -n 1)"
    if [[ -n "${latest_ndk}" ]]; then
        export ANDROID_NDK_HOME="${latest_ndk}"
    fi
fi

if [[ -f "${ANDROID_DIR}/gradlew" ]]; then
    GRADLE_CMD=("./gradlew")
elif [[ -x "/opt/homebrew/opt/gradle@8/bin/gradle" ]]; then
    GRADLE_CMD=("/opt/homebrew/opt/gradle@8/bin/gradle")
else
    GRADLE_CMD=("gradle")
fi

# ── 1. Add Rust targets ────────────────────────────────────────────────────────

log "Adding Rust Android targets..."
for target in "${ABIS[@]}"; do
    rustup target add "${target}" 2>/dev/null || true
done

# ── 2. Build native libraries ─────────────────────────────────────────────────

generate_android_icons
generate_android_brand_asset
write_local_properties

log "Building native libraries for all ABIs..."
cd "${CORE_DIR}"

# Always build Rust in release mode — debug Rust is 5-10x slower for crypto
# (ChaCha20-Poly1305 encryption dominates transfer throughput).
# The --debug/--release flag only affects the APK signing and Gradle build type.
cargo ndk \
    -t aarch64-linux-android \
    -t armv7-linux-androideabi \
    -t x86_64-linux-android \
    -o "${JNI_DIR}" \
    build --features compress --lib --release

log "JNI libs:"
find "${JNI_DIR}" -name "*.so" | while read -r f; do
    echo "  $(du -sh "${f}" | cut -f1)  ${f}"
done

# ── 3. Build APK ─────────────────────────────────────────────────────────────

log "Building Android APK (${BUILD_TYPE})..."
cd "${ANDROID_DIR}"

if [[ "${BUILD_TYPE}" == "release" ]]; then
    "${GRADLE_CMD[@]}" assembleRelease
    APK="${ANDROID_DIR}/app/build/outputs/apk/release/app-release.apk"
else
    "${GRADLE_CMD[@]}" assembleDebug
    APK="${ANDROID_DIR}/app/build/outputs/apk/debug/app-debug.apk"
fi

log "✅ APK: ${APK}  ($(du -sh "${APK}" | cut -f1))"

# ── 4. Install on connected device (optional) ────────────────────────────────

if [[ "${DO_INSTALL}" == "true" ]]; then
    log "Installing on connected device..."
    adb install -r "${APK}"
    log "✅ Installed"
fi
