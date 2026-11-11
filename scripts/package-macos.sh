#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != Darwin ]]; then
  echo "macOS 应用打包需要在安装完整 Xcode 的 Mac 上运行。" >&2
  exit 1
fi

repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
profile=release
if [[ "${1:-}" == --debug ]]; then
  profile=debug
elif [[ $# -ne 0 ]]; then
  echo "用法：$0 [--debug]" >&2
  exit 1
fi

cd "$repo_dir"
build_dir="$repo_dir/target"
if [[ "$profile" == release ]]; then
  build_dir="$repo_dir/target/package"
  cargo build -p voon-app --release --locked --target-dir "$build_dir"
else
  cargo build -p voon-app --locked
fi

bundle="$repo_dir/dist/Voon.app"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources" "$repo_dir/target/swift-module-cache"
cp "$build_dir/$profile/voon" "$bundle/Contents/MacOS/voon.new"
chmod 755 "$bundle/Contents/MacOS/voon.new"
mv -f "$bundle/Contents/MacOS/voon.new" "$bundle/Contents/MacOS/voon"
cat > "$bundle/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleIdentifier</key><string>com.chiyiangel.voon</string>
  <key>CFBundleName</key><string>Voon</string>
  <key>CFBundleDisplayName</key><string>Voon</string>
  <key>CFBundleExecutable</key><string>voon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>CFBundleIconFile</key><string>Voon</string>
  <key>CFBundleDevelopmentRegion</key><string>zh_CN</string>
  <key>CFBundleLocalizations</key><array><string>zh_CN</string></array>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSPrincipalClass</key><string>NSApplication</string>
</dict></plist>
PLIST

swift -module-cache-path "$repo_dir/target/swift-module-cache" "$repo_dir/scripts/macos-icon.swift" "$bundle/Contents/Resources"
iconutil -c icns "$bundle/Contents/Resources/Voon.iconset" -o "$bundle/Contents/Resources/Voon.icns"
plutil -lint "$bundle/Contents/Info.plist"
echo "已生成：$bundle"
echo "未添加代码签名或公证流程。"
