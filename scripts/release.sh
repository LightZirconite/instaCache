#!/usr/bin/env bash
# Cut a instaCache release.
#
# Bumps the version everywhere it appears, updates the changelog, commits,
# tags and pushes. The push of the tag is what triggers
# .github/workflows/release.yml, which builds the binaries and publishes the
# GitHub Release.
#
#   scripts/release.sh patch          1.0.0 -> 1.0.1
#   scripts/release.sh minor          1.0.0 -> 1.1.0
#   scripts/release.sh major          1.0.0 -> 2.0.0
#   scripts/release.sh 1.4.2          set an exact version
#   scripts/release.sh patch --dry-run   show what would happen, change nothing

set -Eeuo pipefail

REPO_ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$REPO_ROOT"

if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    C_RESET=$'\033[0m'; C_BOLD=$'\033[1m'
    C_RED=$'\033[31m'; C_GREEN=$'\033[32m'
    C_YELLOW=$'\033[33m'; C_BLUE=$'\033[34m'
else
    C_RESET=''; C_BOLD=''; C_RED=''; C_GREEN=''; C_YELLOW=''; C_BLUE=''
fi

step() { printf '%s==>%s %s\n' "$C_BLUE$C_BOLD" "$C_RESET" "$*"; }
ok()   { printf '%s  ok%s %s\n' "$C_GREEN" "$C_RESET" "$*"; }
warn() { printf '%swarn%s %s\n' "$C_YELLOW" "$C_RESET" "$*" >&2; }
die()  { printf '%serror%s %s\n' "$C_RED" "$C_RESET" "$*" >&2; exit 1; }

usage() {
    sed -n '2,/^set -/p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//; $d'
}

# ------------------------------------------------------------- arguments ----

BUMP=""
DRY_RUN=0
SKIP_CHECKS=0
REMOTE="origin"

while [ $# -gt 0 ]; do
    case "$1" in
        major|minor|patch)   BUMP="$1" ;;
        [0-9]*.[0-9]*.[0-9]*) BUMP="$1" ;;
        --dry-run|-n)        DRY_RUN=1 ;;
        --skip-checks)       SKIP_CHECKS=1 ;;
        --remote)            shift; [ $# -gt 0 ] || die "--remote requires a name"; REMOTE="$1" ;;
        --remote=*)          REMOTE="${1#--remote=}" ;;
        -h|--help)           usage; exit 0 ;;
        *)                   die "unknown argument '$1' (try --help)" ;;
    esac
    shift
done

[ -n "$BUMP" ] || { usage; exit 2; }

run() {
    if [ "$DRY_RUN" = 1 ]; then
        printf '  %s[dry-run]%s %s\n' "$C_YELLOW" "$C_RESET" "$*"
    else
        "$@"
    fi
}

# ---------------------------------------------------------- sanity checks ----

step "Checking the working tree"

command -v git >/dev/null || die "git is not installed"
git rev-parse --git-dir >/dev/null 2>&1 || die "not inside a git repository"

BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$BRANCH" != "main" ] && [ "$BRANCH" != "master" ]; then
    warn "you are on '$BRANCH', not on main"
    [ "$DRY_RUN" = 1 ] || die "release from main, or re-run with --dry-run to preview"
fi

if [ -n "$(git status --porcelain)" ]; then
    git status --short >&2
    die "the working tree has uncommitted changes; commit or stash them first"
fi

git remote get-url "$REMOTE" >/dev/null 2>&1 || die "no git remote named '$REMOTE'"
ok "clean tree on '$BRANCH', remote '$REMOTE'"

# ----------------------------------------------------------- new version ----

CURRENT=$(grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2)
[ -n "$CURRENT" ] || die "could not read the version from Cargo.toml"

case "$BUMP" in
    major|minor|patch)
        IFS=. read -r major minor patch <<< "$CURRENT"
        case "$BUMP" in
            major) major=$((major + 1)); minor=0; patch=0 ;;
            minor) minor=$((minor + 1)); patch=0 ;;
            patch) patch=$((patch + 1)) ;;
        esac
        VERSION="${major}.${minor}.${patch}"
        ;;
    *)
        VERSION="$BUMP"
        ;;
esac

[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]] \
    || die "'$VERSION' is not a valid semantic version"

TAG="v$VERSION"
git rev-parse -q --verify "refs/tags/$TAG" >/dev/null && die "tag $TAG already exists"
if git ls-remote --exit-code --tags "$REMOTE" "$TAG" >/dev/null 2>&1; then
    die "tag $TAG already exists on '$REMOTE'"
fi

step "Releasing $C_BOLD$CURRENT$C_RESET -> $C_BOLD$VERSION$C_RESET (tag $TAG)"

# ------------------------------------------------------------ validation ----

if [ "$SKIP_CHECKS" = 1 ]; then
    warn "skipping fmt / clippy / tests"
elif [ "$DRY_RUN" = 1 ]; then
    printf '  %s[dry-run]%s would run cargo fmt --check, clippy and the test suite\n' \
        "$C_YELLOW" "$C_RESET"
