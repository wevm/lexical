#!/bin/sh
# Cargo's runner on macOS (.cargo/config.toml): runs the playground from
# inside an app bundle, `Lexical Playground (GPUI).app` beside the
# executable, so it launches as an app with its icon (AppIcon.icon: the
# Lexical mark with the GPUI mark in the corner) in the Dock and the app
# switcher, as wevm/zero's zbugs-gpui does. The
# bundle stays, to open from Finder or keep in the Dock. Tests and other
# executables run as they are.
set -eu
exe=$1
shift
[ "$(basename "$exe")" = lexical-playground-gpui ] || exec "$exe" "$@"

here=$(cd "$(dirname "$0")" && pwd)
# Absolute: actool takes relative paths from a directory of its own.
app="$(cd "$(dirname "$exe")" && pwd)/Lexical Playground (GPUI).app"
res="$app/Contents/Resources"
mkdir -p "$app/Contents/MacOS" "$res"

# The icon, through Xcode's actool, when it changed: Assets.car for macOS 26
# and later, AppIcon.icns for earlier ones.
if [ ! -e "$res/Assets.car" ] || [ -n "$(find "$here/AppIcon.icon" -newer "$res/Assets.car")" ]; then
  partial=$(mktemp)
  out=$(xcrun actool "$here/AppIcon.icon" --compile "$res" --platform macosx \
    --minimum-deployment-target 11.0 --app-icon AppIcon \
    --output-partial-info-plist "$partial" --output-format human-readable-text) ||
    { echo "$out" >&2; exit 1; }
  rm -f "$partial"
fi

cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleExecutable</key>
	<string>lexical-playground-gpui</string>
	<key>CFBundleIdentifier</key>
	<string>dev.lexical.playground-gpui</string>
	<key>CFBundleName</key>
	<string>Lexical Playground (GPUI)</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleIconFile</key>
	<string>AppIcon</string>
	<key>CFBundleIconName</key>
	<string>AppIcon</string>
	<key>LSApplicationCategoryType</key>
	<string>public.app-category.developer-tools</string>
	<key>NSHighResolutionCapable</key>
	<true/>
</dict>
</plist>
PLIST
# A hard link, not a copy: the executable can be large in a debug build.
ln -f "$exe" "$app/Contents/MacOS/lexical-playground-gpui"
# LaunchServices can see the bundle before it's whole, as when actool runs,
# and the Dock then shows the app as broken: register it as it is now.
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister \
  -f "$app"
exec "$app/Contents/MacOS/lexical-playground-gpui" "$@"
