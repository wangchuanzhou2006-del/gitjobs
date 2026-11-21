create or replace function complete_talent_source_run(p_run jsonb)
returns boolean as $$
declare
    v_success boolean := nullif(p_run->>'error_code', '') is null;
    v_subscription_id uuid;
begin
    if p_run is null or jsonb_typeof(p_run) <> 'object' then
        raise exception 'source run must be an object' using errcode = '22023';
    end if;
    update talent_source_runs set
        discovered_count = greatest(0, coalesce((p_run->>'discovered_count')::integer, 0)),
        accepted_count = greatest(0, coalesce((p_run->>'accepted_count')::integer, 0)),
        duplicate_count = greatest(0, coalesce((p_run->>'duplicate_count')::integer, 0)),
        failed_count = greatest(0, coalesce((p_run->>'failed_count')::integer, 0)),
        error_code = nullif(p_run->>'error_code', ''),
        completed_at = current_timestamp
    where run_id = (p_run->>'run_id')::uuid and completed_at is null
    returning subscription_id into v_subscription_id;
    if v_subscription_id is null then return false; end if;

    update talent_source_subscriptions set
        claimed_at = null,
        incremental_cursor = case when v_success then coalesce(nullif(p_run->>'incremental_cursor', ''), incremental_cursor) else incremental_cursor end,
        etag = case when v_success then coalesce(nullif(p_run->>'etag', ''), etag) else etag end,
        last_modified = case when v_success then coalesce(nullif(p_run->>'last_modified', ''), last_modified) else last_modified end,
        consecutive_failures = case when v_success then 0 else consecutive_failures + 1 end,
        last_success_at = case when v_success then current_timestamp else last_success_at end,
        last_error_at = case when v_success then last_error_at else current_timestamp end,
        last_error_code = nullif(p_run->>'error_code', ''),
        next_run_at = current_timestamp + case
            when v_success then make_interval(mins => interval_minutes)
            else make_interval(mins => least(interval_minutes, 15 * (1 << least(consecutive_failures, 5))))
        end,
        updated_at = current_timestamp
    where subscription_id = v_subscription_id;
    return true;
end;
$$ language plpgsql;
