-- Returns non-secret runtime publication controls.
create or replace function get_talent_runtime_settings()
returns jsonb as $$
    select jsonb_build_object(
        'auto_publish_enabled', auto_publish_enabled,
        'updated_by', updated_by,
        'updated_at', updated_at
    )
    from talent_runtime_settings
    where singleton;
$$ language sql stable;
