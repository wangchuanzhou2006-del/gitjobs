-- Atomically persists an encrypted import batch and all encrypted fragments.
create or replace function create_talent_import_batch(p_import jsonb)
returns uuid as $$
declare
    v_batch_id uuid;
    v_items jsonb;
begin
    if p_import is null or jsonb_typeof(p_import) <> 'object' then
        raise exception 'talent import must be a JSON object' using errcode = '22023';
    end if;

    v_items := p_import->'items';
    if v_items is null
       or jsonb_typeof(v_items) <> 'array'
       or jsonb_array_length(v_items) not between 1 and 500 then
        raise exception 'talent import must contain between 1 and 500 items' using errcode = '22023';
    end if;
    if coalesce(p_import->>'raw_text_ciphertext', '') !~ '^(?:[0-9a-fA-F]{2})+$' then
        raise exception 'invalid encrypted talent import payload' using errcode = '22023';
    end if;

    insert into talent_import_batches (
        source_id,
        imported_by,
        raw_text_ciphertext,
        encryption_key_version,
        model_provider,
        model_name,
        prompt_version,
        item_count,
        raw_delete_after
    )
    values (
        (p_import->>'source_id')::uuid,
        nullif(p_import->>'imported_by', '')::uuid,
        lower(p_import->>'raw_text_ciphertext'),
        (p_import->>'encryption_key_version')::integer,
        nullif(p_import->>'model_provider', ''),
        nullif(p_import->>'model_name', ''),
        nullif(p_import->>'prompt_version', ''),
        jsonb_array_length(v_items),
        nullif(p_import->>'raw_delete_after', '')::timestamptz
    )
    returning batch_id into v_batch_id;

    insert into talent_import_items (
        batch_id,
        position,
        raw_fragment_ciphertext,
        encryption_key_version,
        kind
    )
    select
        v_batch_id,
        (item->>'position')::integer,
        lower(item->>'raw_fragment_ciphertext'),
        (item->>'encryption_key_version')::integer,
        item->>'kind'
    from jsonb_array_elements(v_items) as fragments(item)
    where coalesce(item->>'raw_fragment_ciphertext', '') ~ '^(?:[0-9a-fA-F]{2})+$';

    if (select count(*) from talent_import_items where batch_id = v_batch_id) <> jsonb_array_length(v_items) then
        raise exception 'invalid encrypted talent fragment payload' using errcode = '22023';
    end if;

    return v_batch_id;
end;
$$ language plpgsql;
