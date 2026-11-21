-- Lists only the caller's analysis jobs, including report links when ready.
create or replace function list_tender_jobs(p_user_id uuid)
returns jsonb as $$
declare
    v_jobs jsonb;
begin
    select coalesce(jsonb_agg(to_jsonb(job_row) order by job_row.created_at desc, job_row.job_id desc), '[]'::jsonb)
    into v_jobs
    from (
        select job.job_id, document.document_id, document.project_id,
               project.title as project_title, job.status, job.error_code,
               job.model_provider, job.model_name, job.prompt_version,
               job.attempt_count, job.created_at, job.updated_at,
               job.completed_at, report.report_id
        from tender_analysis_jobs job
        join tender_documents document on document.document_id = job.document_id
        join tender_projects project on project.project_id = document.project_id
        left join tender_analysis_reports report on report.job_id = job.job_id
        where project.user_id = p_user_id
    ) job_row;
    return v_jobs;
end;
$$ language plpgsql;
