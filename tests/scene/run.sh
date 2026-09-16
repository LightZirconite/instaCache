#!/bin/sh
# Runs the real QML scene under qmltestrunner, with `shell` replaced by
# MockShell.qml and Instagram replaced by the pages in www/. Nothing is shown on
# screen and no real microphone or camera is opened.
#
#   tests/scene/run.sh
#
# Needs qmltestrunner for Qt 6, the QtTest QML module and python3 for the local
# web server. See "Testing the scene" in AGENTS.md.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
port=8791

runner=
for candidate in /usr/lib/qt6/bin/qmltestrunner qmltestrunner6 qmltestrunner; do
    if command -v "$candidate" >/dev/null 2>&1; then
        runner=$candidate
        break
    fi
done
if [ -z "$runner" ]; then
    echo "qmltestrunner for Qt 6 not found" >&2
    exit 1
fi

work=$(mktemp -d)
server=
cleanup() {
    if [ -n "$server" ]; then
        kill "$server" 2>/dev/null || true
    fi
    rm -rf "$work"
}
trap cleanup EXIT INT TERM

# The scene reads `shell` as a context property; here it becomes a property of
# the root window, which every object in the file can see by the same name.
sed "s|@TMP@|$work|g" "$here/MockShell.qml" > "$work/MockShell.qml"
awk '
    !done && /^    id: root$/ { print; print "    property QtObject shell: MockShell {}"; done = 1; next }
    { print }
' "$repo/src/qml/main.qml" > "$work/Harness.qml"
cp "$here/tst_scene.qml" "$work/"

python3 -m http.server --bind 127.0.0.1 --directory "$here/www" "$port" >/dev/null 2>&1 &
server=$!
sleep 1

cd "$work"
QT_QPA_PLATFORM=offscreen \
QTWEBENGINE_CHROMIUM_FLAGS=--use-fake-device-for-media-stream \
    "$runner" -input tst_scene.qml
