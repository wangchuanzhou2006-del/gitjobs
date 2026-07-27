-- Updates source trust and records Moderator confirmation for trusted sources.
drop function if exists set_talent_source_status(uuid, text, uuid, boolean);

create or replace function set_talent_source_status(
    p_source_id uuid,
    p_trust_status text,
    p_actor_user_id uuid,
    p_auto_publish_enabled boolean,
    p_expected_updated_at timestamptz
)
returns jsonb as $$
declare
    v_status jsonb;
begin
    if p_trust_status not in ('new', 'trusted', 'watchlist', 'blocked') then
        raise exception 'invalid talent source trust status' using errcode = '22023';
    end if;
    if p_trust_status = 'trusted' and p_actor_user_id is null then
        raise exception 'trusted talent source requires a Moderator confirmer'
            using errcode = '22023';
    end if;

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
        updated_at = greatest(clock_timestamp(), updated_at + interval '1 microsecond')
    where updated_source.source_id = p_source_id
      and updated_source.updated_at = p_expected_updated_at
    returning jsonb_build_object(
        'source_id', updated_source.source_id,
        'trust_status', updated_source.trust_status,
        'auto_publish_enabled', updated_source.auto_publish_enabled,
        'confirmed_by', updated_source.confirmed_by,
        'confirmed_at', updated_source.confirmed_at,
        'updated_at', updated_source.updated_at
    )
    into v_status;

    if found then
        return jsonb_build_object('outcome', 'updated', 'resource', v_status);
    end if;
    if exists (select 1 from talent_sources where source_id = p_source_id) then
        return jsonb_build_object('outcome', 'conflict');
    end if;
    return jsonb_build_object('outcome', 'not_found');
end;
$$ language plpgsql;
