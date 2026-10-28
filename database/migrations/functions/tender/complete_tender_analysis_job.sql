-- Validates a tender-analysis.v1 result, stores the report, and completes the
-- job in a single transaction. Only an unexpired, matching claim may write.
create or replace function complete_tender_analysis_job(p_completion jsonb)
returns uuid as $$
declare
    v_report_id uuid;
    v_status text;
    v_claim_token uuid;
    v_claim_version bigint;
    v_claim_expires_at timestamptz;
    v_document_id uuid;
    v_config_hash text;
begin
    if p_completion is null or jsonb_typeof(p_completion) <> 'object' then
        raise exception 'tender completion must be a JSON object' using errcode = '22023';
    end if;
    if jsonb_typeof(p_completion->'result') <> 'object' then
        raise exception 'tender analysis result must be a JSON object' using errcode = '22023';
    end if;
    if not (p_completion->'result' ?& array[
        'schema_version',
        'project_info',
        'qualifications',
        'risks',
        'required_materials',
        'unknowns',
        'analysis_meta'
    ]) then
        raise exception 'tender analysis result is missing top-level fields'
            using errcode = '22023';
    end if;
    if p_completion->'result'->>'schema_version' <> 'tender-analysis.v1' then
        raise exception 'tender analysis result has an invalid version' using errcode = '22023';
    end if;
    if p_completion->>'status' not in ('completed', 'partial') then
        raise exception 'invalid tender analysis completion status' using errcode = '22023';
    end if;

    select status, claim_token, claim_version, claim_expires_at, document_id, analysis_config_hash
    into
        v_status,
        v_claim_token,
        v_claim_version,
        v_claim_expires_at,
        v_document_id,
        v_config_hash
    from tender_analysis_jobs
    where job_id = (p_completion->>'job_id')::uuid
    for update;

    if not found then
        raise exception 'tender analysis job not found';
    end if;
    if v_status <> 'analyzing' then
        raise exception 'tender analysis job is not analyzing';
    end if;
    if v_claim_token is distinct from (p_completion->>'claim_token')::uuid
       or v_claim_version is distinct from (p_completion->>'claim_version')::bigint
       or v_claim_expires_at is null
       or v_claim_expires_at <= current_timestamp then
        raise exception 'tender analysis claim is stale or expired' using errcode = '40001';
    end if;

    insert into tender_analysis_reports (
        job_id, document_id, analysis_config_hash, result, contract_version
    )
    values (
        (p_completion->>'job_id')::uuid,
        v_document_id,
        v_config_hash,
        p_completion->'result',
        'tender-analysis.v1'
    )
    returning report_id into v_report_id;

    update tender_analysis_jobs
    set status = (p_completion->>'status'),
        claim_token = null,
        claim_expires_at = null,
        error_code = null,
        error_message = null,
        completed_at = current_timestamp,
        updated_at = current_timestamp
    where job_id = (p_completion->>'job_id')::uuid;

    return v_report_id;
end;
$$ language plpgsql;
