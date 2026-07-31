create or replace function upsert_talent_source_subscription(p_subscription jsonb)
returns uuid as $$
declare
    v_id uuid;
    v_url text := trim(p_subscription->>'public_url');
    v_keywords text[];
begin
    if p_subscription is null or jsonb_typeof(p_subscription) <> 'object' then
        raise exception 'source subscription must be an object' using errcode = '22023';
    end if;
    if not exists (
        select 1 from "user"
        where user_id = nullif(p_subscription->>'actor_user_id', '')::uuid
          and moderator
    ) then
        raise exception 'source subscription update requires a moderator' using errcode = '42501';
    end if;
    if v_url !~ '^https://[^/?#[:space:]]+(?:[/?][^#[:space:]]*)?$' or v_url ~ '@' then
        raise exception 'source subscription requires a public HTTPS URL' using errcode = '22023';
    end if;
    select array_agg(keyword order by keyword) into v_keywords
    from (
        select distinct lower(trim(value)) keyword
        from jsonb_array_elements_text(p_subscription->'keywords') values_list(value)
        where trim(value) <> '' and length(trim(value)) <= 50
    ) normalized;
    if coalesce(cardinality(v_keywords), 0) not between 1 and 20 then
        raise exception 'source subscription requires 1 to 20 keywords' using errcode = '22023';
    end if;

    if nullif(p_subscription->>'subscription_id', '') is not null then
        update talent_source_subscriptions set
            public_url = v_url,
            keywords = v_keywords,
            interval_minutes = (p_subscription->>'interval_minutes')::integer,
            enabled = coalesce((p_subscription->>'enabled')::boolean, true),
            next_run_at = case when coalesce((p_subscription->>'enabled')::boolean, true) then least(next_run_at, current_timestamp) else next_run_at end,
            updated_at = current_timestamp
        where subscription_id = (p_subscription->>'subscription_id')::uuid
          and source_id = (p_subscription->>'source_id')::uuid
        returning subscription_id into v_id;
        if v_id is null then
            raise exception 'source subscription not found' using errcode = 'P0002';
        end if;
        return v_id;
    end if;

    insert into talent_source_subscriptions (
        subscription_id, source_id, public_url, keywords, interval_minutes, enabled, created_by
    ) values (
        coalesce(nullif(p_subscription->>'subscription_id', '')::uuid, gen_random_uuid()),
        (p_subscription->>'source_id')::uuid,
        v_url,
        v_keywords,
        (p_subscription->>'interval_minutes')::integer,
        coalesce((p_subscription->>'enabled')::boolean, true),
        nullif(p_subscription->>'actor_user_id', '')::uuid
    )
    on conflict (source_id, public_url) do update set
        keywords = excluded.keywords,
        interval_minutes = excluded.interval_minutes,
        enabled = excluded.enabled,
        next_run_at = case when excluded.enabled then least(talent_source_subscriptions.next_run_at, current_timestamp) else talent_source_subscriptions.next_run_at end,
        updated_at = current_timestamp
    returning subscription_id into v_id;
    return v_id;
end;
$$ language plpgsql;
