#!/usr/bin/env bash
# sprint 003 (#1013) — install klams-view from the homelab package store.
#
#   install-from-store.sh [OPTIONS]
#
# Fetches this release's binary AND SPA bundle from the store, verifies
# both against the published SHA256SUMS, asserts the binary reports the
# version it was labelled with, and installs them — rotating the
# outgoing copies to <name>.prev / web.prev.
#
# SELF-CONTAINED BY DESIGN. This script is published *inside* the
# artifact directory alongside what it installs, covered by the same
# SHA256SUMS, so a host with no klams-view checkout can bootstrap from a
# verified fetch instead of `curl | bash`. It depends on nothing but
# bash, curl, sha256sum, tar and install.
#
# klams-view ships TWO assets, unlike its sibling klams. That changes
# two things:
#
#   * Both are fetched and verified before either is installed, and a
#     failure on the second leaves the first untouched. A half-deploy
#     here means a binary serving a bundle it was not built with.
#   * The bundle is read from disk per request, so a new bundle is
#     served IMMEDIATELY by the still-running old binary. That window is
#     why --restart exists and why the closing message is loud about it.
#
# WHAT IT DOES NOT DO, deliberately:
#   * unit files  — the unit is installed by install-systemd.sh, which
#                   also owns the system user and /etc/klams-view. A
#                   unit change is a `just install-systemd`, not a
#                   payload update.
#   * config      — /etc/klams-view/klams-view.env carries this host's
#                   KLAMS_URL and listen address and must never be
#                   clobbered. (It no longer holds a credential: klams-view
#                   authenticates by declared identity, not a token.)
#   * restart     — only with --restart. Installing and activating are
#                   separate steps so the caller decides; `just deploy`
#                   passes it, because that recipe means "deploy here".

set -euo pipefail

STORE_URL=${KLAMS_STORE_URL:-}
VERSION=""
DRY_RUN=0
RESTART=0
NAME=klams-view
BIN_DST_DIR=${BIN_DST_DIR:-/usr/local/bin}
SHARE_DIR=${SHARE_DIR:-/usr/local/share/klams-view}
UNIT=klams-view.service

usage() {
    cat >&2 <<'EOF'
usage: install-from-store.sh [OPTIONS]

options:
  --store URL       package store base URL (default: $KLAMS_STORE_URL)
  --version VER     version to install (default: the store's `latest`)
  --restart         restart klams-view.service afterwards
  --dry-run         print what would happen; touch nothing
  -h, --help        this message

env:
  BIN_DST_DIR       where the binary lands (default /usr/local/bin)
  SHARE_DIR         where the SPA bundle lands (default
                    /usr/local/share/klams-view)

example (any tailnet host, no checkout required):
  sudo KLAMS_STORE_URL=https://store.example:4880 \
      bash install-from-store.sh --restart
EOF
}

fail() {
    printf 'ERROR: %s\n' "$1" >&2
    exit 1
}

say() {
    if [ "$DRY_RUN" -eq 1 ]; then printf '[dry-run] %s\n' "$*"
    else printf '+ %s\n' "$*"; fi
}

run() {
    say "$*"
    [ "$DRY_RUN" -eq 0 ] && eval "$@"
    return 0
}

while [ $# -gt 0 ]; do
    case "$1" in
        --store)   STORE_URL="${2:-}"; shift 2 ;;
        --version) VERSION="${2:-}"; shift 2 ;;
        --restart) RESTART=1; shift ;;
        --dry-run) DRY_RUN=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *)         usage; fail "unknown argument: $1" ;;
    esac
done

# --- 0. Pre-flight --------------------------------------------------------

# No default store URL, on purpose (the #682 / #776 pattern, and the same
# call klams made): a guessed hostname fails later as a confusing curl
# error instead of saying which variable you forgot.
[ -n "$STORE_URL" ] || fail \
    "no package store URL — pass --store URL or set KLAMS_STORE_URL (e.g. https://<host>:4880)"
STORE_URL=${STORE_URL%/}

