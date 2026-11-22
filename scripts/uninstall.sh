#!/bin/sh
# Remove the installed launcher and its login item. Settings and usage data
# (~/.config/rusty-spotlight, ~/Library/Application Support/rusty-spotlight) are kept.
set -u
LABEL="local.rusty-spotlight"
launchctl bootout "gui/$(id -u)/$LABEL" 2>/dev/null || true
pkill -x rusty-spotlight 2>/dev/null || true
rm -f "$HOME/Library/LaunchAgents/$LABEL.plist"
rm -rf "/Applications/Rusty Spotlight.app"
echo "Removed. Settings and usage data were kept."
