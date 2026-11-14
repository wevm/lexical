#!/bin/sh
# Cargo's runner on macOS (.cargo/config.toml): runs the playground from
# inside an app bundle, `Lexical Playground (GPUI).app` beside the
# executable, so it launches as an app in the Dock and the app switcher. The
# bundle stays, to open from Finder or keep in the Dock. Tests and other
# executables run as they are.
set -eu
exe=$1
shift
[ "$(basename "$exe")" = lexical-playground-gpui ] || exec "$exe" "$@"

app="$(cd "$(dirname "$exe")" && pwd)/Lexical Playground (GPUI).app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
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
	<key>LSApplicationCategoryType</key>
	<string>public.app-category.developer-tools</string>
	<key>NSHighResolutionCapable</key>
	<true/>
</dict>
</plist>
PLIST
# A hard link, not a copy: the executable can be large in a debug build.
ln -f "$exe" "$app/Contents/MacOS/lexical-playground-gpui"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister \
  -f "$app"
exec "$app/Contents/MacOS/lexical-playground-gpui" "$@"
