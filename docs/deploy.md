# Deploying klams-view

klams-view runs as a plain systemd unit on the same host as klams. There
is no container: it is one static-ish Rust binary plus a directory of
built SPA assets, and co-locating it with klams keeps `KLAMS_URL` on
localhost — no token crossing the network, no second machine to keep in
sync when the klams API changes.

**Releases go through the homelab package store** (k-homelab
`docs/deploying.md`, sprint 020): every deploy publishes a versioned
artifact, and every install pulls from the store — even when the deploy
is local, as klams-view's is. Sprint 003 (#1013) converted it; before
that, `just deploy` built from the checkout, which is the pinned-clone
pattern the store exists to retire.

Two paths, and the split is the point:

| Recipe | Owns | When |
|---|---|---|
| `just install-systemd` | the system user, `/etc/klams-view`, the unit | first install on a host; any unit or config change; store unreachable |
| `just publish` + `just deploy` | the binary and the SPA bundle | every routine release |

Set these once per machine, in the gitignored `.env`:

```sh
KLAMS_STORE_URL=https://<store-host>:4880   # read from, on every host
KLAMS_STORE_HOST=<store-host>               # ssh host running kpkg; publish only
```

Neither has a default. A guessed hostname fails later as a confusing
`curl` or `ssh` error instead of naming the variable you forgot — the
same call klams makes.

## First deploy

```sh
just install-systemd-dry-run   # prints every step, touches nothing
just install-systemd           # user + config + unit + a build from this checkout
sudoedit /etc/klams-view/klams-view.env    # check KLAMS_URL — no credential to set
sudo systemctl restart klams-view
```

What lands where:

| Path | What |
|---|---|
| `/usr/local/bin/klams-view` | the binary (previous one kept as `.prev`) |
| `/usr/local/share/klams-view/web` | the SPA bundle (previous one as `web.prev`) |
| `/etc/klams-view/klams-view.env` | config (no credential), `0640 root:klams-view` |
| `/etc/systemd/system/klams-view.service` | the unit |

The env file is written **only if absent**. Re-deploying never
overwrites it, so this host's settings survive upgrades; conversely, a
new setting added to `deploy/klams-view.env.example` has to be added by
hand on hosts that already have one.

## The identity

klams-view has no credential. It authenticates to klams by **declaring
who it is** — the `X-Homelab-Agent: klams-view` header — and klams
allow-lists that name read-scoped (program korg:2440: on a single-user
tailnet a shared token was a name tag, not a lock, so it became one).

Add an identity row to klams' config:

```toml
[[auth.identities]]
agent_name = "klams-view"
scopes = ["read"]
label = "klams-view"
```

Nothing is minted, nothing is copied to this host, and nothing goes in
klams-view's env file. Read scope covers every endpoint klams-view
calls, and keeping klams-view's own name — rather than borrowing an
agent's — keeps the dashboard from showing up as one of the agents it
is reporting on. klams hot-reloads identities on `systemctl reload
klams-service` — no restart needed.

A name klams does not know is refused with 401, which the doctor
reports on its own row; see "Diagnosing a broken connection" below.

## Bind address and tailnet publishing

klams-view has **no authentication of its own**. Anything that can open
the port reads the entire memory store, so the bind address is the
whole access-control story.

Bind loopback, and publish to the tailnet through `tailscale serve`:

```sh
KLAMS_VIEW_ADDR=127.0.0.1:7779
```

```sh
tailscale serve --bg --https=7779 http://localhost:7779
```

That gives one URL — `https://<host>.<tailnet>.ts.net:7779` — which
works from every tailnet machine *including the serving host*, with TLS
terminated in tailscaled. `http://localhost:7779` keeps working as the
co-located fallback if tailscaled is down. It is the same shape klams,
korg and kvllm all use.

**Do not bind `0.0.0.0`, and do not bind the tailscale IP directly.**
Both look like they would work and both are traps:

- `tailscale serve` makes tailscaled hold real listeners on
  `<tailscale-ip>:<port>` (v4 and the ts.net v6). A service that binds
  `0.0.0.0:<port>` collides with that specific-IP bind and dies with
  `EADDRINUSE` — and only on its *next* restart, because serve is
  normally set up while the service is already running, so the breakage
  is planted silently. This is exactly what moved klams-service off
  `0.0.0.0:7777`.
