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

## Shipped

- `src/doctor.rs` — the chain, with 8 unit tests on the skew rules.
- `/api/status` reshaped; `Doctor.svelte` renders it on `/health`.
- `src/lib.rs` — a lib target, so `tests/` can drive the real router
  (a contract test that reaches through `main.rs` cannot exist).
- `tests/api_contract.rs` — 17 contract tests, hermetic.
- `scripts/smoke-live.sh` + `just smoke-live` — 24 live checks.
- README gains Testing and "Diagnosing a bad connection"; docs/design.md
  gains the new `/api/status` row and a testing section.

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
