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

    select status
    into v_status
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
        (v_status = 'needs_review' and p_next_status in ('rejected', 'extracting'))
        or (v_status in ('rejected', 'failed') and p_next_status = 'extracting')
    ) then
        raise exception 'invalid talent import item transition' using errcode = '22023';
    end if;

    update talent_import_items
    set
        status = p_next_status,
        lock_version = lock_version + 1,
        reviewed_at = current_timestamp,
        updated_at = current_timestamp
    where item_id = p_item_id;

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