- Binding the tailscale IP yourself takes the address tailscaled wants,
  loses localhost and IPv6 (the config takes one socket address), and
  adds a startup ordering dependency on tailscaled.

**Do not add a ufw rule for this port.** ufw default-denies incoming
and tailscale accepts on its own interface ahead of it, so a loopback
bind plus serve is already reachable exactly where it should be. A ufw
rule would only widen it.

## Why not :7778

`:7778` belongs to klams: its eval bake-offs (the sprint 029/030
throwaway pattern) build a branch binary and run it on 7778 against the
live datastores. A permanent listener there fails the next bake-off, or
gets killed to make room for one. klams-view uses `:7779`.

## Releasing and upgrading

```sh
just smoke-live          # every /api route against the real klams, first
just publish             # -> artifacts/klams-view/<version>/
just deploy              # fetch + verify + install + restart, on this host
```

`just publish` refuses a dirty tree — a published version must name a
commit — and takes the version from `klams-view --version` rather than
`Cargo.toml`, which is the same label `install-from-store.sh` asserts on
the way in. The store refuses to overwrite a published version, so bump
`Cargo.toml` (`0.1.<sprint>`) before republishing.

What a version directory holds:

```
artifacts/klams-view/<version>/klams-view-x86_64-linux
artifacts/klams-view/<version>/klams-view-web.tar.gz     (carries VERSION)
artifacts/klams-view/<version>/install-from-store.sh
artifacts/klams-view/<version>/SHA256SUMS
artifacts/klams-view/latest                              -> <version>
```

klams-view ships **two** assets, which is the one way this differs from
its sibling klams. Both live under one version and one `latest` — they
are never separately useful — and `install-from-store.sh` fetches and
verifies **both before installing either**, so a bad bundle cannot leave
a new binary in place serving someone else's frontend. It also checks
that the binary *reports* the version it was published as, and that the
bundle's `VERSION` stamp agrees: the checksums prove the transfer, these
prove the label.

The bundle's `VERSION` stamp is also how a host answers "what is
installed here" with the store unreachable — k-homelab's rule for a
degraded `--check`.

### On a host with no checkout

```sh
just deploy-remote <host>
```

It fetches `install-from-store.sh` out of the store, verifies it against
the same `SHA256SUMS` as the payload, and runs it — a verified fetch
rather than `curl | bash`. The copy-paste equivalent works identically
from a shell on the target host; see the script's own header.

### Rollback

```sh
just rollback                    # .prev binary AND .prev bundle, together
just deploy --version <older>    # any published version, from the store
```

`just rollback` moves both assets or neither, and refuses if only one
`.prev` exists — a binary paired with another release's frontend is the
failure it exists to prevent. The deeper path is the store's version
history, which is what replaced "rebuild from source and hope".

## Operating it

```sh
systemctl status klams-view
just deploy-logs                  # journalctl -u klams-view -f
klams-view --version              # what is installed
cat /usr/local/share/klams-view/web/VERSION   # ...and which bundle
curl -s localhost:7779/api/status | jq .      # the connection doctor
```

`/api/status` is the **connection doctor** (#808). It walks the chain one
link at a time — `KLAMS_URL` parses → identity declared → DNS → TCP →
TLS → unauthenticated `/healthz` → **authenticated read** → klams version
vs the version klams-view was verified against — and reports each link
separately, with the fix on the row that failed. It always answers 200,
because it describes a chain rather than asserting one; the rollup is
`overall: "ok" | "advisory" | "down"`.

The authenticated step is the point. `/healthz` is unauthenticated, so a
stale or under-scoped token leaves reachability green while every read
fails — the 2026-07-28 incident (klams #739) that cost an afternoon. The
`/health` page renders the doctor above everything else and fetches it
*separately* from the panels, so it keeps working when they cannot.

The UI still degrades rather than erroring out: without a token it
renders the shell plus public health and metrics.

The unit is hardened (`ProtectSystem=strict`, `ProtectHome`, a syscall
filter, no write access anywhere). klams-view writes nothing, so if a
future feature needs disk, that is a deliberate unit change and not an
accident to work around with `ReadWritePaths` in a hurry.

## Not chosen: a container on the docker host

Considered and rejected (WI #794). It would match korg's runbook and
keep this host lean, but it puts the klams API call across the network,
means the token travels off-box, and adds an image build to a project
whose entire deploy is otherwise "copy two things and restart". The
latency and the token exposure both argue for co-location, and klams
already establishes the native-unit pattern on this host.
