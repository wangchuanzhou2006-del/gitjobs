-- Advances an in-flight job after a valid, unexpired claim.
create or replace function transition_tender_analysis_job(p_transition jsonb)
returns void as $$
declare
    v_status text;
    v_claim_token uuid;
    v_claim_version bigint;
    v_claim_expires_at timestamptz;
begin
    if jsonb_typeof(p_transition->'job_id') <> 'string'
       or (p_transition->>'job_id')
           !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$' then
        raise exception 'invalid tender analysis job' using errcode = '22023';
    end if;

    select status, claim_token, claim_version, claim_expires_at
    into v_status, v_claim_token, v_claim_version, v_claim_expires_at
    from tender_analysis_jobs
    where job_id = (p_transition->>'job_id')::uuid
    for update;

    if not found then
        raise exception 'tender analysis job not found';
    end if;
    if v_claim_token is distinct from (p_transition->>'claim_token')::uuid
       or v_claim_version is distinct from (p_transition->>'claim_version')::bigint
       or v_claim_expires_at is null
       or v_claim_expires_at <= current_timestamp then
        raise exception 'tender analysis claim is stale or expired' using errcode = '40001';
    end if;

    if not (
        v_status = 'parsing' and (p_transition->>'next_status') = 'analyzing'
    ) then
        raise exception 'invalid tender analysis transition' using errcode = '22023';
    end if;

    update tender_analysis_jobs
    set status = (p_transition->>'next_status'),
        claim_expires_at = current_timestamp + interval '10 minutes',
        updated_at = current_timestamp
    where job_id = (p_transition->>'job_id')::uuid;
end;
$$ language plpgsql;
