-- Completes extraction and advances batch counters in the same transaction.
create or replace function complete_talent_import_item(p_completion jsonb)
returns void as $$
declare
    v_batch_id uuid;
    v_status text;
begin
    v_status := p_completion->>'status';
    if v_status not in ('needs_review', 'auto_published') then
        raise exception 'invalid completed talent import item status' using errcode = '22023';
    end if;
    if jsonb_typeof(coalesce(p_completion->'extraction', 'null'::jsonb)) <> 'object' then
        raise exception 'talent extraction must be a JSON object' using errcode = '22023';
    end if;
    if nullif(p_completion->>'contact_candidates_ciphertext', '') is not null
       and (p_completion->>'contact_candidates_ciphertext') !~ '^(?:[0-9a-fA-F]{2})+$' then
        raise exception 'invalid encrypted talent contact payload' using errcode = '22023';
    end if;

    update talent_import_items
    set
        status = v_status,
        extraction = (p_completion->'extraction') - 'contacts',
        extraction_confidence = nullif(p_completion->>'extraction_confidence', '')::numeric,
        contact_candidates_ciphertext = nullif(p_completion->>'contact_candidates_ciphertext', ''),
        contact_hashes = coalesce(
            array(select jsonb_array_elements_text(coalesce(p_completion->'contact_hashes', '[]'::jsonb))),
            '{}'::text[]
        ),
        contact_masked = coalesce(p_completion->'contact_masked', '[]'::jsonb),
        normalized_fingerprint = nullif(p_completion->>'normalized_fingerprint', ''),
        validation_errors = coalesce(p_completion->'validation_errors', '[]'::jsonb),
        risk_score = nullif(p_completion->>'risk_score', '')::numeric,
        risk_level = nullif(p_completion->>'risk_level', ''),
        risk_reasons = coalesce(p_completion->'risk_reasons', '[]'::jsonb),
        duplicate_candidates = coalesce(p_completion->'duplicate_candidates', '[]'::jsonb),
        processed_at = current_timestamp,
        updated_at = current_timestamp
    where item_id = (p_completion->>'item_id')::uuid
      and status = 'extracting'
    returning batch_id into v_batch_id;

    if v_batch_id is null then
        raise exception 'talent import item is not extracting';
    end if;

    update talent_import_batches
    set
        processed_count = processed_count + 1,
        status = case
            when processed_count + 1 = item_count and failed_count > 0 then 'partially_failed'
            when processed_count + 1 = item_count then 'completed'
            else 'processing'
        end,
        completed_at = case
            when processed_count + 1 = item_count then current_timestamp
            else null
        end
    where batch_id = v_batch_id
      and processed_count < item_count;

    if not found then
        raise exception 'talent import batch counters are invalid';
    end if;
end;
$$ language plpgsql;
