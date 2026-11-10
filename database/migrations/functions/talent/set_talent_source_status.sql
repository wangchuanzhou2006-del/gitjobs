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
    v_before talent_sources%rowtype;
    v_after talent_sources%rowtype;
    v_effective_auto_publish boolean;
begin
    if p_trust_status not in ('new', 'trusted', 'watchlist', 'blocked') then
        raise exception 'invalid talent source trust status' using errcode = '22023';
    end if;
    if p_trust_status = 'trusted' and p_actor_user_id is null then
        raise exception 'trusted talent source requires a Moderator confirmer'
            using errcode = '22023';
    end if;
    if p_actor_user_id is null then
        raise exception 'talent source policy requires a Moderator actor'
            using errcode = '22023';
    end if;

    select *
    into v_before
    from talent_sources
    where source_id = p_source_id
    for update;

    if not found then
        return jsonb_build_object('outcome', 'not_found');
    end if;
    if v_before.updated_at <> p_expected_updated_at then
        return jsonb_build_object('outcome', 'conflict');
    end if;

    v_effective_auto_publish := case
        when p_trust_status in ('watchlist', 'blocked', 'new') then false
        else coalesce(p_auto_publish_enabled, false)
    end;

    if v_before.trust_status = p_trust_status
       and v_before.auto_publish_enabled = v_effective_auto_publish
       and (
           p_trust_status <> 'trusted'
           or v_before.confirmed_by = p_actor_user_id
       ) then
        return jsonb_build_object(
            'outcome', 'updated',
            'resource', jsonb_build_object(
                'source_id', v_before.source_id,
                'trust_status', v_before.trust_status,
                'auto_publish_enabled', v_before.auto_publish_enabled,
                'confirmed_by', v_before.confirmed_by,
                'confirmed_at', v_before.confirmed_at,
                'updated_at', v_before.updated_at
            )
        );
    end if;


    update talent_sources as updated_source
    set
        trust_status = p_trust_status,
        auto_publish_enabled = v_effective_auto_publish,
        confirmed_by = case
            when p_trust_status = 'trusted' then p_actor_user_id
            else updated_source.confirmed_by
        end,
        confirmed_at = case
            when p_trust_status = 'trusted' then clock_timestamp()
            else updated_source.confirmed_at
        end,
        updated_at = greatest(clock_timestamp(), updated_source.updated_at + interval '1 microsecond')
    where updated_source.source_id = p_source_id
    returning updated_source.* into v_after;

    insert into talent_review_events (
        resource_scope,
        source_id,
        actor_type,
        actor_user_id,
        action,
        reason,
        before_snapshot,
        after_snapshot,
        rule_version,
        policy_version,
        created_at
    )
    values (
        'source',
        v_after.source_id,
        'moderator',
        p_actor_user_id,
        'source_policy_changed',
        case
            when v_before.trust_status = p_trust_status
                 and v_before.auto_publish_enabled <> v_effective_auto_publish
                then case when v_effective_auto_publish
                    then 'source_auto_publish_enabled'
                    else 'source_auto_publish_paused'
                end
            when p_trust_status = 'trusted' and v_before.trust_status = 'trusted'
                then 'source_reconfirmed'
            when p_trust_status = 'trusted' then 'source_trusted'
            when p_trust_status = 'watchlist' then 'source_watchlisted'
            when p_trust_status = 'blocked' then 'source_blocked'
            else 'source_reset'
        end,
        jsonb_build_object(
            'trust_status', v_before.trust_status,
            'auto_publish_enabled', v_before.auto_publish_enabled,
            'confirmed_by', v_before.confirmed_by,
            'confirmed_at', v_before.confirmed_at
        ),
        jsonb_build_object(
            'trust_status', v_after.trust_status,
            'auto_publish_enabled', v_after.auto_publish_enabled,
            'confirmed_by', v_after.confirmed_by,
            'confirmed_at', v_after.confirmed_at
        ),
        'talent-risk-v1',
        'talent-auto-publish-v1',
        clock_timestamp()
    );

    return jsonb_build_object(
        'outcome', 'updated',
        'resource', jsonb_build_object(
            'source_id', v_after.source_id,
            'trust_status', v_after.trust_status,
            'auto_publish_enabled', v_after.auto_publish_enabled,
            'confirmed_by', v_after.confirmed_by,
            'confirmed_at', v_after.confirmed_at,
            'updated_at', v_after.updated_at
        )
    );
end;
$$ language plpgsql;
