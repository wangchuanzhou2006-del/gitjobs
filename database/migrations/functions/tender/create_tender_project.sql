-- Creates a user-owned tender project and returns its ID.
create or replace function create_tender_project(p_project jsonb)
returns uuid as $$
declare
    v_project_id uuid;
begin
    if p_project is null or jsonb_typeof(p_project) <> 'object' then
        raise exception 'tender project must be a JSON object' using errcode = '22023';
    end if;
    if jsonb_typeof(p_project->'user_id') <> 'string'
       or (p_project->>'user_id')
           !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$' then
        raise exception 'invalid tender project user' using errcode = '22023';
    end if;
    if jsonb_typeof(p_project->'title') <> 'string'
       or nullif(trim(p_project->>'title'), '') is null then
        raise exception 'invalid tender project title' using errcode = '22023';
    end if;
    if p_project ? 'source_url'
       and jsonb_typeof(p_project->'source_url') not in ('string', 'null') then
        raise exception 'invalid tender project source URL' using errcode = '22023';
    end if;

    insert into tender_projects (user_id, title, source_url)
    values (
        (p_project->>'user_id')::uuid,
        trim(p_project->>'title'),
        nullif(p_project->>'source_url', '')
    )
    returning project_id into v_project_id;

    return v_project_id;
end;
$$ language plpgsql;
