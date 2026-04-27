create extension if not exists pgcrypto;

create table if not exists public.work_days (
    id uuid primary key default gen_random_uuid(),
    user_id uuid not null default auth.uid() references auth.users (id) on delete cascade,
    work_date date not null,
    target_minutes integer not null,
    enabled boolean not null default true,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint work_days_target_minutes_non_negative check (target_minutes >= 0),
    constraint work_days_user_date_unique unique (user_id, work_date)
);

create table if not exists public.work_entries (
    id uuid primary key default gen_random_uuid(),
    work_day_id uuid not null references public.work_days (id) on delete cascade,
    starts_at timestamptz not null,
    ends_at timestamptz not null,
    metadata jsonb not null default '{}'::jsonb,
    sort_index integer not null default 0,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint work_entries_time_order check (ends_at > starts_at)
);

create index if not exists work_days_user_date_idx on public.work_days (user_id, work_date);
create index if not exists work_entries_work_day_sort_idx on public.work_entries (work_day_id, sort_index);
create index if not exists work_entries_metadata_gin_idx on public.work_entries using gin (metadata);

create or replace function public.set_updated_at()
returns trigger
language plpgsql
as $$
begin
    new.updated_at = now();
    return new;
end;
$$;

drop trigger if exists set_work_days_updated_at on public.work_days;
create trigger set_work_days_updated_at
before update on public.work_days
for each row
execute function public.set_updated_at();

drop trigger if exists set_work_entries_updated_at on public.work_entries;
create trigger set_work_entries_updated_at
before update on public.work_entries
for each row
execute function public.set_updated_at();

alter table public.work_days enable row level security;
alter table public.work_entries enable row level security;

drop policy if exists "users_manage_own_work_days" on public.work_days;
create policy "users_manage_own_work_days"
on public.work_days
for all
to authenticated
using (user_id = auth.uid())
with check (user_id = auth.uid());

drop policy if exists "users_manage_own_work_entries" on public.work_entries;
create policy "users_manage_own_work_entries"
on public.work_entries
for all
to authenticated
using (
    exists (
        select 1
        from public.work_days d
        where d.id = work_entries.work_day_id
          and d.user_id = auth.uid()
    )
)
with check (
    exists (
        select 1
        from public.work_days d
        where d.id = work_entries.work_day_id
          and d.user_id = auth.uid()
    )
);
