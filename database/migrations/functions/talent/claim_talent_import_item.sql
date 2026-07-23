-- Claims one queued item without blocking another worker's claim.
create or replace function claim_talent_import_item()
returns jsonb as $$
declare
    v_item jsonb;
begin
    with next_item as (
        select item_id
        from talent_import_items
        where status = 'queued'
        order by created_at, batch_id, position
        for update skip locked
        limit 1
    )
    update talent_import_items i
    set status = 'extracting', updated_at = current_timestamp
    from next_item n
    where i.item_id = n.item_id
    returning row_to_json(i) into v_item;

    if v_item is not null then
        update talent_import_batches
        set status = 'processing'
        where batch_id = (v_item->>'batch_id')::uuid
          and status = 'queued';
    end if;

    return v_item;
end;
$$ language plpgsql;
