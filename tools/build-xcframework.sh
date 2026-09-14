#!/usr/bin/env bash
# Builds ankifruit-core for Apple targets and assembles the xcframework that
# ios/Package.swift expects at ios/Frameworks/AnkiFruitCore.xcframework.
#
# macOS only — cross-compiling to Apple targets needs Xcode's toolchain.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROFILE="${PROFILE:-release}"
OUT="$REPO_ROOT/ios/Frameworks/AnkiFruitCore.xcframework"

# rslib links SQLite and ring through cc; both need a floor deployment target.
export IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-15.0}"

"$REPO_ROOT/tools/write-buildhash.sh"

TARGETS=(aarch64-apple-ios aarch64-apple-ios-sim)

for target in "${TARGETS[@]}"; do
    echo "==> building $target ($PROFILE)"
    (cd "$REPO_ROOT/rust" && cargo build --profile "$PROFILE" --target "$target" -p ankifruit-core)
done

rm -rf "$OUT"
mkdir -p "$(dirname "$OUT")"

args=()
for target in "${TARGETS[@]}"; do
    args+=(-library "$REPO_ROOT/rust/target/$target/$PROFILE/libankifruit_core.a")
    args+=(-headers "$REPO_ROOT/rust/ankifruit-core/include")
done

xcodebuild -create-xcframework "${args[@]}" -output "$OUT"
echo "==> wrote $OUT"
du -sh "$OUT"
