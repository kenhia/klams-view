# Machine-local values (KLAMS_URL, KLAMS_STORE_URL, …) live in a
# gitignored `.env` at the repo root rather than the shell environment.
# Mirrors klams (sprint 035, #776), and it is what lets the store
# variables below be read at parse time — `env_var_or_default` sees the
# process environment, so a recipe sourcing `.env` itself would be too
# late for a `just` variable.
set dotenv-load := true

# List available recipes
default:
    @just --list

# Run CI gates: rust fmt/clippy/test + web check/format/build
check:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test
    cd web && pnpm check
    cd web && pnpm format:check
    cd web && pnpm build

# Build everything for release (SPA bundle + release binary)
build:
    cd web && pnpm build
    cargo build --release

# Run the server against the SPA bundle (sources .env if present)
run:
    cd web && pnpm build
    bash -c 'set -a; [ -f .env ] && . ./.env; set +a; cargo run'

# Sprint 003 (#809) — live-backend smoke: drive every /api route against
# a REAL klams, then again with a deliberately wrong token to prove the
# doctor tells "unreachable" and "unauthorized" apart.
#
# Deliberately NOT part of `just check`: the gate has to stay hermetic
# and tokenless. The CI-runnable half of #809 is
# `tests/api_contract.rs`, which `cargo test` already covers — it
# catches klams-view regressions; this catches upstream skew.
#
# Read-only. It seeds nothing, so pointing it at the live klams is safe.
[doc("Live-backend smoke: every /api route against a real klams")]
smoke-live *ARGS:
    bash -c 'set -a; [ -f .env ] && . ./.env; set +a; exec scripts/smoke-live.sh {{ARGS}}'

# Frontend dev server (proxies /api to the rust server on :7779)
dev-web:
    cd web && pnpm dev

# Sprint 003 (#1013) — the homelab package store klams-view releases go
# to and come from (k-homelab docs/deploying.md).
#
#   KLAMS_STORE_URL   base URL the store is *read* from, e.g.
#                     https://<host>:4880 — used by deploys, every host.
#   KLAMS_STORE_HOST  ssh host that *runs* `kpkg` — `publish` only.
#
# Same names klams uses, deliberately: one store, so one pair of
# variables per machine rather than one pair per repo. Neither has a
# default (the #682 / #776 pattern) — a guessed hostname fails as a
# confusing curl or ssh error instead of "you didn't set the variable".
# Put yours in the gitignored `.env`; the recipes check and say what to
# set.
store      := env_var_or_default('KLAMS_STORE_URL',  '')
store_host := env_var_or_default('KLAMS_STORE_HOST', '')

# Sprint 003 (#1013) — publish this version's release to the package
# store. klams-view ships TWO assets, so both go into one version
# directory under one `latest`: they are never separately useful, and a
# binary paired with someone else's bundle is the failure this prevents.
#
#   artifacts/klams-view/<version>/klams-view-<arch>-<os>
#   artifacts/klams-view/<version>/klams-view-web.tar.gz   (+ VERSION)
#   artifacts/klams-view/<version>/install-from-store.sh
#   artifacts/klams-view/<version>/SHA256SUMS
#   artifacts/klams-view/latest                            -> <version>
#
# install-from-store.sh rides along INSIDE the directory, covered by the
# same SHA256SUMS as the payload — that is what lets a host with no
# checkout bootstrap from a verified fetch instead of `curl | bash`.
#
# The store refuses to overwrite a published version: bump the Cargo
# version (`0.1.<sprint>`) before republishing.
[doc("Publish this version (binary + SPA bundle) to the package store")]
publish:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ -z '{{store_host}}' ]]; then
        echo 'publish: set KLAMS_STORE_HOST to the ssh host running kpkg (see k-homelab docs/deploying.md)' >&2
        exit 1
    fi
    # Clean-tree only: a published version must correspond to a commit,
    # or /api/status reports a version that names no source. Untracked
    # files count — the build may pick them up.
    if [[ -n "$(git status --porcelain)" ]]; then
        echo 'publish: working tree is dirty — commit first (a published version must name a commit)' >&2
        git status --short >&2
        exit 1
    fi
    cd web && pnpm build && cd ..
    cargo build --release
    suffix="$(uname -m)-$(uname -s | tr '[:upper:]' '[:lower:]')"
    # Take the version from the BINARY, not Cargo.toml: that is the label
    # install-from-store.sh asserts on the way in, so this is the same
    # assertion on the way out.
    version=$(./target/release/klams-view --version | awk '{print $NF}')
    if [[ -z "$version" ]]; then
        echo 'publish: klams-view --version printed nothing' >&2
        exit 1
    fi
    stage=$(mktemp -d); trap 'rm -rf "$stage"' EXIT
    # The bundle carries a VERSION stamp, so a host can answer "what is
    # installed here" without the store path that delivered it —
    # k-homelab's rule for when the store is unreachable and a --check
    # has nothing else to read.
    cp -a web/build "$stage/web"
    echo "$version" > "$stage/web/VERSION"
    tar -czf "$stage/klams-view-web.tar.gz" -C "$stage/web" .
    cp "./target/release/klams-view" "$stage/klams-view-$suffix"
    cp deploy/install-from-store.sh "$stage/"
    echo "==> publishing klams-view $version ($suffix) to {{store_host}}"
    remote=$(ssh -n '{{store_host}}' mktemp -d)
    trap 'rm -rf "$stage"; ssh -n "{{store_host}}" rm -rf "$remote"' EXIT
    scp -q "$stage/klams-view-$suffix" "$stage/klams-view-web.tar.gz" \
        "$stage/install-from-store.sh" "{{store_host}}:$remote/"
    ssh -n '{{store_host}}' \
        "kpkg artifact klams-view $version $remote/klams-view-$suffix \
         $remote/klams-view-web.tar.gz $remote/install-from-store.sh"
    echo "==> published klams-view $version"

