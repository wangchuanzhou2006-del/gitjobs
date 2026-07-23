-- Removes expired encrypted raw material while preserving safe and audit data.
create or replace function cleanup_talent_raw_data(p_now timestamptz)
returns jsonb as $$
declare
    v_batches integer;
    v_items integer;
begin
    with due as (
        select batch_id
        from talent_import_batches
        where raw_deleted_at is null
          and (
              raw_delete_after <= p_now
              or (
                  raw_delete_after is null
                  and created_at <= p_now - interval '90 days'
              )
          )
        for update
    ),
    cleaned_items as (
        update talent_import_items i
        set
            raw_fragment_ciphertext = null,
            contact_candidates_ciphertext = null,
            updated_at = current_timestamp
        from due
        where i.batch_id = due.batch_id
          and (
              i.raw_fragment_ciphertext is not null
              or i.contact_candidates_ciphertext is not null
          )
        returning i.item_id
    )
    select count(*) into v_items from cleaned_items;

    update talent_import_batches
    set
        raw_text_ciphertext = null,
        raw_deleted_at = p_now
    where raw_deleted_at is null
      and (
          raw_delete_after <= p_now
          or (
              raw_delete_after is null
              and created_at <= p_now - interval '90 days'
          )
      );
    get diagnostics v_batches = row_count;

    return jsonb_build_object(
        'batches_cleaned', v_batches,
        'items_cleaned', v_items,
        'cleaned_at', p_now
    );
end;
$$ language plpgsql;
