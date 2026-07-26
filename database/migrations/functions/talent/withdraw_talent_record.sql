-- Withdraws a formal record and records a contact-free review event when linked.
drop function if exists withdraw_talent_record(uuid, uuid, text);

create or replace function withdraw_talent_record(
    p_item_id uuid,
    p_record_id uuid,
    p_lock_version integer,
    p_actor_user_id uuid,
    p_reason text
)
returns integer as $$
declare
    v_item talent_import_items%rowtype;
    v_source_id uuid;
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

    select *
    into v_item
    from talent_import_items
    where item_id = p_item_id
    for update;

    if not found then
        raise exception 'talent import item not found';
    end if;
    if v_item.lock_version <> p_lock_version then
        return null;
    end if;
    if v_item.published_record_id is distinct from p_record_id
       or v_item.status not in ('published', 'auto_published') then
        raise exception 'talent withdrawal target does not match published item'
            using errcode = '22023';
    end if;

    update talent_records
    set
        status = 'withdrawn',
        withdrawn_at = coalesce(withdrawn_at, current_timestamp),
        updated_at = current_timestamp
    where record_id = p_record_id
      and status <> 'withdrawn'
    returning source_id into v_source_id;

    if not found then
        if exists (
            select 1 from talent_records
            where record_id = p_record_id and status = 'withdrawn'
        ) then
            return v_item.lock_version;
        end if;
        raise exception 'talent record not found';
    end if;

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
        p_record_id,
        'moderator',
        p_actor_user_id,
        'withdrawn',
        p_reason,
        jsonb_build_object(
            'record_status', 'published',
            'lock_version', v_item.lock_version
        ),
        jsonb_build_object(
            'record_status', 'withdrawn',
            'lock_version', v_item.lock_version + 1
        )
    );

    update talent_import_items
    set lock_version = lock_version + 1, updated_at = current_timestamp
    where item_id = p_item_id;

    if v_source_id is not null then
        update talent_sources
        set withdrawn_count = withdrawn_count + 1, updated_at = current_timestamp
        where source_id = v_source_id;
    end if;

    return v_item.lock_version + 1;
end;
$$ language plpgsql;
