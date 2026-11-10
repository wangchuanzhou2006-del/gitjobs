-- Upserts a document without overwriting an existing one, and reuses an
-- existing reusable (non-failed) job — active, partial, or completed — for the
-- same config. A failed job is left in place so a retry creates a fresh one.
--
-- Concurrency: the document row is locked with FOR UPDATE for the whole
-- read-then-insert section, so concurrent creates for the same file serialize
-- and the insert below can neither race a completion into a duplicate job nor
-- return an empty job_id. The reusable-config unique index remains a backstop.
create or replace function create_tender_document_job(p_input jsonb)
returns jsonb as $$
declare
    v_document_id uuid;
    v_document_created boolean;
    v_job_id uuid;
begin
    -- Bound the document row-lock wait (the FOR UPDATE and the document unique
    -- index below) so a stuck holder fails fast with 55P03 instead of blocking
    -- creates forever.
    perform set_config('lock_timeout', '10 seconds', true);

    if p_input is null or jsonb_typeof(p_input) <> 'object' then
        raise exception 'tender document job must be a JSON object' using errcode = '22023';
    end if;
    if jsonb_typeof(p_input->'project_id') <> 'string'
       or (p_input->>'project_id')
           !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$' then
        raise exception 'invalid tender document project' using errcode = '22023';
    end if;
    if jsonb_typeof(p_input->'sha256') <> 'string'
       or (p_input->>'sha256') !~ '^[0-9a-f]{64}$' then
        raise exception 'invalid tender document checksum' using errcode = '22023';
    end if;
    if jsonb_typeof(p_input->'analysis_config_hash') <> 'string'
       or nullif(p_input->>'analysis_config_hash', '') is null then
        raise exception 'invalid tender analysis config hash' using errcode = '22023';
    end if;

    if not exists (
        select 1
        from tender_projects project
        where project.project_id = (p_input->>'project_id')::uuid
          and project.user_id = (p_input->>'user_id')::uuid
    ) then
        raise exception 'tender project not found or not owned' using errcode = '42501';
    end if;

    insert into tender_documents (
        project_id, storage_key, sha256, size_bytes, filename
    )
    values (
        (p_input->>'project_id')::uuid,
        (p_input->>'storage_key'),
        (p_input->>'sha256'),
        (p_input->>'size_bytes')::bigint,
        nullif(p_input->>'filename', '')
    )
    on conflict (project_id, sha256) do nothing
    returning document_id into v_document_id;

    if v_document_id is null then
        select document_id into v_document_id
        from tender_documents
        where project_id = (p_input->>'project_id')::uuid
          and sha256 = (p_input->>'sha256')
        for update;
        v_document_created := false;
    else
        v_document_created := true;
    end if;

    select job_id into v_job_id
    from tender_analysis_jobs
    where document_id = v_document_id
      and analysis_config_hash = (p_input->>'analysis_config_hash')
      and status <> 'failed'
    order by created_at
    limit 1;

    if v_job_id is not null then
        return jsonb_build_object(
            'job_id', v_job_id,
            'created', false,
            'document_created', v_document_created
        );
    end if;

    insert into tender_analysis_jobs (
        document_id,
        analysis_config_hash,
        model_provider,
        model_name,
        prompt_version
    )
    values (
        v_document_id,
        (p_input->>'analysis_config_hash'),
        nullif(p_input->>'model_provider', ''),
        nullif(p_input->>'model_name', ''),
        nullif(p_input->>'prompt_version', '')
    )
    returning job_id into v_job_id;

    return jsonb_build_object(
        'job_id', v_job_id,
        'created', true,
        'document_created', v_document_created
    );
end;
$$ language plpgsql;
