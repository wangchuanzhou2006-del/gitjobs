-- Returns one job only when the caller owns the containing project.
create or replace function get_tender_analysis_job(p_user_id uuid, p_job_id uuid)
returns jsonb as $$
declare
    v_job jsonb;
begin
    select to_jsonb(job_row) into v_job
    from (
        select
            job.job_id,
            document.document_id,
            document.project_id,
            project.title as project_title,
            job.status,
            job.error_code,
            job.error_message,
            job.model_provider,
            job.model_name,
            job.prompt_version,
            job.attempt_count,
            job.created_at,
            job.updated_at,
            job.completed_at
        from tender_analysis_jobs job
        join tender_documents document on document.document_id = job.document_id
        join tender_projects project on project.project_id = document.project_id
        where job.job_id = p_job_id
          and project.user_id = p_user_id
    ) job_row;

    return v_job;
end;
$$ language plpgsql;
