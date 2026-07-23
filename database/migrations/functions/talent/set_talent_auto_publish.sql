-- Sets the singleton emergency auto-publication switch.
create or replace function set_talent_auto_publish(
    p_enabled boolean,
    p_actor_user_id uuid
)
returns jsonb as $$
    update talent_runtime_settings
    set
        auto_publish_enabled = p_enabled,
        updated_by = p_actor_user_id,
        updated_at = current_timestamp
    where singleton
    returning jsonb_build_object(
        'auto_publish_enabled', auto_publish_enabled,
        'updated_by', updated_by,
        'updated_at', updated_at
    );
$$ language sql;
