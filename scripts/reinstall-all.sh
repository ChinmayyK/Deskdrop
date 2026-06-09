#!/usr/bin/env bash
# reinstall-all.sh
# Completely uninstalls and rebuilds both macOS and Android apps with the latest code changes.

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

BUILD_TYPE="debug"

for arg in "$@"; do
    case "$arg" in
        --release) BUILD_TYPE="release" ;;
        --debug)   BUILD_TYPE="debug" ;;
    esac
done

echo -e "${BLUE}▶ Starting total clean and rebuild for Deskdrop (${BUILD_TYPE})...${NC}\n"

# ==========================================
# 0. Version Bump & Clean
# ==========================================
# echo -e "${BLUE}▶ Bumping version numbers...${NC}"
# python3 scripts/bump-version.py


echo -e "${BLUE}▶ Wiping macOS app data (~/Library/Application Support/deskdrop)...${NC}"
rm -rf ~/Library/Application\ Support/deskdrop

echo -e "${BLUE}▶ Wiping previous build directories and caches to save space...${NC}"
rm -rf platforms/macos/build
rm -rf platforms/android/app/build
rm -rf release
rm -f app-release.apk app-debug.apk libdeskdrop_core.dylib
cargo clean

# ==========================================
# 1. macOS Reinstall
# ==========================================
echo -e "${BLUE}▶ [macOS] Stopping existing processes...${NC}"
pkill -x Deskdrop || true
pkill -x Deskdrop || true
pkill -x deskdrop-daemon || true

echo -e "${BLUE}▶ [macOS] Building latest version...${NC}"
export SKIP_DMG=true
if [ "$BUILD_TYPE" = "release" ]; then
    bash scripts/build-macos.sh --release
else
    bash scripts/build-macos.sh --debug
fi

echo -e "${BLUE}▶ [macOS] Uninstalling old version...${NC}"
rm -rf /Applications/Deskdrop.app

echo -e "${BLUE}▶ [macOS] Installing new version to /Applications...${NC}"
cp -a platforms/macos/build/Deskdrop.app /Applications/

echo -e "${GREEN}▶ [macOS] ✅ Installed! Launching...${NC}"
open /Applications/Deskdrop.app

echo -e "\n----------------------------------------\n"

# ==========================================
# 2. Android Reinstall
# ==========================================
echo -e "${BLUE}▶ [Android] Building latest APK...${NC}"
if [ "$BUILD_TYPE" = "release" ]; then
    bash scripts/build-android.sh --release --fast-abi
    APK_PATH="platforms/android/app/build/outputs/apk/release/app-release.apk"
    APP_ID="com.deskdrop"
else
    bash scripts/build-android.sh --debug --fast-abi
    APK_PATH="platforms/android/app/build/outputs/apk/debug/app-debug.apk"
    APP_ID="com.deskdrop.debug"
fi

echo -e "${BLUE}▶ [Android] Uninstalling old version from connected device...${NC}"
adb uninstall "$APP_ID" || echo -e "${RED}Warning: $APP_ID not found on device or no device connected.${NC}"

echo -e "${BLUE}▶ [Android] Installing new version...${NC}"
if adb install -r "$APK_PATH"; then
    echo -e "${BLUE}▶ [Android] Wiping app data to prevent backup restore...${NC}"
    adb shell pm clear "$APP_ID" || true
    echo -e "${GREEN}▶ [Android] ✅ Installed! Launching...${NC}"
    adb shell monkey -p "$APP_ID" -c android.intent.category.LAUNCHER 1 > /dev/null 2>&1
else
    echo -e "${RED}▶ [Android] ❌ Failed to install APK. Is a device connected?${NC}"
fi

echo -e "\n${GREEN}🎉 All done! Both platforms have been reinstalled with the latest code (${BUILD_TYPE}).${NC}"
