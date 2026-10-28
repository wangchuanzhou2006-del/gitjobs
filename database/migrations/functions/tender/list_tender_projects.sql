-- Lists projects owned by the caller.
create or replace function list_tender_projects(p_user_id uuid)
returns jsonb as $$
declare
    v_projects jsonb;
begin
    select coalesce(jsonb_agg(to_jsonb(project) order by project.created_at), '[]'::jsonb)
    into v_projects
    from (
        select project_id, title, source_url, created_at, updated_at
        from tender_projects
        where user_id = p_user_id
    ) project;

    return v_projects;
end;
$$ language plpgsql;
