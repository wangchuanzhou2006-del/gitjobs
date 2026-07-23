-- Saves editable safe review fields with optimistic concurrency.
create or replace function update_talent_review_item(
    p_item_id uuid,
    p_lock_version integer,
    p_updates jsonb,
    p_actor_user_id uuid
)
returns table(item_id uuid, lock_version integer) as $$
declare
    v_item_id uuid;
    v_lock_version integer;
begin
    if p_updates is null or jsonb_typeof(p_updates) <> 'object' then
        raise exception 'talent review update must be a JSON object' using errcode = '22023';
    end if;
    if jsonb_typeof(coalesce(p_updates->'extraction', '{}'::jsonb)) <> 'object' then
        raise exception 'talent review extraction must be a JSON object' using errcode = '22023';
    end if;
    if coalesce(p_updates->'extraction', '{}'::jsonb) ? 'contacts' then
        raise exception 'complete contacts are forbidden in safe talent extraction' using errcode = '22023';
    end if;
    if p_updates ? 'contact_masked'
       and jsonb_typeof(p_updates->'contact_masked') <> 'array' then
        raise exception 'talent review masked contacts must be a JSON array' using errcode = '22023';
    end if;
    if p_updates ? 'validation_errors'
       and jsonb_typeof(p_updates->'validation_errors') <> 'array' then
        raise exception 'talent review validation errors must be a JSON array' using errcode = '22023';
    end if;
    if p_updates ? 'risk_reasons'
       and jsonb_typeof(p_updates->'risk_reasons') <> 'array' then
        raise exception 'talent review risk reasons must be a JSON array' using errcode = '22023';
    end if;
    if p_updates ? 'risk_reasons'
       and exists (
           select 1
           from jsonb_array_elements(p_updates->'risk_reasons') as reasons(reason)
           where jsonb_typeof(reason) <> 'object'
              or (
                  reason ? 'hard_block'
                  and jsonb_typeof(reason->'hard_block') <> 'boolean'
              )
              or (
                  reason ? 'hardBlock'
                  and jsonb_typeof(reason->'hardBlock') <> 'boolean'
              )
       ) then
        raise exception 'talent review risk reasons contain an invalid entry'
            using errcode = '22023';
    end if;
    if p_updates ? 'duplicate_candidates'
       and jsonb_typeof(p_updates->'duplicate_candidates') <> 'array' then
        raise exception 'talent review duplicate candidates must be a JSON array' using errcode = '22023';
    end if;
    if p_updates ? 'duplicate_candidates'
       and exists (
           select 1
           from jsonb_array_elements(p_updates->'duplicate_candidates') as candidates(candidate)
           where jsonb_typeof(candidate) <> 'object'
              or (
                  candidate ? 'high_similarity'
                  and jsonb_typeof(candidate->'high_similarity') <> 'boolean'
              )
              or (
                  candidate ? 'highSimilarity'
                  and jsonb_typeof(candidate->'highSimilarity') <> 'boolean'
              )
              or (
                  candidate ? 'score'
                  and jsonb_typeof(candidate->'score') <> 'number'
              )
       ) then
        raise exception 'talent review duplicate candidates contain an invalid entry'
            using errcode = '22023';
    end if;

    update talent_import_items as reviewed
    set
        kind = coalesce(nullif(p_updates->>'kind', ''), reviewed.kind),
        extraction = (
            coalesce(reviewed.extraction, '{}'::jsonb)
            || coalesce(p_updates->'extraction', '{}'::jsonb)
        ) - 'contacts',
        extraction_confidence = coalesce(
            nullif(p_updates->>'extraction_confidence', '')::numeric,
            reviewed.extraction_confidence
        ),
        contact_masked = coalesce(p_updates->'contact_masked', reviewed.contact_masked),
        validation_errors = coalesce(p_updates->'validation_errors', reviewed.validation_errors),
        risk_score = coalesce(nullif(p_updates->>'risk_score', '')::numeric, reviewed.risk_score),
        risk_level = coalesce(nullif(p_updates->>'risk_level', ''), reviewed.risk_level),
        risk_reasons = coalesce(p_updates->'risk_reasons', reviewed.risk_reasons),
        duplicate_candidates = coalesce(
            p_updates->'duplicate_candidates',
            reviewed.duplicate_candidates
        ),
        lock_version = reviewed.lock_version + 1,
        reviewed_at = current_timestamp,
        updated_at = current_timestamp
    where reviewed.item_id = p_item_id
      and reviewed.lock_version = p_lock_version
      and reviewed.status = 'needs_review'
    returning reviewed.item_id, reviewed.lock_version
    into v_item_id, v_lock_version;

    if v_item_id is null then
        return;
    end if;

    insert into talent_review_events (
        item_id,
        actor_type,
        actor_user_id,
        action,
        before_snapshot,
        after_snapshot
    )
    values (
        v_item_id,
        'moderator',
        p_actor_user_id,
        'review_updated',
        jsonb_build_object('lock_version', p_lock_version),
        jsonb_build_object('lock_version', v_lock_version)
    );

    item_id := v_item_id;
    lock_version := v_lock_version;
    return next;
end;
$$ language plpgsql;
