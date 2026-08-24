#!/bin/sh
# instaCache uninstaller.
#
#   ./uninstall.sh            remove the application, keep your session and settings
#   ./uninstall.sh --purge    also delete the session, cache and configuration
#   ./uninstall.sh --help     full option list

set -eu

APP=instacache
APP_NAME="instaCache"

PREFIX=""
MODE=user
PURGE=0
ASSUME_YES=0

if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    C_RESET=$(printf '\033[0m'); C_BOLD=$(printf '\033[1m')
    C_RED=$(printf '\033[31m'); C_GREEN=$(printf '\033[32m')
    C_YELLOW=$(printf '\033[33m'); C_BLUE=$(printf '\033[34m')
else
    C_RESET=''; C_BOLD=''; C_RED=''; C_GREEN=''; C_YELLOW=''; C_BLUE=''
fi

step() { printf '%s==>%s %s\n' "$C_BLUE$C_BOLD" "$C_RESET" "$*"; }
ok()   { printf '%s  ok%s %s\n' "$C_GREEN" "$C_RESET" "$*"; }
warn() { printf '%swarn%s %s\n' "$C_YELLOW" "$C_RESET" "$*" >&2; }
die()  { printf '%serror%s %s\n' "$C_RED" "$C_RESET" "$*" >&2; exit 1; }

usage() {
    cat <<EOF
$APP_NAME uninstaller

USAGE:
    ./uninstall.sh [OPTIONS]

OPTIONS:
    --user            Remove from \$HOME/.local (default).
    --system          Remove from /usr/local (needs root).
    --prefix <DIR>    Remove from an arbitrary prefix.
    --purge           Also delete your session, cache and configuration.
                      This signs you out of Instagram.
    -y, --yes         Do not ask for confirmation when purging.
    -h, --help        Show this message.
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --user)      MODE=user ;;
        --system)    MODE=system ;;
        --prefix)    shift; [ $# -gt 0 ] || die "--prefix requires a directory"; PREFIX="$1"; MODE=custom ;;
        --prefix=*)  PREFIX="${1#--prefix=}"; MODE=custom ;;
        --purge)     PURGE=1 ;;
        -y|--yes)    ASSUME_YES=1 ;;
        -h|--help)   usage; exit 0 ;;
        *)           die "unknown option '$1' (try --help)" ;;
    esac
    shift
done

case "$MODE" in
    user)   PREFIX="${PREFIX:-$HOME/.local}" ;;
    system) PREFIX="${PREFIX:-/usr/local}" ;;
esac

DESKTOP_DIR="$PREFIX/share/applications"
ICON_DIR="$PREFIX/share/icons/hicolor"
SUPPORT_DIR="$PREFIX/share/$APP"

SUDO=""
if [ ! -w "$PREFIX" ] && [ "$(id -u)" != 0 ]; then
    if command -v sudo >/dev/null 2>&1; then SUDO="sudo"
    elif command -v doas >/dev/null 2>&1; then SUDO="doas"
    fi
fi
# $SUDO is intentionally unquoted: it is either empty or a single command name.
# shellcheck disable=SC2086
run() { if [ -n "$SUDO" ]; then $SUDO "$@"; else "$@"; fi; }

remove() {
    [ -e "$1" ] || return 0
    run rm -f "$1"
    ok "removed $1"
    REMOVED=$((REMOVED + 1))
}

printf '\n%s%s uninstaller%s\n\n' "$C_BOLD" "$APP_NAME" "$C_RESET"

step "Removing $APP_NAME from $PREFIX"
REMOVED=0

remove "$PREFIX/bin/$APP"
remove "$DESKTOP_DIR/$APP.desktop"
remove "$ICON_DIR/scalable/apps/$APP.svg"
for size in 16 22 24 32 48 64 128 256 512; do
    remove "$ICON_DIR/${size}x${size}/apps/$APP.png"
done
# The copy of this very script that install.sh left behind. Removing it while
# it runs is safe: the shell has already read the file.
remove "$SUPPORT_DIR/uninstall.sh"
[ -d "$SUPPORT_DIR" ] && rmdir "$SUPPORT_DIR" 2>/dev/null || true

if [ "$REMOVED" = 0 ]; then
    warn "nothing was installed under $PREFIX"
    printf '     If you used a different prefix, pass --prefix <DIR> or --system.\n' >&2
fi

if command -v gtk-update-icon-cache >/dev/null 2>&1 && [ -d "$ICON_DIR" ]; then
    run gtk-update-icon-cache -f -t "$ICON_DIR" >/dev/null 2>&1 || true
fi
if command -v update-desktop-database >/dev/null 2>&1 && [ -d "$DESKTOP_DIR" ]; then
    run update-desktop-database "$DESKTOP_DIR" >/dev/null 2>&1 || true
fi

# --------------------------------------------------------- personal data ----

DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/$APP"
CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/$APP"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/$APP"

if [ "$PURGE" = 1 ]; then
    step "Deleting your session, cache and configuration"
    printf '     %s\n     %s\n     %s\n' "$DATA_DIR" "$CACHE_DIR" "$CONFIG_DIR"

    if [ "$ASSUME_YES" != 1 ]; then
        printf '\n  This signs you out of Instagram and cannot be undone.\n'
        printf '  Type %syes%s to continue: ' "$C_BOLD" "$C_RESET"
        read -r answer
        [ "$answer" = "yes" ] || die "aborted; nothing personal was deleted"
    fi

    for dir in "$DATA_DIR" "$CACHE_DIR" "$CONFIG_DIR"; do
        [ -d "$dir" ] || continue
        rm -rf "$dir"
        ok "deleted $dir"
    done
else
    for dir in "$DATA_DIR" "$CACHE_DIR" "$CONFIG_DIR"; do
        if [ -d "$dir" ]; then
            printf '\n%sKept%s your data in %s\n' "$C_BOLD" "$C_RESET" "$dir"
            printf '     Run %s./uninstall.sh --purge%s to delete it too.\n' "$C_BOLD" "$C_RESET"
            break
        fi
    done
fi

printf '\n%sDone.%s\n\n' "$C_GREEN$C_BOLD" "$C_RESET"
