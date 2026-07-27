-- Sets the singleton emergency auto-publication switch.
drop function if exists set_talent_auto_publish(boolean, uuid);

create or replace function set_talent_auto_publish(
    p_enabled boolean,
    p_actor_user_id uuid,
    p_expected_updated_at timestamptz
)
returns jsonb as $$
declare
    v_settings jsonb;
begin
    update talent_runtime_settings
    set
        auto_publish_enabled = p_enabled,
        updated_by = p_actor_user_id,
        updated_at = greatest(clock_timestamp(), updated_at + interval '1 microsecond')
    where singleton
      and updated_at = p_expected_updated_at
    returning jsonb_build_object(
        'auto_publish_enabled', auto_publish_enabled,
        'updated_by', updated_by,
        'updated_at', updated_at
    )
    into v_settings;

    if found then
        return jsonb_build_object('outcome', 'updated', 'resource', v_settings);
    end if;
    return jsonb_build_object('outcome', 'conflict');
end;
$$ language plpgsql;
