#!/bin/bash
#
# build_frameworks.sh — Regenerate UniFFI bindings and build the FFI xcframeworks.
#

set -euo pipefail

if [ "$(uname -s)" != "Darwin" ]; then
    echo "error: build_frameworks.sh builds Apple xcframeworks and must run on macOS." >&2
    exit 1
fi

cd "$(dirname "$0")"
ROOT="$(pwd)"
OUT="$ROOT/platforms/apple"
GEN_SWIFT="$OUT/Sources/Kit/Generated"
FFI_HEADERS="$OUT/FFIHeaders"
STAGING="$ROOT/.build_frameworks"

CRATE="panel-of-experts-ffi"
lib_name="panel_of_experts_ffi"
module="panel_of_experts_ffiFFI"

echo "==> Building Rust crate (release)…"
cargo build --release --lib

rm -rf "$STAGING"
mkdir -p "$STAGING" "$GEN_SWIFT" "$FFI_HEADERS"

echo "==> Processing $CRATE (module: $module)…"

# 1. Regenerate Swift bindings + C header + modulemap from the built dylib.
bindgen_dir="$STAGING/bindgen"
mkdir -p "$bindgen_dir"
cargo run -q -p panel-of-experts-ffi --bin uniffi-bindgen -- \
    generate --library "target/release/lib${lib_name}.dylib" \
    --language swift --out-dir "$bindgen_dir"

# 2. Assemble a framework bundle.
fw="$STAGING/$module.framework"
rm -rf "$fw"
mkdir -p "$fw/Versions/A/Headers" "$fw/Versions/A/Modules" "$fw/Versions/A/Resources"

# Create symlinks
cd "$fw"
ln -sf A Versions/Current
ln -sf Versions/Current/Headers Headers
ln -sf Versions/Current/Modules Modules
ln -sf Versions/Current/Resources Resources
ln -sf Versions/Current/${module} ${module}
cd "$ROOT"

cp "$bindgen_dir/${module}.h" "$fw/Versions/A/Headers/${module}.h"

cat > "$fw/Versions/A/Modules/module.modulemap" <<EOF
framework module ${module} {
    header "${module}.h"
    export *
}
EOF

# Copy the dynamic library as the framework binary
cp "target/release/lib${lib_name}.dylib" "$fw/Versions/A/${module}"

cat > "$fw/Versions/A/Resources/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key><string>en</string>
    <key>CFBundleExecutable</key><string>${module}</string>
    <key>CFBundleIdentifier</key><string>technology.mcfarlin.${module}</string>
    <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
    <key>CFBundleName</key><string>${module}</string>
    <key>CFBundlePackageType</key><string>FMWK</string>
    <key>CFBundleShortVersionString</key><string>1.0</string>
    <key>CFBundleVersion</key><string>1</string>
    <key>CFBundleSupportedPlatforms</key><array><string>MacOSX</string></array>
    <key>MinimumOSVersion</key><string>14.0</string>
</dict>
</plist>
EOF

# 3. Wrap in an xcframework.
rm -rf "$OUT/$module.xcframework"
xcodebuild -create-xcframework -framework "$fw" -output "$OUT/$module.xcframework"

# 4. Copy the generated Swift bindings.
cp "$bindgen_dir/${lib_name}.swift" "$GEN_SWIFT/${lib_name}.swift"

# 5. Copy the headers and modulemap.
mkdir -p "$FFI_HEADERS"
cp "$bindgen_dir/${module}.h" "$FFI_HEADERS/${module}.h"
cp "$fw/Modules/module.modulemap" "$FFI_HEADERS/module.modulemap"

rm -rf "$STAGING"
echo "==> Done. Framework-style xcframework written to $OUT/$module.xcframework"
