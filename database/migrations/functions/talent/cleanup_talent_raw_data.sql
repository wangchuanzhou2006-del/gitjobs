-- Removes expired encrypted raw material while preserving safe and audit data.
create or replace function cleanup_talent_raw_data(p_now timestamptz)
returns jsonb as $$
declare
    v_batches integer := 0;
    v_items integer := 0;
    v_item_count integer;
    v_due record;
begin
    for v_due in
        select
            batch.batch_id,
            batch.raw_text_ciphertext is not null as raw_text_present,
            (
                select count(*)::integer
                from talent_import_items item
                where item.batch_id = batch.batch_id
                  and (
                      item.raw_fragment_ciphertext is not null
                      or item.contact_candidates_ciphertext is not null
                  )
            ) as item_raw_payload_count
        from talent_import_batches batch
        where batch.raw_deleted_at is null
          and (
              batch.raw_delete_after <= p_now
              or batch.created_at <= p_now - interval '90 days'
          )
        for update of batch
    loop
        update talent_import_items i
        set
            raw_fragment_ciphertext = null,
            contact_candidates_ciphertext = null,
            updated_at = current_timestamp
        where i.batch_id = v_due.batch_id
          and (
              i.raw_fragment_ciphertext is not null
              or i.contact_candidates_ciphertext is not null
          );
        get diagnostics v_item_count = row_count;
        v_items := v_items + v_item_count;

        update talent_import_batches
        set
            raw_text_ciphertext = null,
            raw_deleted_at = p_now
        where batch_id = v_due.batch_id
          and raw_deleted_at is null;

        if found then
            v_batches := v_batches + 1;
            insert into talent_review_events (
                resource_scope,
                batch_id,
                actor_type,
                action,
                reason,
                before_snapshot,
                after_snapshot,
                rule_version,
                policy_version,
                created_at
            )
            values (
                'cleanup',
                v_due.batch_id,
                'system',
                'raw_data_cleaned',
                'retention_expired',
                jsonb_build_object(
                    'raw_text_present', v_due.raw_text_present,
                    'item_raw_payload_count', v_due.item_raw_payload_count
                ),
                jsonb_build_object(
                    'raw_text_present', false,
                    'item_raw_payload_count', 0
                ),
                'talent-risk-v1',
                'talent-retention-v1',
                clock_timestamp()
            );
        end if;
    end loop;

    return jsonb_build_object(
        'batches_cleaned', v_batches,
        'items_cleaned', v_items,
        'cleaned_at', p_now
    );
end;
$$ language plpgsql;