# Sprint 003 (#1013) — deploy on THIS host FROM the package store,
# checksum-verified, and restart. This is the deploy path: it needs no
# build and no clean tree, because the artifact was already built and
# published. Roll back with `just rollback`, or deeper with
# `just deploy --version <older>`.
#
# It installs the binary and the bundle only. The unit, the system user
# and /etc/klams-view are `just install-systemd`'s job — a unit change is
# not a payload update.
[doc("Deploy on THIS host from the package store, verified, and restart")]
deploy *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ -z '{{store}}' ]]; then
        echo 'deploy: set KLAMS_STORE_URL to the package store base URL (e.g. https://<host>:4880)' >&2
        exit 1
    fi
    args=({{ARGS}})
    # A dry run writes nothing, so it should not cost a sudo prompt; and
    # only a real install has anything to restart.
    sudo=(sudo); restart=(--restart)
    for a in ${args[@]+"${args[@]}"}; do
        if [[ "$a" == --dry-run ]]; then sudo=(); restart=(); fi
    done
    "${sudo[@]}" env KLAMS_STORE_URL='{{store}}' \
        bash deploy/install-from-store.sh ${restart[@]+"${restart[@]}"} ${args[@]+"${args[@]}"}

# Show what `just deploy` would do, without touching the host
deploy-dry-run: (deploy "--dry-run")

# Sprint 003 (#1013) — deploy on ANOTHER host from the package store.
# The host needs no klams-view checkout and no toolchain: it fetches the
# installer out of the store, verifies it against the same SHA256SUMS as
# the payload, and runs it. This is the documented bootstrap in
# docs/deploy.md, run over ssh — the copy-paste form works identically
# from a shell on the target host.
[doc("Deploy on ANOTHER host from the store (no checkout needed there)")]
deploy-remote host:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ -z '{{store}}' ]]; then
        echo 'deploy-remote: set KLAMS_STORE_URL to the package store base URL (e.g. https://<host>:4880)' >&2
        exit 1
    fi
    ssh -n '{{host}}' "set -euo pipefail
        base='{{store}}/artifacts/klams-view'
        v=\$(curl -fsS \"\$base/latest\")
        d=\$(mktemp -d); trap 'rm -rf \"\$d\"' EXIT; cd \"\$d\"
        curl -fsSO \"\$base/\$v/install-from-store.sh\"
        curl -fsS \"\$base/\$v/SHA256SUMS\" | grep install-from-store.sh | sha256sum -c --status -
        sudo KLAMS_STORE_URL='{{store}}' bash install-from-store.sh --version \"\$v\" --restart"

# Sprint 003 (#1013) — atomic rollback: swap the .prev binary AND the
# .prev bundle back into place together, then restart. The two must move
# as a pair, or the host runs one release's binary against another's
# frontend. No-op when there is no .prev.
#
# Deeper than one step back: `just deploy --version <older>`, which the
# store's version history makes possible.
[doc("Swap the .prev binary and bundle back together, then restart")]
rollback:
    #!/usr/bin/env bash
    set -euo pipefail
    bin=/usr/local/bin/klams-view
    share=/usr/local/share/klams-view
    if [[ ! -f "$bin.prev" || ! -d "$share/web.prev" ]]; then
        echo "rollback: need both $bin.prev and $share/web.prev — found:" >&2
        ls -d "$bin.prev" "$share/web.prev" 2>&1 | sed 's/^/  /' >&2
        echo 'rollback: use `just deploy --version <older>` instead' >&2
        exit 1
    fi
    sudo mv -f "$bin" "$bin.broken"
    sudo mv -f "$bin.prev" "$bin"
    sudo rm -rf "$share/web.broken"
    sudo mv -f "$share/web" "$share/web.broken"
    sudo mv -f "$share/web.prev" "$share/web"
    sudo systemctl restart klams-view.service
    echo "rolled back to $("$bin" --version | awk '{print $NF}') — previous is now .broken"

# First install on a host, and the only path that touches the unit, the
# system user and /etc/klams-view. Builds from THIS checkout, so it is
# also the fallback when the store is unreachable. Routine payload
# updates go through `just publish` + `just deploy` instead
# (k-homelab docs/deploying.md).
[doc("First install / unit + user + config changes, from this checkout")]
install-systemd: build
    sudo deploy/install-systemd.sh

# Show what `just install-systemd` would do, without touching the host
install-systemd-dry-run: build
    deploy/install-systemd.sh --dry-run

# Publish the loopback listener to the tailnet over HTTPS (idempotent).
# Run once per host; see docs/deploy.md for why the service itself must
# stay bound to 127.0.0.1.
[doc("Publish the loopback listener to the tailnet over HTTPS")]
deploy-serve port="7779":
    tailscale serve --bg --https={{port}} http://localhost:{{port}}
    tailscale serve status

# Tail the deployed service's log
deploy-logs:
    journalctl -u klams-view.service -f

# Rust server only, API mode (sources .env if present)
dev-api:
    bash -c 'set -a; [ -f .env ] && . ./.env; set +a; cargo run'
