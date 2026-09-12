# Sprint 004 — klams identity header instead of a bearer token

**Proposal:** korg:2422 (covers #2397)
**Program:** korg:2440 — simplify homelab secrets, slice 3 of 22
**Started:** 2026-09-12

## Goal

klams-view stops presenting a bearer token to klams and instead
declares who it is: `X-Homelab-Agent: klams-view`, which klams
allow-lists read-scoped in `[[auth.identities]]`. Drop `KLAMS_TOKEN`
from both env files, and turn `docs/deploy.md`'s "mint a grant"
section into "add an identity row".

The program's premise, in one line: on a single-user tailnet, with his
agents, one trust domain and sudo everywhere, a shared bearer token
between two of our own services was a **name tag, not a lock**. So it
becomes a name.

## Premise check

Verified against the live klams on `:7777` **from kubs0** — the host
that runs klams-view, so the probe measures the thing it is used to
mean. Slice 1 (korg:2419) had already shipped, so this was a check of
deployed reality, not of a plan:

| probe | result |
|---|---|
| `X-Homelab-Agent: klams-view` on every route klams-view calls | 200 |
| POST write routes under that identity | 403 |
| an agent name klams does not know | 401 |
| the old bearer token | 200 — transition window still open |

`/etc/klams/klams.toml` already carried `agent_name = "klams-view"`,
`scopes = ["read"]` beside the legacy token row. **Premise holds.**

A note worth keeping: the API-truth clone this repo's `CLAUDE.md` sent
me to (`/home/ken/tmp-clone/klams`) does not exist, and the working
copy's router did not yet show the header. The live service settled
it. See "Repaired in passing".

## Decisions

### The identity is configurable, defaulting to `klams-view`

`KLAMS_AGENT`, defaulting to `klams-view`. The obvious alternative was
a hardcoded constant — it *is* a constant in every real deployment —
but a hardcoded name silently deletes a test we already had. The smoke
ran a second instance with a deliberately wrong token to prove the
doctor tells "unreachable" from "unauthorized" apart (the klams #739
headline case, WI #808). With no way to declare a wrong name, that
instance has nothing to be wrong about and the assertion quietly
becomes untestable.

So the override exists to keep the negative path reachable. It is not
a secret, so it costs nothing to expose: the env examples ship it
commented out, because the default is the right answer everywhere.

### The "unconfigured" state is gone, not renamed

This is the change with the most reach, and it is the point of the
program rather than a side effect. A token could be absent; an
identity cannot, because it has a default. Everything that existed to
describe "no credential set" therefore had no state left to describe:

- `Client::has_token()` and the `Option<String>` token field.
- `upstream_err`'s `503 unconfigured` branch — an unknown name now
  comes back as klams' own 401 through the relay, which is the honest
  report and was already being relayed correctly.
- `config.rs`'s startup `warn!`.
- `probe_authed`'s `Option` return, and the doctor's `authed: skipped`
  arm. The authed step is now **always really attempted**, which is
  strictly better: the step that exists to catch auth failure can no
  longer decline to run.
- The test `no_token_configured_is_503_unconfigured_not_a_panic`, whose
  panic-safety is covered by the test directly above it (an unknown
  identity relays 401 rather than a flattened 502).

### The doctor's `token` step became an `identity` step, not a deletion

It could have been dropped — there is no unset state to catch. It
earns its place with a changed job: it prints **which name we are
sending**, which is the one string an operator must match against
klams' `[[auth.identities]]`, and which is not a secret. The test
asserts it reports the *configured* name rather than a hardcoded one;
otherwise a mismatched deployment would read as correct.

The chain is now `config → identity → dns → tcp → tls → healthz →
authed → version`.

### The Pulse banner: `configured` became `authed`

The frontend had a banner reading "KLAMS_TOKEN is not configured —
showing public health and metrics only", driven by `overview.configured`.
Left alone it would have been permanently false and permanently dead.

Deleting it was wrong: the banner exists for the klams #739 shape —
`/healthz` green while every store read fails — and a **rejected
identity reproduces that shape exactly**. So the field became `authed`,
set from whether the authed aggregation actually succeeded, and the
banner now says klams rejected the identity. Same safeguard, true
premise.

### The two remaining `KLAMS_TOKEN` strings are history and stay

`deploy/install-from-store.sh` and `tests/install_from_store.rs` both
quote `"KLAMS_TOKEN not set — /api routes will return 503"`. That is a
verbatim record of what the pre-0.1.3 binary logged during the 0.1.3
deploy, which is how `report_version` came to record a version of
"503". Rewriting it would falsify an incident record to tidy a grep.
The fixture is explicitly a stand-in for "a line whose last field is a
number" and still is one.

## Shipped

- `src/klams.rs` — `AGENT_HEADER` const; `token: Option<String>` →
  `agent: String`; `authed()` (fallible) → `identified()` (infallible);
  `has_token()` → `agent()`.
- `src/config.rs` — `klams_token` → `klams_agent`, `KLAMS_AGENT` with a
  default.
- `src/api.rs` — `upstream_err` loses the unconfigured branch;
  `overview` always attempts the store read; `configured` → `authed`.
- `src/doctor.rs` — the identity step, identity-aware 401/403 detail,
  `IDENTITY_REJECTED_FIX` naming the TOML row to add.
- `web/` — `Overview.authed`, banner and empty-state text.
- `tests/api_contract.rs` — the stub allow-lists an agent name instead
  of checking a bearer; the missing-token test is replaced by
  `doctor_names_the_identity_it_declares`.
- `scripts/smoke-live.sh` — no required secret at all now; the
  bad-token instance became an unknown-identity instance.
- Env files, `deploy/`, `docs/deploy.md` ("The token" → "The identity",
  with the `[[auth.identities]]` row to add), `README.md`, `CLAUDE.md`,
  `.github/copilot-instructions.md`, `justfile`.

### Verification

- `just check` — green.
- `just smoke-live` against the live klams (0.1.49): **24 passed, 0
  failed, 0 advisory**, with no token in the env file. Both halves of
  #2397's acceptance: the dashboard renders end to end, and a write
  under this identity is refused (403).

## Repaired in passing

- **`KLAMS_VERIFIED_VERSION` was two releases stale** (0.1.45; klams is
  0.1.49). The smoke itself emits the advisory and the README documents
  a green run as exactly what licenses the bump, so the evidence
  removed the decision. Bumped; the advisory is gone and the smoke is
  24/24 clean.
- **`CLAUDE.md` and `.github/copilot-instructions.md` pointed at clones
  that do not exist** — `/home/ken/tmp-clone/klams` and
  `/home/ken/tmp-clone/korg` — while telling the reader *not* to use
  the real working copy. Following the instruction as written yields an
  empty directory and a silently wrong answer about the klams API,
  which is precisely the trap this sprint's premise check had to walk
  out of. Now: klams is at `~/src/ai/klams`, read-only, check
  `git status` first; korg is not on kubs0 at all (reach it through
  kaed's `kai:src`); and the live service on `:7777` is the API truth a
  working copy can only approximate.

## Follow-ups

None filed. The two obvious candidates are already owned elsewhere:
deleting klams' `[[auth.tokens]]` rows is korg:2450 (which `depends_on`
this slice), and retiring the age-store entries for those tokens is
k-homelab WI #2456.

## Deployed 2026-09-12

**What shipped:** klams-view **0.1.4** — binary and SPA bundle — published to
the homelab package store and installed on kubs0 from it (`just publish` →
`just deploy`). `latest` moved 0.1.3 → 0.1.4. Previous binary and bundle are
rotated to `.prev`; `just rollback` swaps both back together.

**Ordering, which was the whole risk.** The clearance on korg:2422 fixed the
sequence and it was followed exactly: publish → deploy → *then* remove
`KLAMS_TOKEN` from `/etc/klams-view/klams-view.env` → restart → confirm.
Removing the token first would have taken the live dashboard down, because
until the new binary is installed the running one still needs it.

`/etc/klams-view` is outside every kaed root, so that edit took the documented
fallback rather than kaed — the line was deleted without the value being read
or printed. File perms unchanged (`0640 root:klams-view`). No plaintext backup
was left behind; the value still exists in klams' own config and the age store
until korg:2450 deletes it, which is also what a rollback to 0.1.3 would need.

**Verified live, after the token was gone:**

| check | result |
|---|---|
| service | `active`, reporting `klams-view 0.1.4` |
| doctor `/api/status` | `overall: ok` — every link green, TLS skipped (http upstream) |
| doctor identity row | reports `sending X-Homelab-Agent: klams-view` |
| doctor authed row | klams accepts `klams-view`, read-scoped |
| `/api/overview` | `authed: true`; 47 authors, 200118 knowledge, 58 facts, 38 events, 20 recent |
| SPA shell | `GET /` → 200 |
| write under this identity | `POST /memory/events` → 403 |

There is now **no credential anywhere in klams-view's configuration**, on this
host or in the repo, and the dashboard renders end to end. That is #2397's
acceptance, literally rather than in principle.

### Repaired in passing, at ship time

The version was still `0.1.3` — the same version already published and
serving as the store's `latest` from sprint 003. Publishing this sprint's code
under it would have replaced a released artifact's contents in place, leaving
the store saying `0.1.3` while serving different code. Sprint 003 bumped
inside its own PR; this sprint missed it. Caught before the publish and fixed
through its own PR (#5) rather than pushing code straight to `main`, since the
push-to-main exemption covers the deploy record only.
