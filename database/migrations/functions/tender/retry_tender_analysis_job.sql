-- An owned failed job can be retried without re-uploading the private PDF.
-- The document row serializes concurrent retries with ordinary uploads.
create or replace function retry_tender_analysis_job(p_user_id uuid, p_failed_job_id uuid)
returns uuid as $$
declare
    v_document_id uuid;
    v_config_hash text;
    v_model_provider text;
    v_model_name text;
    v_prompt_version text;
    v_job_id uuid;
begin
    perform set_config('lock_timeout', '10s', true);

    select job.document_id, job.analysis_config_hash, job.model_provider,
           job.model_name, job.prompt_version
    into v_document_id, v_config_hash, v_model_provider, v_model_name, v_prompt_version
    from tender_analysis_jobs job
    join tender_documents document on document.document_id = job.document_id
    join tender_projects project on project.project_id = document.project_id
    where job.job_id = p_failed_job_id
      and job.status = 'failed'
      and project.user_id = p_user_id;

    if not found then
        return null;
    end if;

    perform 1 from tender_documents where document_id = v_document_id for update;

    select job_id into v_job_id
    from tender_analysis_jobs
    where document_id = v_document_id
      and analysis_config_hash = v_config_hash
      and status <> 'failed'
    order by created_at
    limit 1;
    if v_job_id is not null then
        return v_job_id;
    end if;

    insert into tender_analysis_jobs (
        document_id, analysis_config_hash, model_provider, model_name, prompt_version
    ) values (
        v_document_id, v_config_hash, v_model_provider, v_model_name, v_prompt_version
    ) returning job_id into v_job_id;
    return v_job_id;
end;
$$ language plpgsql;