for cmd in curl sha256sum tar install; do
    command -v "$cmd" >/dev/null 2>&1 || fail "$cmd not found on this host"
done

# The precondition is writability, not root: both destinations are
# overridable, and a host may install into a user-owned prefix. kaed
# taught this too — writability is a property of the DIRECTORY.
if [ "$DRY_RUN" -eq 0 ]; then
    [ -d "$BIN_DST_DIR" ] || fail "$BIN_DST_DIR does not exist"
    [ -w "$BIN_DST_DIR" ] || fail \
        "$BIN_DST_DIR is not writable by $(id -un) — try: sudo env KLAMS_STORE_URL=\"\$KLAMS_STORE_URL\" bash $0"
    # SHARE_DIR may legitimately not exist yet on a first install; its
    # PARENT has to be writable so we can create it.
    if [ -d "$SHARE_DIR" ]; then
        [ -w "$SHARE_DIR" ] || fail "$SHARE_DIR is not writable by $(id -un)"
    else
        parent=$(dirname "$SHARE_DIR")
        [ -d "$parent" ] && [ -w "$parent" ] || fail \
            "$SHARE_DIR does not exist and $parent is not writable by $(id -un)"
    fi
fi

# The published binary carries its target arch, so a wrong-arch host gets
# a 404 naming what it asked for rather than an ELF that will not exec.
ARCH=$(uname -m)
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
SUFFIX="${ARCH}-${OS}"

# --- 1. Resolve the version ----------------------------------------------

if [ -z "$VERSION" ]; then
    VERSION=$(curl -fsS "$STORE_URL/artifacts/$NAME/latest" 2>/dev/null) \
        || fail "cannot read $STORE_URL/artifacts/$NAME/latest — is $NAME published, and is the store reachable?"
    VERSION=$(printf '%s' "$VERSION" | tr -d '[:space:]')
    [ -n "$VERSION" ] || fail "the latest pointer for $NAME is empty"
    printf 'resolved latest %s = %s\n' "$NAME" "$VERSION"
fi

BASE="$STORE_URL/artifacts/$NAME/$VERSION"
BIN_FILE="$NAME-$SUFFIX"
WEB_FILE="$NAME-web.tar.gz"

printf 'installing %s %s (%s) from %s\n' "$NAME" "$VERSION" "$SUFFIX" "$STORE_URL"

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# --- 2. Fetch + verify BOTH assets before installing EITHER --------------

sums=$(curl -fsS "$BASE/SHA256SUMS") || fail "fetch failed: $BASE/SHA256SUMS"

fetch_verified() {
    local file=$1 line
    printf '==> %s\n' "$file"
    curl -fsS -o "$WORK/$file" "$BASE/$file" \
        || fail "fetch failed: $BASE/$file (is $NAME published at $VERSION?)"
    line=$(printf '%s\n' "$sums" | grep -E "[[:space:]]\*?$(printf '%s' "$file" | sed 's/[.[\*^$]/\\&/g')\$" | head -1)
    [ -n "$line" ] || fail "$file is not listed in $BASE/SHA256SUMS"
    ( cd "$WORK" && printf '%s\n' "$line" | sha256sum -c --status - ) \
        || fail "checksum MISMATCH for $file — refusing to install"
    printf '    checksum OK\n'
}

# A binary's self-reported version, or empty when it does not have one.
#
# `awk '{print $NF}'` alone is not enough, and the 0.1.3 deploy proved it:
# the outgoing pre-0.1.3 build had no --version flag, so it started up,
# logged "KLAMS_TOKEN not set — /api routes will return 503" to stdout,
# and the rotation line recorded its version as "503". The rotation log is
# the record of what you can roll back to, so it must not invent one.
report_version() {
    local field
    field=$("$1" --version 2>/dev/null | head -1 | awk '{print $NF}') || true
    case "$field" in
        [0-9]*.[0-9]*) printf '%s' "$field" ;;
        *)             : ;;
    esac
}

fetch_verified "$BIN_FILE"
fetch_verified "$WEB_FILE"

