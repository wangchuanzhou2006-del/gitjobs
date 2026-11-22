-- Lists projects owned by the caller.
create or replace function list_tender_projects(p_user_id uuid)
returns jsonb as $$
declare
    v_projects jsonb;
begin
    select coalesce(jsonb_agg(to_jsonb(project) order by project.created_at), '[]'::jsonb)
    into v_projects
    from (
        select project.project_id, project.title, project.source_url,
               project.created_at, project.updated_at,
               latest.job_id as latest_job_id, latest.status as latest_job_status,
               latest.report_id as latest_report_id
        from tender_projects project
        left join lateral (
            select job.job_id, job.status, report.report_id
            from tender_documents document
            join tender_analysis_jobs job on job.document_id = document.document_id
            left join tender_analysis_reports report on report.job_id = job.job_id
            where document.project_id = project.project_id
            order by job.created_at desc, job.job_id desc
            limit 1
        ) latest on true
        where project.user_id = p_user_id
    ) project;

    return v_projects;
end;
$$ language plpgsql;
