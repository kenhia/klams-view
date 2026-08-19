#!/usr/bin/env bash
# Live-backend smoke for the /api aggregation layer (#809).
#
#   scripts/smoke-live.sh [--addr 127.0.0.1:PORT] [--release]
#
# Starts klams-view against a REAL klams (KLAMS_URL / KLAMS_TOKEN) and
# drives every /api route end to end, then starts a second instance with
# a deliberately wrong token and asserts the doctor tells the two
# failures apart. This is the layer `just check` structurally cannot
# see: `cargo test` covers klams-view's own decoding against a stub
# (tests/api_contract.rs), but only a real klams can tell you the
# upstream still speaks the shapes this layer decodes.
#
# READ-ONLY, deliberately. klams-view has no write path and this script
# points at whatever klams you name — including the live one — so it
# seeds nothing and asserts nothing about specific rows. Controlled data
# is the hermetic half's job.
#
# Exit code is the gate: 0 only if every check passed. Version skew is
# reported as an ADVISORY rather than a failure — a green run against a
# newer klams is precisely the evidence needed to bump
# KLAMS_VERIFIED_VERSION in src/doctor.rs, so it must not be the thing
# that stops you.

set -uo pipefail

ADDR=${SMOKE_ADDR:-127.0.0.1:17779}
PROFILE=debug
CARGO_FLAGS=()

while [ $# -gt 0 ]; do
    case "$1" in
        --addr)    ADDR="${2:?--addr needs a value}"; shift 2 ;;
        --release) PROFILE=release; CARGO_FLAGS=(--release); shift ;;
        -h|--help) sed -n '2,26p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *)         printf 'unknown argument: %s\n' "$1" >&2; exit 2 ;;
    esac
done

REPO_DIR=$(cd -- "$(dirname -- "$0")/.." && pwd -P)
cd "$REPO_DIR"

for cmd in curl jq cargo; do
    command -v "$cmd" >/dev/null 2>&1 || { printf 'ERROR: %s not found\n' "$cmd" >&2; exit 1; }
done

# KLAMS_TOKEN has no default, the #682 pattern: a guessed value fails
# later as a 401 that reads like a regression instead of "you did not
# set the variable". KLAMS_URL keeps its documented default.
: "${KLAMS_URL:=http://localhost:7777}"
if [ -z "${KLAMS_TOKEN:-}" ]; then
    cat >&2 <<'EOF'
ERROR: KLAMS_TOKEN is not set.

This smoke needs a real klams to talk to. Set KLAMS_URL and KLAMS_TOKEN
(the repo's gitignored .env is the usual home; `just smoke-live` sources
it), then run again.
EOF
    exit 1
fi

WORK=$(mktemp -d)
PIDS=()
cleanup() {
    for pid in ${PIDS[@]+"${PIDS[@]}"}; do
        kill "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
    done
    rm -rf "$WORK"
}
trap cleanup EXIT

pass=0
fail=0
advisories=()

ok() {
    printf '  ok    %s\n' "$1"
    pass=$((pass + 1))
}
bad() {
    printf '  FAIL  %s\n' "$1"
    printf '        %s\n' "$2"
    fail=$((fail + 1))
}
advise() {
    printf '  ADV   %s\n' "$1"
    advisories+=("$1")
}
heading() { printf '\n%s\n' "$1"; }

# --- boot -------------------------------------------------------------

printf '==> building klams-view (%s)\n' "$PROFILE"
cargo build --quiet ${CARGO_FLAGS[@]+"${CARGO_FLAGS[@]}"} || {
    printf 'ERROR: build failed\n' >&2
    exit 1
}
BIN="$REPO_DIR/target/$PROFILE/klams-view"

# Serve the SPA bundle when one is built, so the deep-link fallback is
# covered too (korg WI #284: ServeDir::fallback, not not_found_service,
# or deep links 404). Without a bundle the run is API-only and that one
# check reports as skipped rather than silently vanishing.
STATIC=""
if [ -f "$REPO_DIR/web/build/index.html" ]; then
    STATIC="$REPO_DIR/web/build"
fi

