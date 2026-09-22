# 005 — Merge the author memory list into one timeline

Proposal korg:3055 (leg of program korg:3062, *Low-hanging fruit, run 2*),
covering **WI #1448**. Overseen sprint: findings return to the overseer as a
korg handoff, and the ship waits on its green light.

## Goal

`/authors/[id]` showed an author's memories as klams' **kind segments**, not
as a timeline. Make `/api/authors/{id}/memories` serve one newest-first
all-time list, so the page stops contradicting the activity chart directly
above it.

## The premise, re-measured

The item was written against klams 0.1.45; live is **0.1.52**. Same bug, same
shape — author `claude`, `kinds=knowledge,fact,event`:

| page | rows | kind | range |
|---|---|---|---|
| 1 | 4 | `fact` | 2026-09-13 → 2026-09-13 (desc) |
| 2 | 6 | `event` | 2026-08-08 → 2026-07-10 (desc) |
| 3 | 50 | `knowledge` | 2026-07-10 → 2026-07-23 (**asc**) |

Premise holds; only the version moved.

## The decision: the chosen fix shape was infeasible

The proposal specified "fetch per selected kind and merge newest-first
server-side, with one opaque cursor per kind segment". That cannot be built,
and the reason is in klams' store:

- The cursor is `base64("<section>:<ts_nanos>:<uuid>")`, sections `f` → `e` →
  `k` in fixed order (`klams-store/src/composite.rs`,
  `list_author_memories_impl`).
- The `k` section is a **Qdrant scroll keyed on `after_id`** — ascending by
  point uuid, no descending option, no `since` filter. The route's params are
  only `limit`/`cursor`/`kinds`/`state`.

So **there is no newest-first knowledge stream for an author**. Merging per
kind requires one; getting it means scrolling the author's entire knowledge
segment. Measured live, that is fatal: `klams-scanner` holds **103,856**
knowledge rows and `kai-scanner` **93,849** — ~520 upstream requests to render
one page.

That is the klams API change the brief said to push back on rather than build
around, so the mechanism moved (the deliverable did not).

## What shipped instead

`/v1/memories` **already** does this merge correctly — klams #54 replaced the
same section-order bug there with one `(created_at, id)` keyset across all
three kinds, knowledge ordered by a datetime index. It was simply never
applied to the author-scoped route.

Its 30-day limit is on the **width** of `since..until`, not on how far back the
pair may sit (`validate_window`). So `/api/authors/{id}/memories` now **walks
that window backwards in 30-day steps**, newest-first, accumulating until it
has `limit` rows or reaches the author's own `created_at`.

Details that matter:

- **The floor is real, not a guess.** The author row is written before any
  memory can reference it, so `author.created_at` bounds the walk. Confirmed
  live: `claude` created 00:55:50, oldest memory 01:58:47.
- **Windows tile exactly.** klams' SQL filters `created_at >= since AND
  created_at < until` — half-open, so the next window's `until` is this one's
  `since`, with no overlap and no epsilon arithmetic.
- **Each window asks for only what is missing** (`limit - out.len()`). klams
  returns a short page *only* when the window is exhausted
  (`take_merged_page`), so a page that fits can never hide rows the walk then
  steps past. This is the gap-freeness argument.
- **One opaque token.** The cursor packs the current window and klams' cursor
  within it, base64url. The browser never learns there are two halves.
- Resolving the author first also keeps an unknown id a **404** and a refused
  identity a **401**, rather than an empty timeline that reads as "wrote
  nothing".

`/v1/memories` rows are a **superset** of the author route's (`source_path`,
`volatility` added, both optional), so the SPA needed no type change — only
the deletion of the notice that explained the old ordering.

## Verified live (klams 0.1.52, from kubs0 — the host that runs klams-view)

- Author `claude`: full walk = **299 rows, 299 unique ids, 0 out-of-order
  pairs**, all three kinds, 2026-09-22 back to 2026-07-10 (their oldest
  memory). Counts match the author record exactly: 289 knowledge, 6 events,
  4 facts.
- Page 1 is now today's writes. It was four facts from 2026-09-13.
- `klams-scanner` (103,856 knowledge): first page **0.425 s**, correctly
  ordered — the heavy single-kind case does not walk.
- Dormant author (created 2026-05-25, four empty windows): **17 ms**.
- `kyac` shows a genuine interleave — knowledge at 17:16:31 sits between
  events at 17:16:42 and 17:13:10.

## Tests

Six contract tests (`tests/api_contract.rs`), and the stub gained the
fidelity they need: klams' 30-day window cap (it now 400s `window_too_large`),
the `kinds` filter, half-open window bounds, and cursor paging. `AUTHOR_A`
gained a four-month history so one page cannot come from one window.

- one newest-first merge, not kind segments — asserts alternating kinds, which
  a section order cannot produce, and that the segmented endpoint is no longer
  called at all
- never asks klams for more than a 30-day window
- the composite cursor pages without gaps or repeats (walked at `limit=2`, so
  the boundary lands mid-window as well as on a window edge)
- the kind filter is forwarded upstream
- an unknown author is still a 404
- a corrupt cursor is a 400, not a 500

`just check` green: clippy `-D warnings`, 50 tests, svelte-check 0 errors,
prettier, SPA build. `just smoke-live` 26 passed / 0 failed / 0 advisories.

### The live check had to be made a real control

`smoke-live`'s `author memories` assertion only checked `.memories` was an
array — it passed with the bug in place. The first attempt at replacing it was
**also worthless**, twice over, and both were caught by testing the assertion
against the old behaviour rather than assuming it:

1. It took the author `/api/authors` happens to return first, which here is
   facts-only — and *any* ordering assertion passes on a single-kind list.
2. Ordering alone cannot catch this bug anyway: the old endpoint served one
   kind **section** per page, so a single page was usually ordered fine within
   itself. The bug only showed across pages.

So the check now picks an author with more than one kind **that wrote inside
the window** (last *seen* is not last *wrote* — the busiest multi-kind author
here was seen minutes ago and last wrote two months back), and asserts the
timeline leads with the newest row as known independently from
`/api/memories?authors=`. Measured control: the old endpoint led with
2026-09-13 where the true newest was 2026-09-22. When no such author exists it
raises an advisory rather than silently reporting a check that could not fail.

## Repaired in passing

- **`KLAMS_VERIFIED_VERSION` 0.1.49 → 0.1.52** (`src/doctor.rs`).
  `smoke-live` raised it as an advisory naming the bump as the action, and the
  run was green against 0.1.52; the whole sprint was verified against it. The
  gate proves it (`doctor` unit tests + `smoke-live`, now 0 advisories).
- **`smoke-live`'s author-memories check**, above. It was a live assertion
  that could not fail; it is now a control with a measured negative case.

## Filed, not repaired

- **klams #3079** — apply `list_memories_impl`'s merge to
  `list_author_memories_impl`, so `/v1/authors/{id}/memories` is a timeline
  upstream. Filed rather than repaired because it is **another repo's public
  API contract**: it changes the response ordering of a published endpoint for
  every client, which is a decision klams owns. klams-view does not wait on
  it — this sprint's route is independent.

## Follow-ups

None outstanding in this repo.
