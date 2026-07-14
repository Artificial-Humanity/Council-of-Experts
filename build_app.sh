#!/bin/bash
set -e

echo "==> Building Panel of Experts SwiftUI App (Release)..."
swift build --package-path platforms/apple -c release --product PanelOfExpertsApp

echo "==> Creating App Bundle Structure..."
APP_DIR="PanelOfExperts.app"
CONTENTS_DIR="$APP_DIR/Contents"
MACOS_DIR="$CONTENTS_DIR/MacOS"
FRAMEWORKS_DIR="$CONTENTS_DIR/Frameworks"

# Clean previous build
rm -rf "$APP_DIR"

mkdir -p "$MACOS_DIR"
mkdir -p "$FRAMEWORKS_DIR"

echo "==> Copying executable binary..."
cp "platforms/apple/.build/release/PanelOfExpertsApp" "$MACOS_DIR/PanelOfExpertsApp"

echo "==> Copying linked frameworks..."
# Copy the macOS framework from xcframework target
cp -R "platforms/apple/panel_of_experts_ffiFFI.xcframework/macos-arm64/panel_of_experts_ffiFFI.framework" "$FRAMEWORKS_DIR/"

echo "==> Generating Info.plist..."
cat <<EOF > "$CONTENTS_DIR/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>PanelOfExpertsApp</string>
    <key>CFBundleIdentifier</key>
    <string>technology.mcfarlin.panel-of-experts</string>
    <key>CFBundleName</key>
    <string>Panel of Experts</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>LSMinimumSystemVersion</key>
    <string>14.0</string>
</dict>
</plist>
EOF

# Ensure execute permissions on binary
chmod +x "$MACOS_DIR/PanelOfExpertsApp"

echo "==> App bundle built successfully: $APP_DIR"
echo "==> You can launch it using: open $APP_DIR"
