#!/usr/bin/env bash
# Measures response time for a first request: every curl call opens a new
# connection (TCP + TLS), like a user arriving at the site.
# Usage: bench/latency.sh <url> [requests]
# Example: bench/latency.sh "https://calendar-api-bench.example.workers.dev/calendars/1/bookings?from=2026-11-01T00:00:00Z&to=2026-12-01T00:00:00Z"
set -euo pipefail
URL="${1:?usage: latency.sh <url> [requests]}"
N="${2:-20}"

for _ in $(seq "$N"); do
  curl -s -o /dev/null -w "%{time_total}\n" "$URL"
done | sort -n | awk '
  { t[NR] = $1 * 1000 }
  END {
    printf "requests: %d\n", NR
    printf "median:   %.0f ms\n", t[int((NR + 1) / 2)]
    printf "p90:      %.0f ms\n", t[int(NR * 0.9 + 0.5)]
    printf "min/max:  %.0f / %.0f ms\n", t[1], t[NR]
  }'
