# Deploying klams-view

klams-view runs as a plain systemd unit on the same host as klams. There
is no container: it is one static-ish Rust binary plus a directory of
built SPA assets, and co-locating it with klams keeps `KLAMS_URL` on
localhost — no token crossing the network, no second machine to keep in
sync when the klams API changes.

Everything below is done by `deploy/install-systemd.sh`, which
`just deploy` runs on the host you invoke it from. Build where you
deploy; there is no cross-compilation or artefact registry in the loop.

## First deploy

```sh
just deploy-dry-run      # prints every step, touches nothing
just deploy              # cargo build --release + pnpm build, then install
sudoedit /etc/klams-view/klams-view.env    # set KLAMS_TOKEN
sudo systemctl restart klams-view
```

What lands where:

| Path | What |
|---|---|
| `/usr/local/bin/klams-view` | the binary (previous one kept as `.prev`) |
| `/usr/local/share/klams-view/web` | the SPA bundle (previous one as `web.prev`) |
| `/etc/klams-view/klams-view.env` | config + token, `0640 root:klams-view` |
| `/etc/systemd/system/klams-view.service` | the unit |

The env file is written **only if absent**. Re-deploying never
overwrites it, so the token survives upgrades; conversely, a new setting
added to `deploy/klams-view.env.example` has to be added by hand on
hosts that already have one.

## The token

Mint klams-view its own `[[auth.tokens]]` grant in klams' config with
`scopes = ["read"]` rather than reusing an agent's token. Read scope
covers every endpoint klams-view calls, and giving it a distinct
identity keeps the dashboard from showing up as one of the agents it is
reporting on. klams hot-reloads tokens on `systemctl reload
klams-service` — no restart needed.

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

## Upgrades

```sh
git pull && just deploy
```

Binary and bundle are both staged and moved into place, and the unit is
restarted at the end, so a deploy never leaves a new binary serving an
old bundle. Rollback is manual and deliberate:

```sh
sudo mv /usr/local/bin/klams-view.prev /usr/local/bin/klams-view
sudo rm -rf /usr/local/share/klams-view/web
sudo mv /usr/local/share/klams-view/web.prev /usr/local/share/klams-view/web
sudo systemctl restart klams-view
```

## Operating it

```sh
systemctl status klams-view
just deploy-logs                 # journalctl -u klams-view -f
curl -s localhost:7779/api/status # {"view":"ok","klams":"ok"}
```

`/api/status` reports the viewer and its view of klams separately —
`"klams":"unconfigured"` means the token is missing or empty,
`"unreachable"` means klams is down or `KLAMS_URL` is wrong. The UI
degrades rather than erroring out: without a token it still renders the
shell plus public health and metrics.

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
