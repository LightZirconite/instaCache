#!/bin/sh
# GramCache installer.
#
#   ./install.sh              install for the current user (no root needed)
#   ./install.sh --system     install for every user (needs root)
#   ./install.sh --help       full option list
#
# Works both from a release archive (a `gramcache` binary sits next to this
# script) and from a source checkout (the binary is built with cargo).

set -eu

APP=gramcache
APP_NAME="GramCache"
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

PREFIX=""
MODE=user
FORCE_BUILD=0
SKIP_DEP_CHECK=0

# ---------------------------------------------------------------- output ----

if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    C_RESET=$(printf '\033[0m'); C_BOLD=$(printf '\033[1m')
    C_RED=$(printf '\033[31m'); C_GREEN=$(printf '\033[32m')
    C_YELLOW=$(printf '\033[33m'); C_BLUE=$(printf '\033[34m')
else
    C_RESET=''; C_BOLD=''; C_RED=''; C_GREEN=''; C_YELLOW=''; C_BLUE=''
fi

step()  { printf '%s==>%s %s\n' "$C_BLUE$C_BOLD" "$C_RESET" "$*"; }
ok()    { printf '%s  ok%s %s\n' "$C_GREEN" "$C_RESET" "$*"; }
warn()  { printf '%swarn%s %s\n' "$C_YELLOW" "$C_RESET" "$*" >&2; }
die()   { printf '%serror%s %s\n' "$C_RED" "$C_RESET" "$*" >&2; exit 1; }

usage() {
    cat <<EOF
$APP_NAME installer

USAGE:
    ./install.sh [OPTIONS]

OPTIONS:
    --user            Install into \$HOME/.local (default, no root required).
    --system          Install into /usr/local for every user (needs root).
    --prefix <DIR>    Install into an arbitrary prefix.
    --build           Build from source even if a prebuilt binary is present.
    --skip-deps       Do not check for the WebKitGTK runtime library.
    -h, --help        Show this message.

WHAT GETS INSTALLED:
    <prefix>/bin/$APP
    <prefix>/share/applications/$APP.desktop
    <prefix>/share/icons/hicolor/scalable/apps/$APP.svg
    <prefix>/share/icons/hicolor/<size>/apps/$APP.png   (when a converter exists)

Run ./uninstall.sh to remove all of it.
EOF
}

# ------------------------------------------------------------- arguments ----

while [ $# -gt 0 ]; do
    case "$1" in
        --user)       MODE=user ;;
        --system)     MODE=system ;;
        --prefix)     shift; [ $# -gt 0 ] || die "--prefix requires a directory"; PREFIX="$1"; MODE=custom ;;
        --prefix=*)   PREFIX="${1#--prefix=}"; MODE=custom ;;
        --build)      FORCE_BUILD=1 ;;
        --skip-deps)  SKIP_DEP_CHECK=1 ;;
        -h|--help)    usage; exit 0 ;;
        *)            die "unknown option '$1' (try --help)" ;;
    esac
    shift
done

case "$MODE" in
    user)   PREFIX="${PREFIX:-$HOME/.local}" ;;
    system) PREFIX="${PREFIX:-/usr/local}" ;;
esac

BIN_DIR="$PREFIX/bin"
DESKTOP_DIR="$PREFIX/share/applications"
ICON_DIR="$PREFIX/share/icons/hicolor"

# `install` needs elevation only when the prefix is not writable by us.
SUDO=""
needs_root() {
    target="$PREFIX"
    while [ ! -e "$target" ] && [ "$target" != "/" ]; do target=$(dirname "$target"); done
    [ ! -w "$target" ]
}
if needs_root; then
    if [ "$(id -u)" = 0 ]; then
        SUDO=""
    elif command -v sudo >/dev/null 2>&1; then
        SUDO="sudo"
        step "$PREFIX is not writable; sudo will be used"
    elif command -v doas >/dev/null 2>&1; then
        SUDO="doas"
    else
        die "$PREFIX is not writable and neither sudo nor doas is available"
    fi
fi
# $SUDO is intentionally unquoted: it is either empty or a single command name.
# shellcheck disable=SC2086
run() { if [ -n "$SUDO" ]; then $SUDO "$@"; else "$@"; fi; }

# ---------------------------------------------------- runtime dependency ----

# The package that provides libwebkit2gtk-4.1.so.0, per distribution family.
runtime_packages() {
    id=""; like=""
    if [ -r /etc/os-release ]; then
        # shellcheck disable=SC1091
        . /etc/os-release
        id="${ID:-}"; like="${ID_LIKE:-}"
    fi
    case " $id $like " in
        *" arch "*|*" archlinux "*|*" cachyos "*|*" manjaro "*|*" endeavouros "*)
            echo "sudo pacman -S --needed webkit2gtk-4.1 gtk3" ;;
        *" debian "*|*" ubuntu "*)
            echo "sudo apt install libwebkit2gtk-4.1-0 libgtk-3-0" ;;
        *" fedora "*|*" rhel "*|*" centos "*)
            echo "sudo dnf install webkit2gtk4.1 gtk3" ;;
        *" suse "*|*" opensuse "*)
            echo "sudo zypper install libwebkit2gtk-4_1-0 gtk3" ;;
        *" alpine "*)
            echo "sudo apk add webkit2gtk-4.1 gtk+3.0" ;;
        *" void "*)
            echo "sudo xbps-install -S webkit2gtk gtk+3" ;;
        *" gentoo "*)
            echo "sudo emerge net-libs/webkit-gtk:4.1 x11-libs/gtk+:3" ;;
        *)
            echo "" ;;
    esac
}

