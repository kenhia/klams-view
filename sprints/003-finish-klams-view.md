# Sprint 003 — finish klams-view

**Proposal:** korg:820 (covers #809, #808, #807, #1013)
**Started:** 2026-08-19

## Goal

Close klams-view's entire open backlog. Three items are residuals
carried over from klams #740 when klams-view became the rewrite that
won; the fourth puts the deploy path on the homelab package store.

Order is the proposal's, and it is load-bearing: #809 → #808 → #807 →
#1013. The deploy goes last so the artifact that lands in the store is
the one carrying the other three, and #809 is the check that says it is
worth publishing.

## Decisions

### #809 — two test layers, and neither substitutes for the other

The WI asked for a smoke that "points klams-view's axum server at a
live klams, seeds a few rows, and drives the /api/* aggregation layer
end-to-end … ideally CI-runnable". Those two wants pull apart: a live
klams is what catches upstream skew, and CI cannot have one. So both
got built.

- **`tests/api_contract.rs` (17 tests, in `cargo test`, so in CI).** A
  stub klams on an ephemeral loopback port; the *real* `/api` router
  talks to it over real HTTP through the real `reqwest` client. Covers
  every route, the aggregations (author totals, hourly/daily bucketing,
  empty-bucket filling, scanner exclusion, the author filter), and every
  error envelope — 401 relayed as 401, 404 as 404, unconfigured as 503,
  unreachable as 502, and `overview` degrading to nulls instead of
  failing.
- **`scripts/smoke-live.sh` (`just smoke-live`, 24 checks).** The same
  routes against a real klams, plus the deep-link fallback and a
  *second* klams-view instance started with a deliberately wrong token.

**Seeding was dropped, deliberately.** klams-view has no write path, and
the smoke's whole value is that you can point it at the live store — so
it seeds nothing and asserts nothing about specific rows. Controlled
data is the hermetic layer's job, where it is free.

**Fixtures are synthetic, not captured.** The shapes were derived from
`klams-types` and verified field-by-field against live klams 0.1.45, but
this repo is public (sprint 002), so no real memory text, path or host
lands in it.

**`smoke-live` is not part of `just check`.** The gate stays hermetic and
tokenless. Requiring a live klams and a bearer token to run `just check`
would make the gate lie on any machine without them.

**Version skew is an advisory in the smoke, not a failure.** A green run
against a newer klams is exactly the evidence needed to bump
`KLAMS_VERIFIED_VERSION`, so it must not be the thing that stops you.

### #808 — the doctor is a chain, and the authenticated step is the point

`/api/status` used to answer `{view, klams: "ok"|"unreachable"|
"unconfigured"}` — three words for a failure space that needs eight. It
now walks `config → token → dns → tcp → tls → healthz → authed →
version`, reporting each link separately with the fix on the row that
failed. Nothing consumed the old shape (the SPA never called it), so
reshaping cost nothing.

Three choices worth recording:

- **`skipped` is a real state.** A step downstream of a broken link says
  "not attempted — TCP did not connect" rather than reporting a failure
  it never observed. One broken link explains the rest.
- **`token` and `authed` are separate rows.** "You never set it" and
  "klams rejected it" have completely different fixes, and the klams
  #739 incident was the second one wearing the first one's face.
- **Rollup is `ok | advisory | down`, not the health-badge trio.**
  `advisory` is k-homelab `bin/audit`'s word for "checked, but weaker
  than a full verification", and version skew belongs there: klams
  releases one patch per sprint, so a `degraded` badge between klams
  releases would cry wolf permanently — the inverse of the failure this
  WI exists to fix.
- **TLS is classified out of the `reqwest` error chain**, because a
  wrong certificate and a stopped service are not the same call to make.
  reqwest offers no predicate for it, so the source chain is the signal.

The panel on `/health` is fetched **separately from every other call on
the page**, and that separation is the feature: when klams is
unreachable or the token is stale, the `Promise.all` feeding the panels
rejects, and the one thing that has to keep working is the thing that
says why. It collapses to one line when all eight links pass and
self-expands when they do not.

### #807 — the author residuals, plus a defect the work uncovered

**Per-author activity** needed no new endpoint: `/api/activity` already
bucketed by window, and klams' `authors` filter is a CSV of author
**UUIDs** (not agent names), so the whole server-side change was
forwarding a parameter that already existed. `/authors/[id]` gains a
"Writes over time" section reusing `StackedColumns` (kind identity
preserved) behind the existing `TimeRange` presets, defaulting to 7d.
Scanners are included here **against the app-wide default**: the chart is
scoped to one author, so there is no 1000:1 asymmetry left to hide, and
an author page for a scanner has to chart something. Measured worst case
— `kai-scanner`, 7d, 61K-row corpus — is 1.4s; the 100-page cap's
`truncated`/`covered_since` are surfaced when it bites.

**Memory → author jump.** `MemoryRow` was a single `<button>`, and an
`<a>` cannot live inside one — so the whole-row click target became a
sibling layer *underneath* the content rather than a wrapper around it.
That keeps the author name a real link (middle-click, copy-link,
keyboard) and the row still opens the drawer anywhere else. `author.id`
is `Option<Uuid>` upstream and Explore synthesises rows with no author
at all, so the plain-text fallback is a real case. In `MemoryDetail`
only the fact/event branch links: the knowledge branch renders a
`KnowledgeItem`, which carries no author, and its supersede links can
navigate to a *different* memory — `m.author` would stop describing
what is on screen.

#### The defect #809's territory turned up

`/v1/authors/{id}/memories?kinds=knowledge,fact,event` does **not**
interleave by `created_at`. It serves the postgres-backed kinds
newest-first and *then* the knowledge rows, ascending, across cursor
pages. For an author with 178 knowledge and 6 events, the first page is
six events from weeks ago — directly under a new chart saying they wrote
37 things this week. Not data loss (page 2 has the knowledge), but the
page read as broken.

`/v1/memories?authors=<uuid>` sorts correctly and is what the chart uses,
but it caps the window at 30 days, so it is not a drop-in for an all-time
history. Rather than re-architect the list mid-sprint, the page now
**names the ordering** when it is actually biting (more pages exist, the
author has knowledge, and none is on screen) and points at the
one-checkbox workaround. Recorded in docs/design.md's contract-gotcha
list; merging it properly in the `/api` layer — which is where this repo
absorbs klams' quirks — is filed as a follow-up.

### #1013 — publish then deploy, and the two-asset problem

Copied from klams' sprint 042 (which was written expecting this sprint to
copy it), with one structural difference that shapes everything:
**klams-view ships two assets.** klams publishes three independently
useful binaries under three artifact names; klams-view's binary and SPA
bundle are never separately useful, so they live under **one** artifact
name, one version directory, one `latest`.

That difference has consequences the binary-only shape does not carry:

- **Both are fetched and verified before either is installed.** A bad
  bundle must not leave a new binary in place serving someone else's
  frontend. `a_tampered_bundle_refuses_the_binary_too` is the test.
- **The bundle carries a `VERSION` stamp**, checked against the label. A
  binary and a bundle from different builds published into one version
  directory pass every checksum and disagree only here. It is also how a
  host answers "what is installed" with the store unreachable —
  k-homelab's rule for a degraded `--check`.
- **`just rollback` moves both or neither**, and refuses when only one
  `.prev` exists.
- **Not restarting is louder than it is for klams.** `ServeDir` reads the
  bundle per request, so the new bundle is served *immediately* by the
  still-running old binary. The installer says so in as many words, and
  `just deploy` passes `--restart` because that recipe means "deploy
  here".

**`klams-view --version` did not exist**, and the installer's
label-assertion needs it — the same defect klams found in its own 042. On
the currently deployed build, `klams-view --version` tries to *start the
server* and dies with `Error: binding 127.0.0.1:7779`. Added as an
early-out before tracing and config, matching clap's `<name> <version>`
and `-V` so `awk '{print $NF}'` readers keep working.

**Recipe names moved, deliberately.** `just deploy` now means "install
from the store on this host" — the path anyone should reach for — and the
build-from-checkout path is `just install-systemd`, which is also the
only thing that touches the unit, the system user and
`/etc/klams-view`. A unit change is not a payload update, and the store
installer refuses to pretend otherwise.

**Store variables reuse klams' names** (`KLAMS_STORE_URL`,
`KLAMS_STORE_HOST`), because there is one store: one pair of variables
per machine beats one pair per repo. Neither has a default (#682/#776).

**Version is `0.1.<sprint>`** — 0.1.3 — adopting klams' convention so a
deployed version names the sprint that shipped it. The store had no
`klams-view` artifact at all before this, so 0.1.3 is free.

Tested the way klams tested its own installer, and for the same reason it
is cheap: **curl speaks `file://`**, so the whole package store is a temp
directory and 14 tests cover the happy path, `.prev` rotation for both
assets, version pinning (the rollback path), dry-run, and every refusal —
tampered binary, tampered bundle, mislabelled binary, mismatched bundle
stamp, unpublished version, unset store URL, unwritable destination,
unknown argument — plus that the installer *published* into the artifact
directory is byte-identical to the one under test, which is what makes
the repo-less bootstrap a verified fetch rather than `curl | bash`.

## Shipped

- `src/doctor.rs` — the chain, with 8 unit tests on the skew rules.
- `/api/status` reshaped; `Doctor.svelte` renders it on `/health`.
- `src/lib.rs` — a lib target, so `tests/` can drive the real router
  (a contract test that reaches through `main.rs` cannot exist).
- `tests/api_contract.rs` — 17 contract tests, hermetic.
- `scripts/smoke-live.sh` + `just smoke-live` — 24 live checks.
- `Writes over time` on `/authors/[id]`; author links in `MemoryRow` and
  `MemoryDetail`.
- `klams-view --version` / `-V`, before anything can fail; version 0.1.3.
- `deploy/install-from-store.sh` + `tests/install_from_store.rs` (14).
- justfile: `publish`, `deploy` (from the store), `deploy-remote`,
  `rollback`, `install-systemd`; `[doc(...)]` attributes so
  `just --list` is readable instead of showing each comment's last line.
- README gains Testing and "Diagnosing a bad connection"; docs/design.md
  gains the new `/api/status` row, a testing section, and the
  author-memories ordering gotcha; docs/deploy.md rewritten around
  publish-then-deploy; `.env.example`, CLAUDE.md and the copilot mirror
  updated.

### Verified

- `just check` — green (12 unit + 17 contract tests).
- `just smoke-live` against live klams 0.1.45 on kubs0 — **24 passed, 0
  failed, 0 advisory**.
- The gate is not vacuous: pointed at a dead port it reports 18 failures
  and exits 1.
- The three failure classes, each with its own fix line, confirmed by
  hand against the live service: wrong token (`authed` fail, 401,
  reachability green), unreachable port (`tcp` fail, everything
  downstream `skipped`), no token (`token` fail, `authed` skipped).
- Screenshotted headless against the live klams: the doctor panel
  collapsed to one line on `/health`, and `/authors/[id]` charting 37
  writes across 7 daily buckets. 20 of 20 rows in Pulse's recent feed
  carry both an author link and their own open-detail button.
- **A full publish→install rehearsal with the real artifacts**, against a
  `file://` store: the release binary reports 0.1.3, the bundle tars and
  stamps, checksums verify, both land, and the installed pair serves the
  shell (200), a deep link (200) and a healthy doctor reporting
  `view.version = 0.1.3`.
- `just publish` / `deploy` / `deploy-remote` each name the store
  variable they need when it is unset, rather than failing as curl or
  ssh noise.

## Follow-ups

- **#1448** — merge klams' kind segments into one `created_at` timeline
  for the author memory list. Filed from this sprint; the page currently
  names the ordering rather than fixing it.
- **The version-parse fix is in the repo but not in 0.1.3.** The real
  deploy printed `rotating /usr/local/bin/klams-view (503)` — the
  outgoing pre-0.1.3 binary has no `--version`, so it started up, logged
  *"KLAMS_TOKEN not set — /api routes will return 503"* to stdout, and
  `awk '{print $NF}'` read `503` as its version. A rotation line is the
  record of what you can roll back to, so it must not invent one;
  `report_version()` now requires a version-shaped answer, with two
  tests. Not republished as 0.1.4: it is a log-line accuracy fix for a
  condition that cannot recur on this host (every future outgoing binary
  reports properly), and churning an immutable version for it buys
  nothing. It ships with the next release.
- **`0.1.<sprint>` names the release a sprint *ships*.** If a sprint ever
  needs a second release it takes the next patch and the number stops
  being an exact index — the store's history is the truth, not the
  arithmetic.

## Deployed 2026-08-19

- **`0.1.3` live on kubs0**, deployed with **this sprint's own path** —
  `just publish` → `just deploy` — not `install-systemd`. Dogfooded on
  its own ship, as klams did in its sprint 042.
- Published from a clean tree at `0819489`:
  `artifacts/klams-view/0.1.3/` holding
  `klams-view-x86_64-linux`, `klams-view-web.tar.gz`,
  `install-from-store.sh` and `SHA256SUMS`; `latest` → `0.1.3`.
  klams-view had **no artifact in the store at all** before this.
- The published `install-from-store.sh` is byte-identical to the repo
  copy the tests drive (`d053154e…` both sides) — which is what makes
  the repo-less bootstrap a verified fetch rather than `curl | bash`.
- Config changes required: **none**. `/etc/klams-view/klams-view.env`
  untouched, as the installer promises; the unit was not reinstalled.
- Rollback targets in place: `/usr/local/bin/klams-view.prev` and
  `/usr/local/share/klams-view/web.prev` (the pre-sprint pair, which is
  why `web.prev` has no `VERSION` stamp — nothing before 0.1.3 wrote
  one). Deeper: nothing older is in the store yet, so the next release
  is the first one with a store-backed previous version.

### Verified live

Over the tailnet URL, not just localhost:

- `klams-view --version` → `klams-view 0.1.3`;
  `/usr/local/share/klams-view/web/VERSION` → `0.1.3`. The binary and
  the bundle agree, which the installer refuses to let them not do.
- `https://kubs0.…:7779/` → 200; a deep link
  (`/authors/<uuid>`) → 200, so the `ServeDir::fallback(ServeFile)`
  contract survived the bundle swap.
- `/api/status` → `overall: "ok"`, all eight links reported, `authed`
  **ok** (the step `/healthz` cannot make), `tls` `skipped` (loopback
  `http://`), `version` ok against klams `0.1.45`.
- `/api/overview` → `configured: true`, 45 authors, 45 agents, 20 recent
  rows, klams `0.1.45`.
- Unit `active` / `enabled` after the restart.

### The defect this sprint fixed, caught one last time on its way out

The dry run and then the real deploy both logged
`rotating /usr/local/bin/klams-view (unknown) -> …` / `(503)`: the
installer could not read the outgoing binary's version, because that
binary is the build without `--version` — exactly the gap `#1013`'s
label-assertion required closing. Baseline for the record:

```
$ /usr/local/bin/klams-view --version        # the OLD binary
Error: binding 127.0.0.1:7779                # tries to start the server
$ curl -s localhost:7779/api/status          # the OLD status shape
{"klams":"ok","view":"ok"}
```

Every future rotation names both versions.
