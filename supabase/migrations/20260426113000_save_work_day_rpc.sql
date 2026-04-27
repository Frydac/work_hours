create unique index if not exists work_entries_work_day_sort_unique_idx
on public.work_entries (work_day_id, sort_index);

create or replace function public.save_work_day_with_entries(
    p_work_date date,
    p_target_minutes integer,
    p_enabled boolean,
    p_entries jsonb
)
returns jsonb
language plpgsql
security invoker
as $$
declare
    v_day public.work_days%rowtype;
begin
    insert into public.work_days (user_id, work_date, target_minutes, enabled)
    values (auth.uid(), p_work_date, p_target_minutes, p_enabled)
    on conflict (user_id, work_date)
    do update
    set target_minutes = excluded.target_minutes,
        enabled = excluded.enabled
    returning * into v_day;

    delete from public.work_entries
    where work_day_id = v_day.id
      and not exists (
          select 1
          from jsonb_array_elements(coalesce(p_entries, '[]'::jsonb)) as entry
          where (entry->>'sort_index')::integer = public.work_entries.sort_index
      );

    insert into public.work_entries (work_day_id, starts_at, ends_at, metadata, sort_index)
    select
        v_day.id,
        (entry->>'starts_at')::timestamptz,
        (entry->>'ends_at')::timestamptz,
        coalesce(entry->'metadata', '{}'::jsonb),
        (entry->>'sort_index')::integer
    from jsonb_array_elements(coalesce(p_entries, '[]'::jsonb)) as entry
    on conflict (work_day_id, sort_index)
    do update
    set starts_at = excluded.starts_at,
        ends_at = excluded.ends_at,
        metadata = excluded.metadata;

    return to_jsonb(v_day) || jsonb_build_object(
        'work_entries',
        coalesce(
            (
                select jsonb_agg(to_jsonb(e) order by e.sort_index)
                from public.work_entries e
                where e.work_day_id = v_day.id
            ),
            '[]'::jsonb
        )
    );
end;
$$;

grant execute on function public.save_work_day_with_entries(date, integer, boolean, jsonb) to authenticated;
