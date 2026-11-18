#!/bin/sh
# macOS counterpart of scripts/windows.ps1.
#
#   ./scripts/macos.sh [Setup|Check|Test|Dev|Build|Package] [--universal]
#
# Package writes dist/EarControl.app, a DMG, a ZIP and a SHA-256 manifest.
# --universal builds one app for Apple silicon and Intel. Set CODESIGN_IDENTITY
# to a "Developer ID Application" identity to sign for distribution; otherwise
# the app is signed ad hoc, which runs on the Mac that built it.
set -eu

task=${1:-Build}
universal=0
[ "${2:-}" = --universal ] && universal=1
case $task in
  Setup|Check|Test|Dev|Build|Package) ;;
  *) echo "Usage: $0 [Setup|Check|Test|Dev|Build|Package] [--universal]" >&2; exit 2 ;;
esac
[ "$(uname -s)" = Darwin ] || { echo "Run this script on macOS." >&2; exit 1; }

cd "$(dirname "$0")/.."
project=$PWD
if [ -x "$project/.tools/cargo/bin/cargo" ]; then
  export CARGO_HOME="$project/.tools/cargo" RUSTUP_HOME="$project/.tools/rustup"
  PATH="$CARGO_HOME/bin:$PATH"
fi
command -v cargo >/dev/null || { echo "Install Rust from https://rustup.rs first." >&2; exit 1; }
export MACOSX_DEPLOYMENT_TARGET=11.0

case $task in
  Setup) exec cargo fetch --locked ;;
  Check)
    cargo fmt --all -- --check
    exec cargo clippy --workspace --all-targets --locked -- -D warnings ;;
  Test) exec cargo test --workspace --locked ;;
  Dev) exec cargo run --locked ;;
esac

version=$(sed -n 's/^version = "\(.*\)"$/\1/p' desktop/Cargo.toml | head -n 1)
if [ $universal = 1 ]; then
  arch=universal
  for target in aarch64-apple-darwin x86_64-apple-darwin; do
    command -v rustup >/dev/null && rustup target add "$target" >/dev/null
    cargo build --release --locked --target "$target"
  done
  mkdir -p target/universal-apple-darwin/release
  binary=target/universal-apple-darwin/release/earcontrol
  lipo -create -output "$binary" \
    target/aarch64-apple-darwin/release/earcontrol \
    target/x86_64-apple-darwin/release/earcontrol
else
  arch=$(uname -m)
  cargo build --release --locked
  binary=target/release/earcontrol
fi
[ "$task" = Build ] && { echo "Built $binary"; exit 0; }

dist="$project/dist"
app="$dist/EarControl.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$binary" "$app/Contents/MacOS/EarControl"
cp LICENSE desktop/assets/THIRD-PARTY-NOTICES.txt "$app/Contents/Resources/"

work=$(mktemp -d "${TMPDIR:-/tmp}/earcontrol-package.XXXXXX")
trap 'rm -rf "$work"' EXIT
qlmanage -t -s 1024 -o "$work" desktop/assets/macos-icon.svg >/dev/null 2>&1
[ -f "$work/macos-icon.svg.png" ] || { echo "Could not render the app icon." >&2; exit 1; }
iconset="$work/EarControl.iconset"
mkdir "$iconset"
for size in 16 32 128 256 512; do
  sips -z $size $size "$work/macos-icon.svg.png" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z $double $double "$work/macos-icon.svg.png" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/EarControl.icns"

cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleDisplayName</key><string>EarControl</string>
  <key>CFBundleExecutable</key><string>EarControl</string>
  <key>CFBundleIconFile</key><string>EarControl</string>
  <key>CFBundleIdentifier</key><string>app.earcontrol.desktop</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>EarControl</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>
  <key>LSMinimumSystemVersion</key><string>$MACOSX_DEPLOYMENT_TARGET</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSHumanReadableCopyright</key><string>MIT License</string>
</dict>
</plist>
EOF

if [ -n "${CODESIGN_IDENTITY:-}" ]; then
  codesign --force --options runtime --timestamp --sign "$CODESIGN_IDENTITY" "$app"
else
  codesign --force --sign - "$app"
fi
codesign --verify --strict "$app"

stage="$work/dmg"
mkdir "$stage"
cp -R "$app" "$stage/"
ln -s /Applications "$stage/Applications"
name="EarControl-$version-macos-$arch"
rm -f "$dist/$name.dmg" "$dist/$name.zip" "$dist/$name-SHA256SUMS.txt"
hdiutil create -quiet -volname EarControl -srcfolder "$stage" -ov -format UDZO "$dist/$name.dmg"
ditto -c -k --keepParent "$app" "$dist/$name.zip"
(cd "$dist" && shasum -a 256 "$name.dmg" "$name.zip" > "$name-SHA256SUMS.txt")
echo "Packaged $app"
echo "         dist/$name.dmg"
echo "         dist/$name.zip"
