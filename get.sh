#!/bin/sh
# instaCache one-line installer.
#
#     curl -fsSL https://raw.githubusercontent.com/LightZirconite/instaCache/main/get.sh | sh
#
# Downloads the latest release for this machine's architecture, checks it
# against the checksum published alongside it, and runs the installer inside —
# which also installs the system libraries instaCache needs.
#
# Options are forwarded to that installer:
#
#     curl -fsSL .../get.sh | sh -s -- --system
#     curl -fsSL .../get.sh | sh -s -- --yes
#
# Pin a version with INSTACACHE_VERSION=v1.0.0.

set -eu

REPO="${INSTACACHE_REPO:-LightZirconite/instaCache}"
APP=instacache
APP_NAME="instaCache"
API="https://api.github.com/repos/$REPO"

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

# ------------------------------------------------------------ prequisites ---

if command -v curl >/dev/null 2>&1; then
    DOWNLOAD='curl -fsSL -o'
    FETCH='curl -fsSL'
elif command -v wget >/dev/null 2>&1; then
    DOWNLOAD='wget -qO'
    FETCH='wget -qO-'
else
    die "this needs curl or wget, and neither is installed"
fi

command -v tar >/dev/null 2>&1 || die "this needs tar, and it is not installed"

case "$(uname -s)" in
    Linux) ;;
    *) die "$APP_NAME is a Linux application; this is $(uname -s)" ;;
esac

case "$(uname -m)" in
    x86_64|amd64)  ARCH=x86_64 ;;
    aarch64|arm64) ARCH=aarch64 ;;
    *) die "no prebuilt binary for $(uname -m). Build from source: https://github.com/$REPO" ;;
esac

# ------------------------------------------------------------- the version --

printf '\n%s%s%s\n\n' "$C_BOLD" "$APP_NAME installer" "$C_RESET"
step "Looking up the latest release"

VERSION="${INSTACACHE_VERSION:-}"
if [ -z "$VERSION" ]; then
    # Read the tag out of the API response without requiring jq.
    VERSION=$($FETCH "$API/releases/latest" 2>/dev/null |
        tr ',' '\n' |
        sed -n 's/.*"tag_name" *: *"\([^"]*\)".*/\1/p' |
        head -1)
fi
[ -n "$VERSION" ] || die "could not reach the GitHub release API. Check your connection, or download the archive by hand from https://github.com/$REPO/releases"

NUMBER="${VERSION#v}"
NAME="$APP-$NUMBER-linux-$ARCH"
BASE="https://github.com/$REPO/releases/download/$VERSION"
ok "$VERSION for $ARCH"

# ---------------------------------------------------------------- download --

TMP=$(mktemp -d)
cleanup() { rm -rf "$TMP"; }
trap cleanup EXIT INT TERM

step "Downloading"
# shellcheck disable=SC2086
$DOWNLOAD "$TMP/$NAME.tar.gz" "$BASE/$NAME.tar.gz" ||
    die "could not download $BASE/$NAME.tar.gz"
ok "$NAME.tar.gz"

# The archive is about to be unpacked and a script inside it run, so verify it
# against the checksum published next to it before touching anything.
step "Verifying the download"
# shellcheck disable=SC2086
if $DOWNLOAD "$TMP/$NAME.tar.gz.sha256" "$BASE/$NAME.tar.gz.sha256" 2>/dev/null; then
    expected=$(cut -d' ' -f1 < "$TMP/$NAME.tar.gz.sha256")
    if command -v sha256sum >/dev/null 2>&1; then
        actual=$(sha256sum "$TMP/$NAME.tar.gz" | cut -d' ' -f1)
    elif command -v shasum >/dev/null 2>&1; then
        actual=$(shasum -a 256 "$TMP/$NAME.tar.gz" | cut -d' ' -f1)
    else
        actual=""
    fi

    if [ -z "$actual" ]; then
        warn "no sha256 tool available; the download could not be verified"
    elif [ "$actual" = "$expected" ]; then
        ok "checksum matches"
    else
        die "checksum mismatch — refusing to install.
       expected $expected
       got      $actual"
    fi
else
    warn "no published checksum for this release; the download could not be verified"
fi

# ----------------------------------------------------------------- install --

step "Unpacking"
tar -xzf "$TMP/$NAME.tar.gz" -C "$TMP" || die "the archive could not be unpacked"
[ -x "$TMP/$NAME/install.sh" ] || die "the archive does not contain an installer"
ok "unpacked"

printf '\n'
# Forwards whatever came after `sh -s --`.
"$TMP/$NAME/install.sh" "$@"