else
    step "Running the checks"
    cargo fmt --all --check      || die "cargo fmt found unformatted code"
    cargo clippy --all-targets --locked -- -D warnings || die "clippy reported problems"
    cargo test --locked          || die "the test suite failed"
    ok "fmt, clippy and tests pass"
fi

# --------------------------------------------------------- version bumps ----

step "Updating the version"

bump_file() {
    local file="$1" pattern="$2" replacement="$3"
    [ -f "$file" ] || return 0
    if [ "$DRY_RUN" = 1 ]; then
        printf '  %s[dry-run]%s %s\n' "$C_YELLOW" "$C_RESET" "$file"
        return 0
    fi
    # A temporary file keeps the original intact if sed fails halfway.
    local tmp
    tmp=$(mktemp)
    sed -E "s|${pattern}|${replacement}|" "$file" > "$tmp"
    mv "$tmp" "$file"
    ok "$file"
}

# Only the first `version = ` line, which belongs to [package].
if [ "$DRY_RUN" = 1 ]; then
    printf '  %s[dry-run]%s Cargo.toml\n' "$C_YELLOW" "$C_RESET"
else
    tmp=$(mktemp)
    awk -v v="$VERSION" '
        !done && /^version = "/ { sub(/"[^"]*"/, "\"" v "\""); done = 1 }
        { print }
    ' Cargo.toml > "$tmp"
    mv "$tmp" Cargo.toml
    ok "Cargo.toml"
fi

# instacache.desktop carries the Desktop Entry Specification version, not the
# application version, so it is deliberately left alone.

# Keeps Cargo.lock's own record of the package version in step.
if [ "$DRY_RUN" = 1 ]; then
    printf '  %s[dry-run]%s Cargo.lock (via cargo update -p instacache)\n' "$C_YELLOW" "$C_RESET"
else
    cargo update --offline -p instacache >/dev/null 2>&1 \
        || cargo update -p instacache >/dev/null 2>&1 \
        || warn "could not refresh Cargo.lock; it will update on the next build"
    ok "Cargo.lock"
fi

# ------------------------------------------------------------- changelog ----

step "Updating CHANGELOG.md"

TODAY=$(date +%Y-%m-%d)
PREVIOUS_TAG=$(git describe --tags --abbrev=0 2>/dev/null || true)

if [ -n "$PREVIOUS_TAG" ]; then
    COMMITS=$(git log --no-merges --pretty='- %s' "${PREVIOUS_TAG}..HEAD")
else
    COMMITS=$(git log --no-merges --pretty='- %s')
fi
[ -n "$COMMITS" ] || COMMITS="- No changes recorded since ${PREVIOUS_TAG:-the first commit}."

# A section written by hand ahead of the release is better than one generated
# from commit subjects, so leave it alone rather than adding a second one.
if [ -f CHANGELOG.md ] && grep -q "^## \[\?${VERSION}\]\?" CHANGELOG.md; then
    ok "CHANGELOG.md already documents $VERSION; leaving it as written"
elif [ "$DRY_RUN" = 1 ]; then
    printf '  %s[dry-run]%s would prepend:\n\n## [%s] - %s\n\n%s\n\n' \
        "$C_YELLOW" "$C_RESET" "$VERSION" "$TODAY" "$COMMITS"
else
    if [ ! -f CHANGELOG.md ]; then
        cat > CHANGELOG.md <<'HEADER'
# Changelog

All notable changes to instaCache are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to [Semantic Versioning](https://semver.org/).

HEADER
    fi

    tmp=$(mktemp)
    {
        head -n 6 CHANGELOG.md
        printf '## [%s] - %s\n\n%s\n\n' "$VERSION" "$TODAY" "$COMMITS"
        tail -n +7 CHANGELOG.md
    } > "$tmp"
    mv "$tmp" CHANGELOG.md
    ok "CHANGELOG.md"
fi

# ------------------------------------------------------- commit, tag, push --

step "Committing and tagging"

run git add -A
run git commit -m "release: $VERSION"
run git tag -a "$TAG" -m "instaCache $VERSION"

step "Pushing to '$REMOTE'"
run git push "$REMOTE" "$BRANCH"
run git push "$REMOTE" "$TAG"

if [ "$DRY_RUN" = 1 ]; then
    printf '\n%sDry run finished.%s Nothing was changed.\n\n' "$C_YELLOW$C_BOLD" "$C_RESET"
    exit 0
fi

REMOTE_URL=$(git remote get-url "$REMOTE")
printf '\n%sReleased %s.%s\n\n' "$C_GREEN$C_BOLD" "$TAG" "$C_RESET"
printf '  The release workflow is now building the binaries.\n'
case "$REMOTE_URL" in
    *github.com*)
        slug=$(printf '%s' "$REMOTE_URL" | sed -E 's#(git@|https://)github.com[:/]##; s#\.git$##')
        printf '  Watch it here:  https://github.com/%s/actions\n' "$slug"
        printf '  Release page:   https://github.com/%s/releases/tag/%s\n\n' "$slug" "$TAG"
        ;;
    *)
        printf '  Check your forge'\''s CI page for progress.\n\n'
        ;;
esac
