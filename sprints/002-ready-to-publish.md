# Sprint 002 — ready to publish

**Goal**: take klams-view from "runs ad hoc on a spare port" to a real
service with a public repo. Covers korg WIs #796 (scanner defaults),
#803 (publish hygiene), #794 (deploy target), #795 (publish), with
#793 (dedicated read-only token) resolved by Ken before the sprint
opened.

## Decisions

- **Scanners are excluded by default, everywhere they distort a
  chart.** `klams-scanner` (127K) and `kai-scanner` (53K) outweigh
  every interactive agent combined by ~1000:1, so an unfiltered agent
  chart is a scanner chart and nothing else. There is no upstream flag
  for "bulk ingest agent" — the `-scanner` name suffix is the signal,
  agreed in both `src/api.rs::is_scanner` and `web/src/lib/agents.ts`.
- **Two mechanisms for one meaning, deliberately.** Pulse asks the
  server (`/api/activity?include_scanners=false`) — it has no author
  roster in hand when it fires, and one call feeds both panels.
  Activity has a chart *and* a paged table fed by a passthrough
  (`/api/memories`), where exclusion can only be expressed as an
  inclusion list of the other authors; using that list for both calls
  keeps the two halves of the page agreeing by construction. Authors
  filters client-side — it already holds every author object.
- **Coverage is a property of the walk, not the filter.** Scanner rows
  still advance `oldest_seen`, so `covered_since` keeps telling the
  truth about what the 100-page cap actually reached. The filter
  changes what is counted, not what was read.
- **Deploy: systemd unit on the klams host** (Ken's call on #794).
  Co-location keeps `KLAMS_URL` on localhost, so the token never
  crosses the network, and klams already establishes the native-unit
  pattern there. The container-on-the-docker-host alternative is
  written up and rejected in docs/deploy.md rather than left implicit.
- **Bind address is the access-control story.** klams-view has no auth
  of its own. The shipped default is `0.0.0.0:7778`, tailnet-only *in
  practice* because ufw denies incoming and tailscale accepts ahead of
  it — firewall-enforced, not bind-enforced, which is weaker than what
  klams does for itself. docs/deploy.md states this plainly and gives
  the stronger option (pin the tailscale IP, accept the tailscaled
  ordering dependency).
- **Public, and the scrub was nearly a no-op.** Ken's call: host names
  may stay, tailnet names may not. Verified before publishing — git
  history has never contained `.env` or any file outside the current
  tree, and nothing in the tree matches a tailnet name, `.ts.net`, a
  private IP, or a token literal. One README phrase ("reachable from
  any browser on the tailnet") was the only change.

## Shipped

- **#796** — `isScanner()` / `is_scanner()`, an `include_scanners`
  param on `/api/activity`, and a shared `ScannerToggle` on Pulse,
  Activity and Authors (off by default, with the affected headings
  saying "scanners hidden" so a filtered chart never reads as the
  whole truth). Verified live: a 29-day window drops from 20,000 rows
  to 42, peak bucket 15,196 → 22.
- **#803** — `.github/workflows/ci.yml` running `just check`'s steps
  (the repo had no CI at all, while klams and korg both do),
  `.env.example`, and a README rewritten for someone who has never
  seen the homelab.
- **#794** — `deploy/klams-view.service` (hardened: no writes
  anywhere, syscall filter), `deploy/install-systemd.sh` (idempotent,
  `--dry-run`, stages binary *and* bundle so a deploy never serves a
  new binary with an old SPA, and never overwrites an existing env
  file), `just deploy` / `deploy-dry-run` / `deploy-logs`, and
  docs/deploy.md as the runbook.
- **#795** — published public as `kenhia/klams-view`.

## Follow-ups

- klams #802: remove the parked `viewport` app from the klams repo and
  point its README at this one.
- The SPA bundle is copied to `/usr/local/share`; embedding it in the
  binary (rust-embed) would make deploy a single-file move and
  rollback atomic. Not worth it yet.
- No component or E2E tests still (roadmap item). This sprint's UI
  change was verified through the API and by eye, not by a test.
