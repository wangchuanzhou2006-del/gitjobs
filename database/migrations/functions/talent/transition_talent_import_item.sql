-- Applies non-publication review state transitions with optimistic concurrency.
create or replace function transition_talent_import_item(
    p_item_id uuid,
    p_lock_version integer,
    p_next_status text,
    p_actor_user_id uuid,
    p_reason text default null
)
returns boolean as $$
declare
    v_status text;
    v_batch_id uuid;
begin
    if p_reason is not null
       and p_reason not in (
           'duplicate',
           'rejected',
           'reprocess',
           'withdrawn',
           'moderator_correction',
           'policy_violation',
           'expired',
           'other'
       ) then
        raise exception 'invalid talent audit reason' using errcode = '22023';
    end if;

    select status, batch_id
    into v_status, v_batch_id
    from talent_import_items
    where item_id = p_item_id
    for update;

    if not found then
        raise exception 'talent import item not found';
    end if;
    if not exists (
        select 1
        from talent_import_items
        where item_id = p_item_id
          and lock_version = p_lock_version
    ) then
        return false;
    end if;
    if not (
        (v_status = 'needs_review' and p_next_status in ('rejected', 'queued'))
        or (v_status in ('rejected', 'failed') and p_next_status = 'queued')
    ) then
        raise exception 'invalid talent import item transition' using errcode = '22023';
    end if;

    update talent_import_items
    set
        status = p_next_status,
        lock_version = lock_version + 1,
        processed_at = case when p_next_status = 'queued' then null else processed_at end,
        reviewed_at = case when p_next_status = 'queued' then null else current_timestamp end,
        updated_at = current_timestamp
    where item_id = p_item_id;

    if p_next_status = 'queued' then
        update talent_import_batches
        set
            processed_count = processed_count - 1,
            failed_count = failed_count - case when v_status = 'failed' then 1 else 0 end,
            status = 'queued',
            completed_at = null
        where batch_id = v_batch_id
          and processed_count > 0
          and (v_status <> 'failed' or failed_count > 0);

        if not found then
            raise exception 'talent import batch counters are invalid';
        end if;
    end if;

    insert into talent_review_events (
        item_id,
        actor_type,
        actor_user_id,
        action,
        reason,
        before_snapshot,
        after_snapshot
    )
    values (
        p_item_id,
        'moderator',
        p_actor_user_id,
        p_next_status,
        p_reason,
        jsonb_build_object('status', v_status, 'lock_version', p_lock_version),
        jsonb_build_object('status', p_next_status, 'lock_version', p_lock_version + 1)
    );

    return true;
end;
$$ language plpgsql;