chmod 0755 "$WORK/$BIN_FILE"

# The checksum proves the transfer; this proves the LABEL. A binary
# published under the wrong version would otherwise install cleanly and
# then lie to --version, which is exactly the signal a version floor
# reads. klams found this defect in its own sprint 042.
reported=$(report_version "$WORK/$BIN_FILE")
[ -n "$reported" ] || fail \
    "$BIN_FILE --version printed no version — wrong arch, or not a klams-view binary"
[ "$reported" = "$VERSION" ] || fail \
    "$BIN_FILE reports version $reported but was published as $VERSION — the store labelling is wrong, not this host"
printf '    reports %s\n' "$reported"

# Unpack the bundle in the staging dir, so a corrupt tar fails before
# anything on the host has moved.
mkdir -p "$WORK/web"
tar -xzf "$WORK/$WEB_FILE" -C "$WORK/web" \
    || fail "$WEB_FILE is not a readable tarball — refusing to install"
[ -f "$WORK/web/index.html" ] || fail "$WEB_FILE contains no index.html at its root"

# The bundle carries a VERSION stamp so a host can answer "what is
# installed here" without the store path that delivered it — k-homelab
# docs/deploying.md's rule for when the store is unreachable, where a
# --check has nothing else to read.
web_version=$(cat "$WORK/web/VERSION" 2>/dev/null | tr -d '[:space:]')
[ -n "$web_version" ] || fail "$WEB_FILE carries no VERSION stamp"
[ "$web_version" = "$VERSION" ] || fail \
    "$WEB_FILE is stamped $web_version but was published as $VERSION — the bundle and the binary are from different builds"
printf '    bundle stamped %s\n' "$web_version"

# --- 3. Install (rotate prev) --------------------------------------------

BIN_DST="$BIN_DST_DIR/$NAME"
if [ -e "$BIN_DST" ]; then
    old=$(report_version "$BIN_DST")
    say "rotating $BIN_DST (${old:-unknown}) -> $BIN_DST.prev"
    [ "$DRY_RUN" -eq 0 ] && mv -f "$BIN_DST" "$BIN_DST.prev"
fi
run "install -m 0755 '$WORK/$BIN_FILE' '$BIN_DST'"

# Staged next to the destination and swapped, so the window where the
# bundle is half-written is not also a window where it is served.
run "install -d -m 0755 '$SHARE_DIR'"
run "rm -rf '$SHARE_DIR/web.new'"
run "cp -a '$WORK/web' '$SHARE_DIR/web.new'"
run "chmod -R a+rX '$SHARE_DIR/web.new'"
run "rm -rf '$SHARE_DIR/web.prev'"
if [ -d "$SHARE_DIR/web" ]; then
    old_web=$(cat "$SHARE_DIR/web/VERSION" 2>/dev/null | tr -d '[:space:]' || true)
    say "rotating $SHARE_DIR/web (${old_web:-unstamped}) -> web.prev"
    [ "$DRY_RUN" -eq 0 ] && mv -f "$SHARE_DIR/web" "$SHARE_DIR/web.prev"
fi
run "mv -f '$SHARE_DIR/web.new' '$SHARE_DIR/web'"

# --- 4. Activate, or say what to run ------------------------------------

printf '\ndone — %s %s installed (binary: %s, bundle: %s/web)\n' \
    "$NAME" "$VERSION" "$BIN_DST" "$SHARE_DIR"

if [ "$RESTART" -eq 1 ]; then
    run "systemctl restart $UNIT"
    printf 'restarted %s\n' "$UNIT"
else
    # Not a nicety: ServeDir reads the bundle per request, so the NEW
    # bundle is already being served by the OLD binary.
    printf '\n!! Nothing was restarted, and the new bundle is ALREADY being served\n'
    printf '!! by the still-running old binary. Close that window:\n'
    printf '!!   sudo systemctl restart %s\n' "$UNIT"
fi
printf 'Rollback: install-from-store.sh --version <older> --restart (or `just rollback`)\n'
