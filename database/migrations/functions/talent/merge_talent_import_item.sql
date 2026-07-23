-- Marks a review item as a duplicate of an existing formal record atomically.
create or replace function merge_talent_import_item(
    p_item_id uuid,
    p_lock_version integer,
    p_target_record_id uuid,
    p_actor_user_id uuid,
    p_reason text
)
returns boolean as $$
declare
    v_item talent_import_items%rowtype;
begin
    select *
    into v_item
    from talent_import_items
    where item_id = p_item_id
    for update;

    if not found then
        raise exception 'talent import item not found';
    end if;
    if v_item.lock_version <> p_lock_version then
        return false;
    end if;
    if v_item.status <> 'needs_review' then
        raise exception 'talent import item is not awaiting review';
    end if;
    if not exists (
        select 1
        from talent_records
        where record_id = p_target_record_id
          and status <> 'withdrawn'
    ) then
        raise exception 'talent merge target not found';
    end if;

    update talent_import_items
    set
        status = 'merged',
        duplicate_candidates = coalesce(duplicate_candidates, '[]'::jsonb)
            || jsonb_build_array(
                jsonb_build_object(
                    'candidate_record_id', p_target_record_id,
                    'merge_target', true
                )
            ),
        lock_version = lock_version + 1,
        reviewed_at = current_timestamp,
        updated_at = current_timestamp
    where item_id = p_item_id;

    insert into talent_review_events (
        item_id,
        record_id,
        actor_type,
        actor_user_id,
        action,
        reason,
        before_snapshot,
        after_snapshot
    )
    values (
        p_item_id,
        p_target_record_id,
        'moderator',
        p_actor_user_id,
        'merged',
        p_reason,
        jsonb_build_object('status', v_item.status, 'lock_version', v_item.lock_version),
        jsonb_build_object(
            'status', 'merged',
            'target_record_id', p_target_record_id,
            'lock_version', v_item.lock_version + 1
        )
    );

    return true;
end;
$$ language plpgsql;
