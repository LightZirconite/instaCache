# Publishing GramCache

Three separate channels, with different amounts of manual work.

## 1. A GitHub / Gitea release — automated

```sh
scripts/release.sh patch      # or minor / major / an exact version
```

The script runs the checks, bumps `Cargo.toml`, `Cargo.lock` and
`packaging/PKGBUILD`, writes the changelog, commits, tags and pushes. Pushing
the tag triggers `.github/workflows/release.yml`, which builds the x86_64 and
aarch64 archives and publishes the release with notes and checksums.

**Gitea needs a registered Actions runner.** Actions being enabled on the
repository is not enough: without a runner, pushing a tag creates no workflow
run at all and no release appears. Check with:

```sh
curl -s https://<host>/api/v1/repos/<owner>/<repo>/actions/tasks \
  | python3 -c 'import json,sys; print(json.load(sys.stdin)["total_count"])'
```

A `0` immediately after pushing a tag means no runner picked the job up. Until
one is registered, build the archive locally — it is byte-for-byte what the
workflow produces:

```sh
VERSION=1.0.0 ARCH=$(uname -m) NAME="gramcache-${VERSION}-linux-${ARCH}"
cargo build --release
mkdir -p "dist/${NAME}/assets"
cp target/release/gramcache gramcache.desktop install.sh uninstall.sh \
   README.md LICENSE CHANGELOG.md "dist/${NAME}/"
cp assets/gramcache.svg "dist/${NAME}/assets/"
tar -C dist -czf "dist/${NAME}.tar.gz" "${NAME}"
( cd dist && sha256sum "${NAME}.tar.gz" > "${NAME}.tar.gz.sha256" )
```

…then attach the two files to the release by hand.

## 2. A local pacman package — no account needed

```sh
cd packaging
makepkg -si
```

Produces and installs a real `gramcache-<version>-<rel>-<arch>.pkg.tar.zst`, so
`pacman -Qi gramcache` lists it and `pacman -R gramcache` removes it cleanly.
Preferable to `install.sh` on Arch-based systems.

## 3. The AUR — requires the maintainer's own account

Publishing to the AUR cannot be automated from this repository: it pushes over
SSH with a personal key tied to an aur.archlinux.org account.

**One-time setup.** Create the account, then paste your public SSH key into
*My Account → SSH Public Key*.

**Every release:**

```sh
cd packaging
updpkgsums                         # refresh sha256sums for the new tag
makepkg --printsrcinfo > .SRCINFO  # mandatory: this is what the AUR indexes
makepkg -f                         # confirm it still builds

git clone ssh://aur@aur.archlinux.org/gramcache.git ../aur-gramcache
cp PKGBUILD .SRCINFO ../aur-gramcache/
cd ../aur-gramcache
git commit -am "Update to <version>"
git push
```

### Things that will bite you

- **`.SRCINFO` must be regenerated and committed** with every `PKGBUILD`
  change. The AUR web search reads it, not the `PKGBUILD`.
- **The source URL must stay reachable forever.** The `PKGBUILD` fetches the
  tag archive from the self-hosted Gitea instance; if that host goes away, the
  AUR package breaks for everyone. A GitHub mirror is the usual insurance.
- **Gitea lowercases the directory inside its tag archives.** The repository is
  `instaCache` but the archive unpacks into `instacache/`, which is why the
  `PKGBUILD` uses an explicit `_srcdir` instead of `$pkgname-$pkgver`.
- **Checksums are pinned.** `sha256sums=('SKIP')` is tolerated but discouraged;
  `updpkgsums` fills in the real value.
- **Package naming.** `gramcache` builds from source. A package shipping the
  prebuilt binary would have to be named `gramcache-bin`, and one tracking the
  default branch `gramcache-git`. They are separate AUR packages.

## Repository metadata

Description:

> Native, ultra-light Instagram desktop client for Linux — a 517 KB GTK/WebKit
> binary with a persistent cache and session.

Topics:

```
instagram instagram-client linux linux-desktop desktop-app
rust gtk gtk3 webkit webkitgtk webview
lightweight native electron-alternative no-electron
cache offline-first wayland x11 freedesktop
```

GitHub allows at most 20 topics, lowercase with hyphens. Gitea topics are not
indexed by search engines the way GitHub's are, so a GitHub mirror is what
actually drives discovery.