check_runtime_dependency() {
    [ "$SKIP_DEP_CHECK" = 1 ] && return 0

    found=0
    if command -v ldconfig >/dev/null 2>&1; then
        ldconfig -p 2>/dev/null | grep -q 'libwebkit2gtk-4\.1\.so' && found=1
    fi
    if [ "$found" = 0 ]; then
        for dir in /usr/lib /usr/lib64 /usr/lib/x86_64-linux-gnu /usr/lib/aarch64-linux-gnu /usr/local/lib; do
            [ -e "$dir/libwebkit2gtk-4.1.so.0" ] && { found=1; break; }
        done
    fi

    if [ "$found" = 1 ]; then
        ok "WebKitGTK 4.1 runtime found"
        return 0
    fi

    warn "libwebkit2gtk-4.1.so.0 was not found on this system."
    cmd=$(runtime_packages)
    if [ -n "$cmd" ]; then
        printf '     Install it with:\n       %s%s%s\n' "$C_BOLD" "$cmd" "$C_RESET" >&2
    else
        printf '     Install your distribution'\''s WebKitGTK 4.1 and GTK 3 runtime packages.\n' >&2
    fi
    warn "Continuing anyway; $APP_NAME will not start until it is installed."
}

# ------------------------------------------------------------- the binary ----

resolve_binary() {
    if [ "$FORCE_BUILD" = 0 ]; then
        for candidate in "$SCRIPT_DIR/$APP" "$SCRIPT_DIR/bin/$APP" "$SCRIPT_DIR/target/release/$APP"; do
            [ -x "$candidate" ] && { echo "$candidate"; return 0; }
        done
    fi

    [ -f "$SCRIPT_DIR/Cargo.toml" ] || die "no prebuilt $APP binary here and no Cargo.toml to build from"
    command -v cargo >/dev/null 2>&1 || die "cargo is required to build from source — see https://rustup.rs"

    step "Building $APP_NAME from source (this takes a few minutes the first time)" >&2
    ( cd "$SCRIPT_DIR" && cargo build --release ) >&2 || die "the build failed"
    echo "$SCRIPT_DIR/target/release/$APP"
}

# --------------------------------------------------------------- install ----

install_icons() {
    run install -Dm644 "$SCRIPT_DIR/assets/$APP.svg" "$ICON_DIR/scalable/apps/$APP.svg"
    ok "icon  -> $ICON_DIR/scalable/apps/$APP.svg"

    # Some panels and docks still prefer raster icons; generate them when a
    # converter happens to be available, and quietly skip otherwise.
    converter=""
    if command -v rsvg-convert >/dev/null 2>&1; then converter=rsvg
    elif command -v magick >/dev/null 2>&1; then converter=magick
    elif command -v convert >/dev/null 2>&1; then converter=convert
    elif command -v inkscape >/dev/null 2>&1; then converter=inkscape
    fi
    [ -z "$converter" ] && return 0

    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    for size in 16 22 24 32 48 64 128 256 512; do
        out="$tmp/$size.png"
        case "$converter" in
            rsvg)     rsvg-convert -w "$size" -h "$size" "$SCRIPT_DIR/assets/$APP.svg" -o "$out" 2>/dev/null || continue ;;
            magick)   magick -background none -density 384 "$SCRIPT_DIR/assets/$APP.svg" -resize "${size}x${size}" "$out" 2>/dev/null || continue ;;
            convert)  convert -background none -density 384 "$SCRIPT_DIR/assets/$APP.svg" -resize "${size}x${size}" "$out" 2>/dev/null || continue ;;
            inkscape) inkscape -w "$size" -h "$size" -o "$out" "$SCRIPT_DIR/assets/$APP.svg" >/dev/null 2>&1 || continue ;;
        esac
        [ -s "$out" ] && run install -Dm644 "$out" "$ICON_DIR/${size}x${size}/apps/$APP.png"
    done
    rm -rf "$tmp"
    trap - EXIT
    ok "raster icons generated with $converter"
}

refresh_caches() {
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        run gtk-update-icon-cache -f -t "$ICON_DIR" >/dev/null 2>&1 || true
    fi
    if command -v update-desktop-database >/dev/null 2>&1; then
        run update-desktop-database "$DESKTOP_DIR" >/dev/null 2>&1 || true
    fi
    ok "desktop and icon caches refreshed"
}

check_path() {
    case ":$PATH:" in
        *":$BIN_DIR:"*) return 0 ;;
    esac
    warn "$BIN_DIR is not on your PATH."
    printf '     %s%s%s is still in your application menu, but to launch it from a\n' "$C_BOLD" "$APP_NAME" "$C_RESET" >&2
    printf '     terminal add this to your shell profile:\n\n' >&2
    printf '       export PATH="%s:$PATH"\n\n' "$BIN_DIR" >&2
}

# ------------------------------------------------------------------ main ----

printf '\n%s%s installer%s\n\n' "$C_BOLD" "$APP_NAME" "$C_RESET"

step "Checking the runtime dependency"
check_runtime_dependency

step "Locating the binary"
BINARY=$(resolve_binary)
ok "using $BINARY"

step "Installing into $PREFIX"
run install -Dm755 "$BINARY" "$BIN_DIR/$APP"
ok "binary -> $BIN_DIR/$APP"

run install -Dm644 "$SCRIPT_DIR/$APP.desktop" "$DESKTOP_DIR/$APP.desktop"
ok "menu entry -> $DESKTOP_DIR/$APP.desktop"

install_icons
refresh_caches
check_path

printf '\n%sDone.%s %s is installed.\n\n' "$C_GREEN$C_BOLD" "$C_RESET" "$APP_NAME"
printf '  Launch it from your application menu, or run:  %s%s%s\n' "$C_BOLD" "$APP" "$C_RESET"
printf '  Remove it again with:                          %s./uninstall.sh%s\n\n' "$C_BOLD" "$C_RESET"
