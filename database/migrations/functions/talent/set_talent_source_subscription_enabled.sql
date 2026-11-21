create or replace function set_talent_source_subscription_enabled(
    p_subscription_id uuid,
    p_enabled boolean,
    p_actor_user_id uuid
)
returns boolean as $$
declare
    v_updated integer;
begin
    if not exists (select 1 from "user" where user_id = p_actor_user_id and moderator) then
        raise exception 'talent source subscription update requires a moderator' using errcode = '42501';
    end if;
    update talent_source_subscriptions set
        enabled = p_enabled,
        claimed_at = case when p_enabled then claimed_at else null end,
        next_run_at = case when p_enabled then least(next_run_at, current_timestamp) else next_run_at end,
        updated_at = current_timestamp
    where subscription_id = p_subscription_id;
    get diagnostics v_updated = row_count;
    return v_updated > 0;
end;
$$ language plpgsql;
