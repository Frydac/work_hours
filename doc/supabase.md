# Supabase Setup

This project stores work-hour data in two tables:

- `work_days`: one row per user and calendar date
- `work_entries`: one row per recorded time range

The database setup is split into ordered migrations under [supabase/migrations](/home/emile/repos/rust/work_hours/supabase/migrations/).

Apply them in filename order:

- [20260425103000_work_hours.sql](/home/emile/repos/rust/work_hours/supabase/migrations/20260425103000_work_hours.sql)
  - creates the base tables, indexes, triggers, and RLS policies
- [20260426113000_save_work_day_rpc.sql](/home/emile/repos/rust/work_hours/supabase/migrations/20260426113000_save_work_day_rpc.sql)
  - adds the `save_work_day_with_entries(...)` RPC function
  - adds the unique index used by the RPC upsert logic

The current app code expects both migrations to be present.

## Migrations

For a fresh Supabase project:

1. Run `20260425103000_work_hours.sql`
2. Run `20260426113000_save_work_day_rpc.sql`

For an existing project that already has the tables:

1. Run any newer migration files you have not applied yet
2. In the current state of this repo, that means at least `20260426113000_save_work_day_rpc.sql`

If the app can log in and load data but save fails with a `404` / `PGRST202` error mentioning `save_work_day_with_entries`, the RPC migration is missing or PostgREST has not reloaded its schema cache yet.

## Why two tables

The app already models:

- a day-level target
- a day-level enabled flag
- multiple time ranges per day

Keeping those in separate tables avoids repeating `target_minutes` and `enabled` on every entry row.

## Auth

Use Supabase Auth. Do not create a separate users table for application login.

- `work_days.user_id` references `auth.users(id)`
- `user_id` defaults to `auth.uid()`
- Row Level Security restricts reads and writes to the authenticated user's rows

## Public config

The app now loads public Supabase config through `AppConfig` in [config.rs](/home/emile/repos/rust/work_hours/src/config.rs:1).

- `SUPABASE_URL`
- `SUPABASE_ANON_KEY`

These are safe to ship in the client. Do not ship the `service_role` key.

Native builds:

- first try runtime environment variables
- if not present, fall back to compile-time `option_env!`

WASM builds:

- read the values at compile time with `option_env!`
- this means the variables must be present in the shell where you run `trunk serve` or `trunk build`

Examples:

```bash
export SUPABASE_URL="https://your-project-id.supabase.co"
export SUPABASE_ANON_KEY="your-public-anon-key"
cargo run --release
```

```bash
export SUPABASE_URL="https://your-project-id.supabase.co"
export SUPABASE_ANON_KEY="your-public-anon-key"
trunk serve
```

To seed one test day and one entry through the REST API:

```bash
export SUPABASE_URL="https://your-project-id.supabase.co"
export SUPABASE_ANON_KEY="your-public-anon-key"
export SUPABASE_EMAIL="you@example.com"
export SUPABASE_PASSWORD="your-password"
bash ./seed_supabase.sh
```

Optional arguments:

```bash
bash ./seed_supabase.sh 2026-01-27 2026-01-27T08:30:00Z 2026-01-27T12:00:00Z
```

Optional environment overrides:

- `TARGET_MINUTES`
- `SORT_INDEX`
- `METADATA_JSON`

## Rust API

`src/supabase.rs` now contains:

- `SupabaseClient::sign_in_password`
- `SupabaseClient::refresh_session`
- `SupabaseClient::get_work_day`
- `SupabaseClient::get_work_days_range`
- `SupabaseClient::save_work_day`

It also contains `WorkDayDraft`, which converts between Supabase rows and the existing `ui::Day` app model.

`save_work_day` now uses the Supabase RPC `save_work_day_with_entries(...)` instead of composing a client-side delete/insert sequence.

## App behavior

The main egui app now has a first-pass login and sync flow:

- stores a Supabase session in local `eframe` persistence
- restores that session on startup
- refreshes it automatically when needed
- shows login status in the header using the user email when available
- auto-loads the selected week from Supabase after login and on week navigation
- keeps edits local until the user clicks `Save`

Current limitations:

- session storage is app-local persistence, not OS keychain storage
- save is manual; there is no auto-save yet
- logged-out mode still uses local state only

## Troubleshooting

If save fails but login works:

1. Confirm every SQL file in `supabase/migrations/` has been applied in order.
2. If the error mentions `save_work_day_with_entries` not being found, run the RPC migration and then:

```sql
notify pgrst, 'reload schema';
```

3. Run the app with logging enabled:

```bash
RUST_LOG=work_hours_calculator=debug cargo run --release
```

On web, use `trunk serve` and inspect the browser console.
