#!/usr/bin/env bash
# Produces an unsigned AnkiFruit.ipa for sideloading.
#
# AltStore, SideStore and Sideloadly all re-sign the app with the user's own
# Apple ID at install time, so shipping an unsigned payload is both correct and
# necessary here — a signature of ours would just be stripped.
#
# macOS only.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_DIR="${BUILD_DIR:-$REPO_ROOT/build}"
ARCHIVE="$BUILD_DIR/AnkiFruit.xcarchive"

rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR"

echo "==> building web UI"
(cd "$REPO_ROOT/web" && npm ci && npm run build)

echo "==> staging web UI into the app bundle"
rm -rf "$REPO_ROOT/ios/App/Resources/web"
mkdir -p "$REPO_ROOT/ios/App/Resources"
cp -R "$REPO_ROOT/web/dist" "$REPO_ROOT/ios/App/Resources/web"

echo "==> building AnkiFruitCore.xcframework"
"$REPO_ROOT/tools/build-xcframework.sh"

echo "==> generating Xcode project"
(cd "$REPO_ROOT/ios/App" && xcodegen generate)

echo "==> archiving"
xcodebuild archive \
    -project "$REPO_ROOT/ios/App/AnkiFruit.xcodeproj" \
    -scheme AnkiFruit \
    -configuration Release \
    -destination 'generic/platform=iOS' \
    -archivePath "$ARCHIVE" \
    CODE_SIGNING_ALLOWED=NO \
    CODE_SIGNING_REQUIRED=NO \
    CODE_SIGN_IDENTITY=""

echo "==> packaging .ipa"
PAYLOAD="$BUILD_DIR/Payload"
mkdir -p "$PAYLOAD"
cp -R "$ARCHIVE/Products/Applications/AnkiFruit.app" "$PAYLOAD/"
(cd "$BUILD_DIR" && zip -qry AnkiFruit.ipa Payload)
rm -rf "$PAYLOAD"

echo "==> done"
ls -lh "$BUILD_DIR/AnkiFruit.ipa" | awk '{print "  ipa:       " $5}'
du -sh "$ARCHIVE/Products/Applications/AnkiFruit.app" | awk '{print "  app bundle: " $1}'
# The binary is what actually has to fit; the linker discards unused rslib symbols.
ls -lh "$ARCHIVE/Products/Applications/AnkiFruit.app/AnkiFruit" | awk '{print "  binary:    " $5}'
