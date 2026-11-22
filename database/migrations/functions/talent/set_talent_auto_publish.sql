-- Sets the singleton emergency auto-publication switch.
drop function if exists set_talent_auto_publish(boolean, uuid);

create or replace function set_talent_auto_publish(
    p_enabled boolean,
    p_actor_user_id uuid,
    p_expected_updated_at timestamptz
)
returns jsonb as $$
declare
    v_before talent_runtime_settings%rowtype;
    v_after talent_runtime_settings%rowtype;
begin
    if p_actor_user_id is null then
        raise exception 'talent runtime control requires a Moderator actor'
            using errcode = '22023';
    end if;

    select *
    into v_before
    from talent_runtime_settings
    where singleton
    for update;

    if v_before.updated_at <> p_expected_updated_at then
        return jsonb_build_object('outcome', 'conflict');
    end if;

    if v_before.auto_publish_enabled = p_enabled then
        return jsonb_build_object(
            'outcome', 'updated',
            'resource', jsonb_build_object(
                'auto_publish_enabled', v_before.auto_publish_enabled,
                'updated_by', v_before.updated_by,
                'updated_at', v_before.updated_at
            )
        );
    end if;

    update talent_runtime_settings as settings
    set
        auto_publish_enabled = p_enabled,
        updated_by = p_actor_user_id,
        updated_at = greatest(clock_timestamp(), settings.updated_at + interval '1 microsecond')
    where settings.singleton
    returning settings.* into v_after;

    insert into talent_review_events (
        resource_scope,
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
        'runtime',
        'moderator',
        p_actor_user_id,
        'runtime_auto_publish_changed',
        case when p_enabled then 'auto_publish_resumed' else 'auto_publish_paused' end,
        jsonb_build_object(
            'auto_publish_enabled', v_before.auto_publish_enabled,
            'updated_by', v_before.updated_by,
            'updated_at', v_before.updated_at
        ),
        jsonb_build_object(
            'auto_publish_enabled', v_after.auto_publish_enabled,
            'updated_by', v_after.updated_by,
            'updated_at', v_after.updated_at
        ),
        'talent-risk-v1',
        'talent-auto-publish-v1',
        clock_timestamp()
    );

    return jsonb_build_object(
        'outcome', 'updated',
        'resource', jsonb_build_object(
            'auto_publish_enabled', v_after.auto_publish_enabled,
            'updated_by', v_after.updated_by,
            'updated_at', v_after.updated_at
        )
    );
end;
$$ language plpgsql;
