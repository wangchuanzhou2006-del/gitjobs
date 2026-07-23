-- Withdraws a formal record and records a contact-free review event when linked.
create or replace function withdraw_talent_record(
    p_record_id uuid,
    p_actor_user_id uuid,
    p_reason text
)
returns boolean as $$
declare
    v_source_id uuid;
    v_item_id uuid;
begin
    update talent_records
    set
        status = 'withdrawn',
        withdrawn_at = coalesce(withdrawn_at, current_timestamp),
        updated_at = current_timestamp
    where record_id = p_record_id
      and status <> 'withdrawn'
    returning source_id into v_source_id;

    if not found then
        return exists (
            select 1 from talent_records
            where record_id = p_record_id and status = 'withdrawn'
        );
    end if;

    select item_id
    into v_item_id
    from talent_import_items
    where published_record_id = p_record_id;

    if v_item_id is not null then
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
            v_item_id,
            p_record_id,
            'moderator',
            p_actor_user_id,
            'withdrawn',
            p_reason,
            jsonb_build_object('record_status', 'published'),
            jsonb_build_object('record_status', 'withdrawn')
        );
    end if;

    if v_source_id is not null then
        update talent_sources
        set withdrawn_count = withdrawn_count + 1, updated_at = current_timestamp
        where source_id = v_source_id;
    end if;

    return true;
end;
$$ language plpgsql;
