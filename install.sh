#!/bin/sh
# instaCache installer.
#
#   ./install.sh              install for the current user, no root needed
#   ./install.sh --system     install for every user, needs root
#   ./install.sh --help       full option list
#
# Works from a release archive (an `instacache` binary sits next to this
# script) and from a source checkout (the binary is built with cargo).
#
# Missing system libraries are installed for you. Qt 6 WebEngine renders the
# page -- the distribution's Chromium, shared with every other Qt application,
# never a copy of our own. It decodes video itself, so unlike the WebKitGTK
# builds of instaCache there is no separate codec stack to install, except on
# Fedora which leaves H.264 out of its build on purpose.
#
# STABLE INTERFACE: `instacache --update` runs a copy of this script and passes
# `--prefix`, `--yes` and `--no-deps`. Those three must keep working, or an
# update will fail for everyone already on an older version.

set -eu

APP=instacache
APP_NAME="instaCache"
SCRIPT_DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)

PREFIX=""
MODE=user
FORCE_BUILD=0
INSTALL_DEPS=1
ASSUME_YES=0

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
    -y, --yes         Never ask; install missing system packages automatically.
    --no-deps         Never install system packages, only report what is
                      missing.
    -h, --help        Show this message.

WHAT GETS INSTALLED:
    <prefix>/bin/$APP
    <prefix>/share/applications/$APP.desktop
    <prefix>/share/icons/hicolor/scalable/apps/$APP.svg
    <prefix>/share/icons/hicolor/<size>/apps/$APP.png   (when a converter exists)
    <prefix>/share/$APP/uninstall.sh
    <prefix>/share/$APP/update.sh
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
        -y|--yes)     ASSUME_YES=1 ;;
        --no-deps)    INSTALL_DEPS=0 ;;
        # Kept so older instructions and scripts do not break.
        --skip-deps)  INSTALL_DEPS=0 ;;
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
SUPPORT_DIR="$PREFIX/share/$APP"

# ------------------------------------------------------------- elevation ----

# The installer may need root twice: to write into a system prefix, and to
# install distribution packages. Both go through the same helper.
SUDO=""
if [ "$(id -u)" != 0 ]; then
    if command -v sudo >/dev/null 2>&1; then SUDO="sudo"
    elif command -v doas >/dev/null 2>&1; then SUDO="doas"
    fi
fi

# $SUDO is intentionally unquoted: it is either empty or a single command name.
# shellcheck disable=SC2086
as_root() { if [ -n "$SUDO" ]; then $SUDO "$@"; else "$@"; fi; }

prefix_needs_root() {
    target="$PREFIX"
    while [ ! -e "$target" ] && [ "$target" != "/" ]; do target=$(dirname "$target"); done
    [ ! -w "$target" ]
}

# shellcheck disable=SC2086
run() {
    if prefix_needs_root; then
        [ -n "$SUDO" ] || die "$PREFIX is not writable and neither sudo nor doas is available"
        $SUDO "$@"
    else
        "$@"
    fi
}

# Reads a yes/no answer from the terminal. Works even when this script is
# piped into sh, because the prompt goes to /dev/tty rather than stdin.
confirm() {
    [ "$ASSUME_YES" = 1 ] && return 0
    if [ ! -r /dev/tty ]; then
        # Nothing to ask on: the caller opted in by running the installer.
        return 0
    fi
    printf '\n  %s [Y/n] ' "$1" > /dev/tty
    read -r answer < /dev/tty || answer=""
    case "$answer" in
        ""|y|Y|yes|YES|o|O|oui|OUI) return 0 ;;
        *) return 1 ;;
    esac
}

# ------------------------------------------------------ distro detection ----

DISTRO_ID=""
DISTRO_LIKE=""
if [ -r /etc/os-release ]; then
    # shellcheck disable=SC1091
    . /etc/os-release
    DISTRO_ID="${ID:-}"
    DISTRO_LIKE="${ID_LIKE:-}"
fi

