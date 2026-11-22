#!/bin/sh
# Build the launcher, install it as an app in /Applications, and set it to
# start at login. Run again after any code change to update the installed copy.
set -eu
cd "$(dirname "$0")/.."

APP="/Applications/Rusty Spotlight.app"
LABEL="local.rusty-spotlight"
AGENT="$HOME/Library/LaunchAgents/$LABEL.plist"
LOGS="$HOME/Library/Logs/rusty-spotlight"

cargo build --release

# Stop whatever copy is running, whether started by hand or at login.
launchctl bootout "gui/$(id -u)/$LABEL" 2>/dev/null || true
pkill -x rusty-spotlight 2>/dev/null || true

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$LOGS"
cp target/release/rusty-spotlight "$APP/Contents/MacOS/rusty-spotlight"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key><string>$LABEL</string>
  <key>CFBundleName</key><string>Rusty Spotlight</string>
  <key>CFBundleDisplayName</key><string>Rusty Spotlight</string>
  <key>CFBundleExecutable</key><string>rusty-spotlight</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.0.1</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>LSMinimumSystemVersion</key><string>26.0</string>
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
# Signed for this Mac only (ad hoc); enough to run here, not to give to anyone else.
# The requirement names the bundle identifier instead of this build's hash, so
# a permission granted once (Input Monitoring) still applies after a rebuild.
codesign --force --sign - -r="designated => identifier \"$LABEL\"" "$APP"

mkdir -p "$(dirname "$AGENT")"
cat > "$AGENT" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>$LABEL</string>
  <key>ProgramArguments</key>
  <array><string>$APP/Contents/MacOS/rusty-spotlight</string></array>
  <key>RunAtLoad</key><true/>
  <!-- Restart after a crash, but stay quit when quit from the menu bar. -->
  <key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>
  <key>ProcessType</key><string>Interactive</string>
  <key>StandardErrorPath</key><string>$LOGS/launcher.log</string>
  <key>StandardOutPath</key><string>$LOGS/launcher.log</string>
</dict>
</plist>
PLIST
launchctl bootstrap "gui/$(id -u)" "$AGENT"

echo "Installed $APP"
echo "Starts at login. Log: $LOGS/launcher.log"
echo "To remove: scripts/uninstall.sh"
