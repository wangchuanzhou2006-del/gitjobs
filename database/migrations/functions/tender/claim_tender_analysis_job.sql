-- Claims one queued job, or one in-flight job whose lease expired, with
-- atomic FOR UPDATE SKIP LOCKED fencing and a fresh lease credential. The
-- returned JSON includes the document storage key so the worker can open it.
create or replace function claim_tender_analysis_job()
returns jsonb as $$
declare
    v_job_id uuid;
    v_job jsonb;
    v_claim_lease constant interval := interval '10 minutes';
begin
    with next_job as (
        select job_id
        from tender_analysis_jobs
        where status = 'queued'
           or (
               status in ('parsing', 'analyzing')
               and (claim_expires_at is null or claim_expires_at <= current_timestamp)
           )
        order by
            case when status = 'queued' then 0 else 1 end,
            created_at
        for update skip locked
        limit 1
    )
    update tender_analysis_jobs job
    set
        status = case when job.status = 'queued' then 'parsing' else job.status end,
        claim_token = gen_random_uuid(),
        claim_version = claim_version + 1,
        claim_expires_at = current_timestamp + v_claim_lease,
        attempt_count = attempt_count + 1,
        updated_at = current_timestamp
    from next_job next
    where job.job_id = next.job_id
    returning job.job_id into v_job_id;

    if v_job_id is null then
        return null;
    end if;

    select to_jsonb(job_row) into v_job
    from (
        select
            job.job_id,
            job.document_id,
            job.status,
            job.claim_token,
            job.claim_version,
            job.claim_expires_at,
            job.attempt_count,
            job.model_provider,
            job.model_name,
            job.prompt_version,
            document.storage_key,
            document.sha256
        from tender_analysis_jobs job
        join tender_documents document on document.document_id = job.document_id
        where job.job_id = v_job_id
    ) job_row;

    return v_job;
end;
$$ language plpgsql;
