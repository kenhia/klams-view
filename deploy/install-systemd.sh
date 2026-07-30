#!/bin/sh
# Install klams-view as a systemd unit on the host this runs on.
# Idempotent; supports --dry-run. Invoked by `just deploy`.
#
# Unlike klams, klams-view ships two artefacts: the binary and the
# built SPA bundle. Both are staged and moved into place, so a deploy
# never leaves a new binary serving an old bundle for long.
#
# Steps:
#   1. Ensure the system user `klams-view` exists.
#   2. Ensure /etc/klams-view exists; install the env file ONLY if
#      absent (it holds the klams token — never clobber it).
#   3. Stage + rotate the binary into /usr/local/bin.
#   4. Replace /usr/local/share/klams-view/web with the new bundle.
#   5. Install the unit, daemon-reload, enable --now, restart.

set -eu

DRY_RUN=0
case "${1:-}" in
    --dry-run) DRY_RUN=1 ;;
    "") ;;
    *) echo "usage: $0 [--dry-run]" >&2; exit 2 ;;
esac

SCRIPT_DIR=$(cd -- "$(dirname -- "$0")" && pwd -P)
REPO_DIR=$(cd -- "$SCRIPT_DIR/.." && pwd -P)
BIN_SRC=${BIN_SRC:-"$REPO_DIR/target/release/klams-view"}
WEB_SRC=${WEB_SRC:-"$REPO_DIR/web/build"}
BIN_DST=/usr/local/bin/klams-view
SHARE_DIR=/usr/local/share/klams-view
CONFIG_DIR=/etc/klams-view
ENV_FILE="$CONFIG_DIR/klams-view.env"
SYSTEMD_DIR=/etc/systemd/system
UNIT=klams-view.service
USER_NAME=klams-view
GROUP_NAME=klams-view

say() {
    if [ "$DRY_RUN" -eq 1 ]; then
        printf '[dry-run] %s\n' "$*"
    else
        printf '+ %s\n' "$*"
    fi
}

run() {
    say "$*"
    if [ "$DRY_RUN" -eq 0 ]; then
        eval "$@"
    fi
}

fail() {
    printf 'ERROR: %s\n' "$1" >&2
    exit 1
}

# --- 0. Pre-flight --------------------------------------------------

[ -x "$BIN_SRC" ] || fail "missing binary $BIN_SRC (run 'cargo build --release' first)"
[ -f "$WEB_SRC/index.html" ] || fail "missing SPA bundle $WEB_SRC (run 'pnpm build' in web/ first)"
[ -f "$SCRIPT_DIR/$UNIT" ] || fail "missing unit file $SCRIPT_DIR/$UNIT"

# --- 1. User --------------------------------------------------------

if getent passwd "$USER_NAME" >/dev/null 2>&1; then
    say "user $USER_NAME exists"
else
    run "useradd --system --no-create-home --shell /usr/sbin/nologin $USER_NAME"
fi

# --- 2. Config ------------------------------------------------------

run "install -d -o root -g $GROUP_NAME -m 0750 $CONFIG_DIR"

if [ -f "$ENV_FILE" ]; then
    say "$ENV_FILE exists — left untouched"
else
    run "install -o root -g $GROUP_NAME -m 0640 $SCRIPT_DIR/klams-view.env.example $ENV_FILE"
    printf '\n!! %s was created from the template.\n' "$ENV_FILE"
    printf '!! Set KLAMS_TOKEN in it, then: systemctl restart %s\n\n' "$UNIT"
fi

# --- 3. Binary (rotate prev) ----------------------------------------

STAGE_DIR="/tmp/klams-view-stage-$$"
run "mkdir -p $STAGE_DIR"
run "install -m 0755 $BIN_SRC $STAGE_DIR/klams-view"
if [ -f "$BIN_DST" ]; then
    run "mv -f $BIN_DST $BIN_DST.prev"
fi
run "mv -f $STAGE_DIR/klams-view $BIN_DST"

# --- 4. SPA bundle --------------------------------------------------
# Staged next to the destination and swapped, so the window where the
# bundle is half-written is not also a window where it is served.

run "install -d -m 0755 $SHARE_DIR"
run "rm -rf $SHARE_DIR/web.new"
run "cp -a $WEB_SRC $SHARE_DIR/web.new"
run "chmod -R a+rX $SHARE_DIR/web.new"
run "rm -rf $SHARE_DIR/web.prev"
if [ -d "$SHARE_DIR/web" ]; then
    run "mv -f $SHARE_DIR/web $SHARE_DIR/web.prev"
fi
run "mv -f $SHARE_DIR/web.new $SHARE_DIR/web"
run "rm -rf $STAGE_DIR"

# --- 5. Unit --------------------------------------------------------

run "install -m 0644 $SCRIPT_DIR/$UNIT $SYSTEMD_DIR/$UNIT"
run "systemctl daemon-reload"
run "systemctl enable $UNIT"
run "systemctl restart $UNIT"

printf 'done. `systemctl status %s` / `journalctl -u %s -f`\n' "$UNIT" "$UNIT"
