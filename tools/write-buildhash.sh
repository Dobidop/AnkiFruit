#!/usr/bin/env bash
# rslib's build script reads its build hash from vendor/anki/out/buildhash,
# a file Anki's own Ninja build produces. We build rslib with plain cargo and
# therefore have to write it ourselves, otherwise anki::version::buildhash()
# returns an empty string and there is no way to tell which Anki is linked in.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="$REPO_ROOT/vendor/anki/out"

hash=$(git -C "$REPO_ROOT/vendor/anki" rev-parse --short HEAD)
mkdir -p "$OUT_DIR"
printf '%s' "$hash" > "$OUT_DIR/buildhash"
echo "wrote vendor/anki/out/buildhash = $hash"