# `start <port> <token>` — one klams-view on loopback, log to $WORK.
start() {
    local addr=$1 token=$2 name=$3
    env KLAMS_URL="$KLAMS_URL" KLAMS_TOKEN="$token" \
        KLAMS_VIEW_ADDR="$addr" KLAMS_VIEW_STATIC="$STATIC" \
        "$BIN" >"$WORK/$name.log" 2>&1 &
    PIDS+=($!)
    local i
    for i in $(seq 1 100); do
        if curl -fsS "http://$addr/healthz" >/dev/null 2>&1; then return 0; fi
        # A process that died is not going to start answering.
        kill -0 "${PIDS[-1]}" 2>/dev/null || break
        sleep 0.1
    done
    printf 'ERROR: klams-view (%s) did not come up on %s\n' "$name" "$addr" >&2
    sed -n '1,40p' "$WORK/$name.log" >&2
    exit 1
}

# A second port for the bad-token instance; +1 off the first.
host=${ADDR%:*}
port=${ADDR##*:}
BAD_ADDR="$host:$((port + 1))"

printf '==> klams at %s\n' "$KLAMS_URL"
start "$ADDR" "$KLAMS_TOKEN" good
BASE="http://$ADDR"
printf '==> klams-view on %s (static: %s)\n' "$BASE" "${STATIC:-none}"

# --- assertion helpers ------------------------------------------------

# GET a path; body lands in $WORK/body.json. Echoes the status code.
fetch() {
    curl -sS -o "$WORK/body.json" -w '%{http_code}' "$1$2" 2>"$WORK/curl.err"
}

# assert NAME PATH JQ — 200 plus a jq filter that must be truthy.
assert() {
    local name=$1 path=$2 filter=$3 code
    code=$(fetch "$BASE" "$path") || {
        bad "$name" "curl failed: $(head -c 200 "$WORK/curl.err")"
        return
    }
    if [ "$code" != "200" ]; then
        bad "$name" "HTTP $code — $(head -c 240 "$WORK/body.json")"
        return
    fi
    if jq -e "$filter" "$WORK/body.json" >/dev/null 2>&1; then
        ok "$name"
    else
        bad "$name" "shape check failed (\`$filter\`) on: $(head -c 240 "$WORK/body.json")"
    fi
}

# assert_post NAME PATH BODY JQ
assert_post() {
    local name=$1 path=$2 body=$3 filter=$4 code
    code=$(curl -sS -o "$WORK/body.json" -w '%{http_code}' \
        -H 'content-type: application/json' -d "$body" "$BASE$path" 2>"$WORK/curl.err") || {
        bad "$name" "curl failed: $(head -c 200 "$WORK/curl.err")"
        return
    }
    if [ "$code" != "200" ]; then
        bad "$name" "HTTP $code — $(head -c 240 "$WORK/body.json")"
        return
    fi
    if jq -e "$filter" "$WORK/body.json" >/dev/null 2>&1; then
        ok "$name"
    else
        bad "$name" "shape check failed (\`$filter\`) on: $(head -c 240 "$WORK/body.json")"
    fi
}

# Read one value out of a path, for chaining (author id -> author page).
value() {
    fetch "$BASE" "$1" >/dev/null && jq -r "$2" "$WORK/body.json" 2>/dev/null
}

iso_ago() { python3 -c "import datetime,sys;print((datetime.datetime.now(datetime.UTC)-datetime.timedelta(hours=int(sys.argv[1]))+datetime.timedelta(minutes=1)).strftime('%Y-%m-%dT%H:%M:%SZ'))" "$1"; }

# --- 1. the doctor ----------------------------------------------------

heading '-- doctor (/api/status)'
assert 'doctor answers with a full chain' /api/status \
    '(.checks|length) >= 8 and (.view.klams_url|type) == "string"'
assert 'every link is ok or not-applicable' /api/status \
    '[.checks[]|select(.state=="fail")]|length == 0'
assert 'the authenticated step passed' /api/status \
    '.checks[]|select(.id=="authed")|.state == "ok"'

klams_version=$(value /api/status '.view.klams_version // "unknown"')
verified=$(value /api/status '.view.klams_verified')
version_state=$(value /api/status '.checks[]|select(.id=="version")|.state')
case "$version_state" in
    ok) ok "klams $klams_version matches the verified version" ;;
    warn)
        advise "klams is $klams_version, klams-view is verified against $verified — if this run is green, bump KLAMS_VERIFIED_VERSION in src/doctor.rs" ;;
    *) bad 'klams version' "version check reported $version_state (klams $klams_version vs $verified)" ;;
