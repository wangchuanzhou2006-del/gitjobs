-- Marks extraction as failed and advances batch counters atomically.
create or replace function fail_talent_import_item(p_failure jsonb)
returns void as $$
declare
    v_batch_id uuid;
    v_status text;
    v_claim_token uuid;
    v_claim_version bigint;
    v_claim_expires_at timestamptz;
    v_after talent_import_items%rowtype;
begin
    select
        batch_id,
        status,
        claim_token,
        claim_version,
        claim_expires_at
    into
        v_batch_id,
        v_status,
        v_claim_token,
        v_claim_version,
        v_claim_expires_at
    from talent_import_items
    where item_id = (p_failure->>'item_id')::uuid
    for update;

    if not found or v_status <> 'extracting' then
        raise exception 'talent import item is not extracting';
    end if;
    if v_claim_token is distinct from (p_failure->>'claim_token')::uuid
       or v_claim_version is distinct from (p_failure->>'claim_version')::bigint
       or v_claim_expires_at is null
       or v_claim_expires_at <= current_timestamp then
        raise exception 'talent import claim is stale or expired' using errcode = '40001';
    end if;

    update talent_import_items
    set
        status = 'failed',
        validation_errors = coalesce(p_failure->'validation_errors', '[]'::jsonb),
        claim_token = null,
        claim_expires_at = null,
        processed_at = current_timestamp,
        updated_at = current_timestamp
    where item_id = (p_failure->>'item_id')::uuid
    returning * into v_after;

    update talent_import_batches
    set
        processed_count = processed_count + 1,
        failed_count = failed_count + 1,
        status = case
            when processed_count + 1 = item_count and failed_count + 1 = item_count then 'failed'
            when processed_count + 1 = item_count then 'partially_failed'
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

    insert into talent_review_events (
        resource_scope,
        item_id,
        actor_type,
        action,
        reason,
        before_snapshot,
        after_snapshot,
        model_provider,
        model_name,
        prompt_version,
        rule_version,
        policy_version,
        created_at
    )
    select
        'item',
        v_after.item_id,
        'system',
        'processing_failed',
        'processor_failed',
        jsonb_build_object(
            'status', v_status,
            'claim_version', v_claim_version
        ),
        jsonb_build_object(
            'status', v_after.status,
            'validation_error_count', jsonb_array_length(
                coalesce(v_after.validation_errors, '[]'::jsonb)
            )
        ),
        batch.model_provider,
        batch.model_name,
        batch.prompt_version,
        'talent-risk-v1',
        'talent-auto-publish-v1',
        clock_timestamp()
    from talent_import_batches batch
    where batch.batch_id = v_batch_id;
end;
$$ language plpgsql;
