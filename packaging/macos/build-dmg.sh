#!/bin/bash
# Builds a universal (Apple Silicon + Intel) Mandarina.app and wraps it in dist/Mandarina-<version>.dmg
# Requires: rustup targets aarch64-apple-darwin + x86_64-apple-darwin, node/npm.
set -e
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
VERSION=$(python3 -c 'import json;print(json.load(open("src-tauri/tauri.conf.json"))["version"])')
npx tauri build --target universal-apple-darwin --bundles app
APP="src-tauri/target/universal-apple-darwin/release/bundle/macos/Mandarina.app"

STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT
ditto "$APP" "$STAGE/Mandarina.app"
cp "packaging/macos/Install Mandarina.command" "packaging/macos/Read Me.txt" "$STAGE/"
cp LICENSE "$STAGE/LICENSE.txt"
mkdir "$STAGE/.support"
ditto "packaging/macos/Convert with Mandarina.workflow" "$STAGE/.support/Convert with Mandarina.workflow"

mkdir -p dist
DMG="dist/Mandarina-$VERSION.dmg"
rm -f "$DMG"
hdiutil create -quiet -volname "Mandarina" -srcfolder "$STAGE" -fs HFS+ -format UDZO -ov "$DMG"
# Checksum to publish next to the DMG in the release
(cd dist && shasum -a 256 "$(basename "$DMG")" > "$(basename "$DMG").sha256")
echo "Built $DMG ($(du -h "$DMG" | cut -f1))"
cat "$DMG.sha256"
