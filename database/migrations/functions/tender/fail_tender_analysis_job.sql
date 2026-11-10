-- Fails an in-flight job. Only an approved error code is stored; caller text
-- is never persisted, so no arbitrary detail can leak through job queries.
create or replace function fail_tender_analysis_job(p_failure jsonb)
returns void as $$
declare
    v_status text;
    v_claim_token uuid;
    v_claim_version bigint;
    v_claim_expires_at timestamptz;
begin
    if p_failure->'error_code' is null
       or jsonb_typeof(p_failure->'error_code') <> 'string'
       or p_failure->>'error_code' not in (
           'invalid_pdf',
           'encrypted_pdf',
           'too_many_pages',
           'extracted_text_too_large',
           'ocr_required',
           'analysis_unavailable',
           'invalid_analysis_contract',
           'storage_unavailable'
       ) then
        raise exception 'invalid tender analysis error code' using errcode = '22023';
    end if;

    select status, claim_token, claim_version, claim_expires_at
    into v_status, v_claim_token, v_claim_version, v_claim_expires_at
    from tender_analysis_jobs
    where job_id = (p_failure->>'job_id')::uuid
    for update;

    if not found then
        raise exception 'tender analysis job not found';
    end if;
    if v_status not in ('parsing', 'analyzing') then
        raise exception 'tender analysis job is not in-flight';
    end if;
    if v_claim_token is distinct from (p_failure->>'claim_token')::uuid
       or v_claim_version is distinct from (p_failure->>'claim_version')::bigint
       or v_claim_expires_at is null
       or v_claim_expires_at <= current_timestamp then
        raise exception 'tender analysis claim is stale or expired' using errcode = '40001';
    end if;

    update tender_analysis_jobs
    set status = 'failed',
        claim_token = null,
        claim_expires_at = null,
        error_code = (p_failure->>'error_code'),
        updated_at = current_timestamp
    where job_id = (p_failure->>'job_id')::uuid;
end;
$$ language plpgsql;
