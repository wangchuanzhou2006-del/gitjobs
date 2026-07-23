-- Creates a source or returns the existing case-insensitive source identity.
create or replace function upsert_talent_source(
    p_source_type text,
    p_name text,
    p_created_by uuid default null
)
returns uuid as $$
declare
    v_source_id uuid;
begin
    if p_source_type not in ('wechat_group', 'qq_group', 'web', 'other') then
        raise exception 'invalid talent source type' using errcode = '22023';
    end if;
    if nullif(btrim(p_name), '') is null then
        raise exception 'talent source name cannot be empty' using errcode = '22023';
    end if;

    insert into talent_sources (source_type, name, created_by)
    values (p_source_type, btrim(p_name), p_created_by)
    on conflict (source_type, lower(name))
    do update set updated_at = current_timestamp
    returning source_id into v_source_id;

    return v_source_id;
end;
$$ language plpgsql;