esac

# --- 2. aggregations --------------------------------------------------

heading '-- aggregations'
assert 'overview renders Pulse in one call' /api/overview \
    '.configured == true
     and (.health.status|type) == "string"
     and (.totals.authors|type) == "number" and .totals.authors > 0
     and (.agents|length) > 0
     and (.recent|type) == "array"
     and (.metrics.queue.capacity|type) == "number"'

since24=$(iso_ago 24)
assert 'activity buckets 24h hourly' "/api/activity?since=$since24&bucket=hour" \
    '.bucket_hours == 1 and (.buckets|length) > 0
     and (.buckets[0]|has("fact") and has("knowledge") and has("event"))
     and (.total|type) == "number" and (.by_author|type) == "array"'

# The 30-day preset once 400d with `window_too_large`: the browser
# computes `since` a beat before klams stamps `until = now`, so a bare
# 30 days lands a hair over klams' window cap (commit 13c6707).
since30d=$(iso_ago 720)
assert 'activity 30d preset stays inside klams window cap' \
    "/api/activity?since=$since30d&bucket=day" \
    '.bucket_hours == 24 and (.buckets|length) > 0'

all_total=$(value "/api/activity?since=$since24&bucket=hour" '.total')
filtered_total=$(value "/api/activity?since=$since24&bucket=hour&include_scanners=false" '.total')
# Both must be integers before they can be compared — a 502 upstream
# leaves them as `null`, and `[ null -le null ]` is a shell error, not a
# check result.
if [[ "$all_total" =~ ^[0-9]+$ && "$filtered_total" =~ ^[0-9]+$ && "$filtered_total" -le "$all_total" ]]; then
    ok "scanner exclusion narrows the window ($filtered_total of $all_total)"
else
    bad 'scanner exclusion' "include_scanners=false gave $filtered_total, unfiltered gave $all_total"
fi

assert 'metrics summary parses the prometheus text' /api/metrics/summary \
    '(.queue.capacity|type) == "number"
     and (.writes_accepted|type) == "object"
     and (.latency|has("search_p95") and has("context_p95"))
     and (.backup|has("last_success_unix"))'

assert 'metrics history is a sampler envelope' /api/metrics/history \
    '(.samples|type) == "array"'

assert 'health relays the full klams snapshot' /api/health \
    '(.status|type) == "string" and (.version|type) == "string"
     and (.queue.capacity|type) == "number"'

# --- 3. passthroughs --------------------------------------------------

heading '-- passthroughs'
assert 'memories page carries flattened rows' '/api/memories?limit=5' \
    '(.memories|length) > 0
     and (.memories[0]|has("id") and has("kind") and has("created_at"))
     and (.memories[0].author.agent_name|type) == "string"'

assert 'authors page carries counts' '/api/authors?limit=5' \
    '(.authors|length) > 0
     and (.authors[0].id|type) == "string"
     and (.authors[0].counts|has("writes") and has("knowledge") and has("events"))'

author_id=$(value '/api/authors?limit=1' '.authors[0].id')
if [ -n "$author_id" ] && [ "$author_id" != "null" ]; then
    assert 'author detail' "/api/authors/$author_id" \
        '(.agent_name|type) == "string" and (.counts.writes|type) == "number"'
    assert 'author memories' "/api/authors/$author_id/memories?limit=5" \
        '(.memories|type) == "array"'
    # #807's per-author activity strip: the same aggregation, filtered
    # by author UUID upstream. Scanners are included on purpose — an
    # author page for a scanner must still chart something.
    assert 'per-author activity (#807)' \
        "/api/activity?since=$since30d&bucket=day&authors=$author_id&include_scanners=true" \
        '(.buckets|length) > 0 and (.by_author|length) <= 1'
