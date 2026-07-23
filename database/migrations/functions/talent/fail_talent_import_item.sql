-- Marks extraction as failed and advances batch counters atomically.
create or replace function fail_talent_import_item(p_failure jsonb)
returns void as $$
declare
    v_batch_id uuid;
begin
    update talent_import_items
    set
        status = 'failed',
        validation_errors = coalesce(p_failure->'validation_errors', '[]'::jsonb),
        processed_at = current_timestamp,
        updated_at = current_timestamp
    where item_id = (p_failure->>'item_id')::uuid
      and status = 'extracting'
    returning batch_id into v_batch_id;

    if v_batch_id is null then
        raise exception 'talent import item is not extracting';
    end if;

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
end;
$$ language plpgsql;
