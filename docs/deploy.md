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

## Bind address, and why it is the interesting decision

klams-view has **no authentication of its own**. Anything that can open
the port reads the entire memory store. So `KLAMS_VIEW_ADDR` is the
whole access-control story, and the shipped default —
`0.0.0.0:7778` — is only safe because of what sits in front of it:

- ufw defaults to deny-incoming and there is no rule for 7778, so LAN
  and WAN traffic is dropped.
- Tailscale accepts traffic on its own interface ahead of ufw, so the
  tailnet reaches it.

The result is tailnet-only in practice, but it is *firewall-enforced*
rather than *bind-enforced*, which is the weaker of the two. klams
itself takes the stronger route: it binds localhost and its tailscale
addresses explicitly and never listens on `0.0.0.0`.

To match that, pin the address:

```sh
KLAMS_VIEW_ADDR=100.x.y.z:7778     # this host's tailscale IPv4
```

The trade: klams-view then binds one address (no localhost, no IPv6 —
the config takes a single socket address), and the unit fails to start
if tailscaled has not brought the address up yet. `Restart=on-failure`
with `RestartSec=10` recovers from that, but it is a real ordering
dependency where `0.0.0.0` has none.

**Do not add a ufw rule for 7778.** That is what would turn the default
from "tailnet-only in practice" into "readable from the LAN".

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
curl -s localhost:7778/api/status # {"view":"ok","klams":"ok"}
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