else
    bad 'author chaining' 'could not read an author id out of /api/authors'
fi

# `author.id` is what the memory -> author jump links to (#807). It is
# Option<Uuid> upstream — absent only when the author could not be
# resolved — so a store where it is missing everywhere would silently
# disable the link.
with_ids=$(value '/api/memories?limit=20' '[.memories[]|select(.author.id != null)]|length')
if [[ "$with_ids" =~ ^[0-9]+$ && "$with_ids" -gt 0 ]]; then
    ok "memory rows carry author ids ($with_ids of 20) — the author jump has a target"
else
    bad 'memory -> author link target' 'no row in the first 20 memories carries author.id'
fi

kn_id=$(value '/api/memories?limit=1&kinds=knowledge' '.memories[0].id // ""')
if [ -n "$kn_id" ]; then
    assert 'knowledge detail (the richest shape)' "/api/knowledge/$kn_id" \
        '(.text|type) == "string" and (.content_hash|type) == "string"
         and (.confidence|type) == "number" and (.use_count|type) == "number"'
else
    advise 'no knowledge rows in this store — skipped the /api/knowledge shape check'
fi

assert_post 'search returns a scored envelope' /api/search \
    '{"query":"klams","top_k":3}' \
    '(.results|type) == "array" and (.total|type) == "number" and (.degraded|type) == "boolean"'

# --- 4. the SPA shell -------------------------------------------------

heading '-- SPA shell'
if [ -n "$STATIC" ]; then
    # Deep links must be 200, not 404 (korg WI #284): the fallback is
    # ServeDir::fallback(ServeFile), never not_found_service.
    code=$(curl -sS -o "$WORK/deep.html" -w '%{http_code}' "$BASE/authors/$author_id")
    if [ "$code" = "200" ] && grep -qi '<html' "$WORK/deep.html"; then
        ok 'deep link serves the SPA shell with 200'
    else
        bad 'deep link' "GET /authors/<id> gave HTTP $code (must be 200 with the shell)"
    fi
else
    advise 'no web/build bundle — skipped the deep-link fallback check (run `pnpm build` in web/)'
fi

# --- 5. the bad-token instance (the #739 headline case) ---------------

heading '-- bad token (klams #739 / WI #808)'
start "$BAD_ADDR" 'deliberately-wrong-token' bad
BASE="http://$BAD_ADDR"

assert 'doctor calls it down' /api/status '.overall == "down"'
assert 'reachability still reads green — the trap, stated out loud' /api/status \
    '(.checks[]|select(.id=="tcp")|.state) == "ok"
     and (.checks[]|select(.id=="healthz")|.state) == "ok"'
assert 'the authenticated step is the one that fails' /api/status \
    '(.checks[]|select(.id=="authed")) as $a
     | $a.state == "fail" and ($a.detail|test("401"))
     and ($a.fix|test("read-scoped grant"))'

code=$(fetch "$BASE" '/api/memories?limit=1')
if [ "$code" = "401" ] && jq -e '.code == "unauthorized"' "$WORK/body.json" >/dev/null 2>&1; then
    ok 'reads relay 401 unauthorized rather than a flattened 502'
else
    bad 'rejected-token relay' "GET /api/memories gave HTTP $code — $(head -c 200 "$WORK/body.json")"
fi

# --- verdict ----------------------------------------------------------

printf '\n'
if [ "${#advisories[@]}" -gt 0 ]; then
    printf 'advisories:\n'
    for a in "${advisories[@]}"; do printf '  * %s\n' "$a"; done
    printf '\n'
fi

if [ "$fail" -eq 0 ]; then
    printf 'smoke-live: %d passed, 0 failed, %d advisory — /api speaks klams %s\n' \
        "$pass" "${#advisories[@]}" "$klams_version"
    exit 0
fi
printf 'smoke-live: %d passed, %d FAILED, %d advisory\n' "$pass" "$fail" "${#advisories[@]}"
printf 'server log (good instance):\n'
sed -n '1,60p' "$WORK/good.log" | sed 's/^/  /'
exit 1