# Prints a family name the package tables below key off.
distro_family() {
    case " $DISTRO_ID $DISTRO_LIKE " in
        *" arch "*|*" archlinux "*|*" cachyos "*|*" manjaro "*|*" endeavouros "*) echo arch ;;
        *" debian "*|*" ubuntu "*)                                                echo debian ;;
        *" fedora "*|*" rhel "*|*" centos "*)                                     echo fedora ;;
        *" suse "*|*" opensuse "*)                                                echo suse ;;
        *" alpine "*)                                                             echo alpine ;;
        *" void "*)                                                               echo void ;;
        *" gentoo "*)                                                             echo gentoo ;;
        *)                                                                        echo unknown ;;
    esac
}

# Packages providing Qt 6 WebEngine, which is the Chromium instaCache renders
# with. Never vendored: this is the distribution's copy, shared with every
# other Qt application on the machine.
engine_packages() {
    case "$(distro_family)" in
        arch)   echo "qt6-webengine qt6-declarative" ;;
        debian) echo "libqt6webenginequick6 qml6-module-qtwebengine qml6-module-qtquick-window" ;;
        fedora) echo "qt6-qtwebengine qt6-qtdeclarative" ;;
        suse)   echo "libQt6WebEngineQuick6 qt6-webengine-imports qt6-declarative-imports" ;;
        alpine) echo "qt6-qtwebengine qt6-qtdeclarative" ;;
        void)   echo "qt6-webengine qt6-declarative" ;;
        gentoo) echo "dev-qt/qtwebengine:6 dev-qt/qtdeclarative:6" ;;
        *)      echo "" ;;
    esac
}

# Chromium carries its own H.264 decoder, so there is no GStreamer plugin set
# to install any more -- with one exception. Fedora builds Qt WebEngine without
# the patent-encumbered codecs; Instagram is H.264, so on Fedora the video is
# blank until the RPM Fusion rebuild is installed alongside it.
codec_packages() {
    case "$(distro_family)" in
        fedora) echo "qt6-qtwebengine-freeworld" ;;
        *)      echo "" ;;
    esac
}

# The command that installs a list of packages, non-interactively.
install_command() {
    case "$(distro_family)" in
        arch)   echo "pacman -S --needed --noconfirm" ;;
        debian) echo "apt-get install -y --no-install-recommends" ;;
        fedora) echo "dnf install -y" ;;
        suse)   echo "zypper --non-interactive install" ;;
        alpine) echo "apk add" ;;
        void)   echo "xbps-install -Sy" ;;
        gentoo) echo "emerge --noreplace" ;;
        *)      echo "" ;;
    esac
}

# ------------------------------------------------------ dependency checks ----

engine_present() {
    if command -v ldconfig >/dev/null 2>&1 &&
       ldconfig -p 2>/dev/null | grep -q 'libQt6WebEngineQuick\.so'; then
        return 0
    fi
    for dir in /usr/lib /usr/lib64 /usr/lib/x86_64-linux-gnu \
               /usr/lib/aarch64-linux-gnu /usr/local/lib; do
        [ -e "$dir/libQt6WebEngineQuick.so.6" ] && return 0
    done
    return 1
}

# Instagram is H.264 throughout. Chromium decodes it itself, so on nearly every
# distribution there is nothing to check -- except where the Qt WebEngine build
# deliberately leaves the codec out, which is Fedora. The symptom there is very
# specific: photos and avatars render, every video stays blank.
missing_codecs() {
    [ "$(distro_family)" = "fedora" ] || return 0

    for dir in /usr/lib64 /usr/lib; do
        [ -e "$dir/qt6/qtwebengine-freeworld" ] && return 0
        [ -e "$dir/libavcodec-freeworld.so" ] && return 0
    done
    printf 'h264-decoder'
}

