#!/usr/bin/env bash
# Runs the same requests against either API and checks the status codes.
# Usage: bench/smoke.sh http://localhost:8787
set -euo pipefail
BASE="${1:?usage: smoke.sh <base-url>}"
fail=0

# check <name> <expected-status> <method> <path> [json-body]
check() {
  local name="$1" want="$2" method="$3" path="$4" body="${5:-}"
  local args=(-s -o /tmp/smoke-body -w "%{http_code}" -X "$method" "$BASE$path")
  [[ -n "$body" ]] && args+=(-H "content-type: application/json" -d "$body")
  local got
  got=$(curl "${args[@]}")
  if [[ "$got" == "$want" ]]; then
    echo "ok    $got  $name"
  else
    echo "FAIL  $got  $name (expected $want): $(cat /tmp/smoke-body)"
    fail=1
  fi
}

check "health" 200 GET /health

cal=$(curl -s -X POST "$BASE/calendars" -H "content-type: application/json" -d '{"name":"Meeting room A"}')
id=$(echo "$cal" | sed -E 's/.*"id" ?: ?([0-9]+).*/\1/')
echo "      created calendar $id"

check "book 10:00-11:00"            201 POST "/calendars/$id/bookings" '{"title":"Standup","start":"2026-11-02T10:00:00+09:00","end":"2026-11-02T11:00:00+09:00"}'
booking=$(sed -E 's/.*"id" ?: ?([0-9]+).*/\1/' /tmp/smoke-body)
check "overlapping 10:30-11:30"     409 POST "/calendars/$id/bookings" '{"title":"Clash","start":"2026-11-02T10:30:00+09:00","end":"2026-11-02T11:30:00+09:00"}'
check "back-to-back 11:00-12:00"    201 POST "/calendars/$id/bookings" '{"title":"1on1","start":"2026-11-02T11:00:00+09:00","end":"2026-11-02T12:00:00+09:00"}'
check "end before start"            400 POST "/calendars/$id/bookings" '{"title":"Bad","start":"2026-11-02T15:00:00+09:00","end":"2026-11-02T14:00:00+09:00"}'
check "missing fields"              400 POST "/calendars/$id/bookings" '{"title":"No times"}'
check "invalid JSON"                400 POST "/calendars/$id/bookings" '{"title":'
check "invalid id"                  400 GET  "/bookings/abc"
check "unknown calendar"            404 POST "/calendars/999999999/bookings" '{"title":"x","start":"2026-11-02T10:00:00+09:00","end":"2026-11-02T11:00:00+09:00"}'
check "list bookings"               200 GET  "/calendars/$id/bookings?from=2026-11-02T00:00:00%2B09:00&to=2026-11-03T00:00:00%2B09:00"
echo "      $(cat /tmp/smoke-body)"
check "get booking"                 200 GET  "/bookings/$booking"
check "delete booking"              204 DELETE "/bookings/$booking"
check "get deleted booking"         404 GET  "/bookings/$booking"

exit $fail
