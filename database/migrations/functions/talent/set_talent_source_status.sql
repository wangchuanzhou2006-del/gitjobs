-- Updates source trust and records Moderator confirmation for trusted sources.
create or replace function set_talent_source_status(
    p_source_id uuid,
    p_trust_status text,
    p_actor_user_id uuid,
    p_auto_publish_enabled boolean
)
returns table(
    source_id uuid,
    trust_status text,
    auto_publish_enabled boolean,
    confirmed_by uuid,
    confirmed_at timestamptz
) as $$
begin
    if p_trust_status not in ('new', 'trusted', 'watchlist', 'blocked') then
        raise exception 'invalid talent source trust status' using errcode = '22023';
    end if;
    if p_trust_status = 'trusted' and p_actor_user_id is null then
        raise exception 'trusted talent source requires a Moderator confirmer'
            using errcode = '22023';
    end if;

    return query
    update talent_sources as updated_source
    set
        trust_status = p_trust_status,
        auto_publish_enabled = case
            when p_trust_status in ('watchlist', 'blocked', 'new') then false
            else coalesce(p_auto_publish_enabled, false)
        end,
        confirmed_by = case
            when p_trust_status = 'trusted' then p_actor_user_id
            else confirmed_by
        end,
        confirmed_at = case
            when p_trust_status = 'trusted' then current_timestamp
            else confirmed_at
        end,
        updated_at = current_timestamp
    where updated_source.source_id = p_source_id
    returning
        updated_source.source_id,
        updated_source.trust_status,
        updated_source.auto_publish_enabled,
        updated_source.confirmed_by,
        updated_source.confirmed_at;
end;
$$ language plpgsql;