# Installs $1 (a package list) after explaining what it is for. Never fatal:
# the app is still installed if this fails, only some of it will not work.
install_packages() {
    what="$1"
    packages="$2"

    if [ "$INSTALL_DEPS" = 0 ]; then
        warn "$what is missing; --no-deps was given, so nothing was installed"
        return 1
    fi

    command=$(install_command)
    if [ -z "$packages" ] || [ -z "$command" ]; then
        warn "$what is missing, and this distribution is not one I know how to"
        warn "install packages on. Install $what with your package manager."
        return 1
    fi

    if [ -z "$SUDO" ] && [ "$(id -u)" != 0 ]; then
        warn "$what is missing, and neither sudo nor doas is available."
        printf '     Run this as root:\n       %s%s %s%s\n' \
            "$C_BOLD" "$command" "$packages" "$C_RESET" >&2
        return 1
    fi

    printf '     %s is missing. It will be installed with:\n' "$what"
    printf '       %s%s%s %s%s\n' "$C_BOLD" "${SUDO:+$SUDO }" "$command" "$packages" "$C_RESET"

    if ! confirm "Install it now?"; then
        warn "skipped; $APP_NAME will be installed but $what will still be missing"
        return 1
    fi

    # Package lists must word-split here.
    # shellcheck disable=SC2086
    if as_root $command $packages; then
        ok "$what installed"
        return 0
    fi

    warn "installing $what failed; $APP_NAME itself will still be installed"
    return 1
}

check_dependencies() {
    if engine_present; then
        ok "Qt 6 WebEngine runtime found"
    else
        install_packages "the Qt 6 WebEngine runtime" "$(engine_packages)" || true
        engine_present && ok "Qt 6 WebEngine runtime found" || \
            warn "$APP_NAME will not start until Qt 6 WebEngine is installed"
    fi

    missing=$(missing_codecs)
    if [ -z "$missing" ]; then
        ok "video codecs found"
        return 0
    fi

    printf '     Missing video support:%s%s%s\n' "$C_BOLD" " $missing" "$C_RESET"
    printf '     Without it photos load but every Reel, Story and video stays blank.\n'
    install_packages "H.264 video support" "$(codec_packages)" || true

    still_missing=$(missing_codecs)
    if [ -z "$still_missing" ]; then
        ok "video codecs found"
    else
        warn "still missing: $still_missing — videos will not play"
    fi
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
    printf '     %s%s%s is still in your application menu. To launch it from a\n' "$C_BOLD" "$APP_NAME" "$C_RESET" >&2
    printf '     terminal as well, add this to your shell profile:\n\n' >&2
    printf '       export PATH="%s:$PATH"\n\n' "$BIN_DIR" >&2
}

# ------------------------------------------------------------------ main ----

printf '\n%s%s installer%s\n\n' "$C_BOLD" "$APP_NAME" "$C_RESET"

step "Checking what this needs to run"
check_dependencies

step "Locating the binary"
BINARY=$(resolve_binary)
ok "using $BINARY"

step "Installing into $PREFIX"
run install -Dm755 "$BINARY" "$BIN_DIR/$APP"
ok "binary -> $BIN_DIR/$APP"

run install -Dm644 "$SCRIPT_DIR/$APP.desktop" "$DESKTOP_DIR/$APP.desktop"
ok "menu entry -> $DESKTOP_DIR/$APP.desktop"

# A one-line install leaves no archive behind, so the uninstaller is kept
# alongside the app instead of only in the download.
if [ -f "$SCRIPT_DIR/uninstall.sh" ]; then
    run install -Dm755 "$SCRIPT_DIR/uninstall.sh" "$SUPPORT_DIR/uninstall.sh"
    ok "uninstaller -> $SUPPORT_DIR/uninstall.sh"
fi

# `instacache --update` re-runs this to fetch and install a newer release.
if [ -f "$SCRIPT_DIR/get.sh" ]; then
    run install -Dm755 "$SCRIPT_DIR/get.sh" "$SUPPORT_DIR/update.sh"
    ok "updater -> $SUPPORT_DIR/update.sh"
fi

install_icons
refresh_caches
check_path

printf '\n%sDone.%s %s is installed.\n\n' "$C_GREEN$C_BOLD" "$C_RESET" "$APP_NAME"
printf '  Launch it from your application menu, or run:  %s%s%s\n' "$C_BOLD" "$APP" "$C_RESET"
printf '  Remove it again with:                          %s%s/uninstall.sh%s\n\n' "$C_BOLD" "$SUPPORT_DIR" "$C_RESET"
