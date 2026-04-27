#!/usr/bin/env bash
set -euo pipefail

require_var() {
    local name="$1"
    if [[ -z "${!name:-}" ]]; then
        echo "missing required environment variable: $name" >&2
        exit 1
    fi
}

if ! command -v jq >/dev/null 2>&1; then
    echo "jq is required but not installed" >&2
    exit 1
fi

require_var SUPABASE_URL
require_var SUPABASE_ANON_KEY
require_var SUPABASE_EMAIL
require_var SUPABASE_PASSWORD

WORK_DATE="${1:-2026-01-27}"
STARTS_AT="${2:-2026-01-27T08:30:00Z}"
ENDS_AT="${3:-2026-01-27T12:00:00Z}"
TARGET_MINUTES="${TARGET_MINUTES:-456}"
SORT_INDEX="${SORT_INDEX:-0}"
METADATA_JSON="${METADATA_JSON:-{\"note\":\"seeded from seed_supabase.sh\"}}"

echo "signing in to Supabase..."
ACCESS_TOKEN="$(curl -fsS \
    -X POST "$SUPABASE_URL/auth/v1/token?grant_type=password" \
    -H "apikey: $SUPABASE_ANON_KEY" \
    -H "Content-Type: application/json" \
    -d "$(jq -nc --arg email "$SUPABASE_EMAIL" --arg password "$SUPABASE_PASSWORD" '{email: $email, password: $password}')" \
    | jq -r '.access_token')"

if [[ -z "$ACCESS_TOKEN" || "$ACCESS_TOKEN" == "null" ]]; then
    echo "failed to obtain access token" >&2
    exit 1
fi

echo "upserting work day for $WORK_DATE..."
DAY_ID="$(curl -fsS \
    -X POST "$SUPABASE_URL/rest/v1/work_days?on_conflict=user_id,work_date" \
    -H "apikey: $SUPABASE_ANON_KEY" \
    -H "Authorization: Bearer $ACCESS_TOKEN" \
    -H "Content-Type: application/json" \
    -H "Prefer: resolution=merge-duplicates,return=representation" \
    -d "$(jq -nc \
        --arg work_date "$WORK_DATE" \
        --argjson target_minutes "$TARGET_MINUTES" \
        '[{work_date: $work_date, target_minutes: $target_minutes, enabled: true}]')" \
    | jq -r '.[0].id')"

if [[ -z "$DAY_ID" || "$DAY_ID" == "null" ]]; then
    echo "failed to obtain work day id" >&2
    exit 1
fi

echo "inserting work entry..."
curl -fsS \
    -X POST "$SUPABASE_URL/rest/v1/work_entries" \
    -H "apikey: $SUPABASE_ANON_KEY" \
    -H "Authorization: Bearer $ACCESS_TOKEN" \
    -H "Content-Type: application/json" \
    -H "Prefer: return=representation" \
    -d "$(jq -nc \
        --arg work_day_id "$DAY_ID" \
        --arg starts_at "$STARTS_AT" \
        --arg ends_at "$ENDS_AT" \
        --argjson metadata "$METADATA_JSON" \
        --argjson sort_index "$SORT_INDEX" \
        '[{
            work_day_id: $work_day_id,
            starts_at: $starts_at,
            ends_at: $ends_at,
            metadata: $metadata,
            sort_index: $sort_index
        }]')"

echo
echo "seeded work entry:"
echo "  work_date: $WORK_DATE"
echo "  work_day_id: $DAY_ID"
echo "  starts_at: $STARTS_AT"
echo "  ends_at: $ENDS_AT"
