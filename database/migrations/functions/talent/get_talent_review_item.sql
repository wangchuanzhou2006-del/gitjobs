-- Returns one private review detail. Ciphertext remains encrypted and is never logged or public.
create or replace function get_talent_review_item(p_item_id uuid)
returns jsonb as $$
    select to_jsonb(detail)
    from (
        select
            i.item_id,
            i.batch_id,
            i.position,
            i.raw_fragment_ciphertext,
            i.encryption_key_version,
            i.contact_candidates_ciphertext,
            coalesce(i.contact_hashes, '{}'::text[]) as contact_hashes,
            i.contact_masked,
            i.normalized_fingerprint,
            i.kind,
            i.status,
            i.extraction,
            i.extraction_confidence,
            i.validation_errors,
            i.risk_score,
            i.risk_level,
            i.risk_reasons,
            i.duplicate_candidates,
            i.published_record_id,
            i.lock_version,
            i.processed_at,
            i.reviewed_at,
            i.updated_at,
            b.model_provider,
            b.model_name,
            b.prompt_version,
            s.source_id,
            s.source_type,
            s.name as source_name,
            s.trust_status as source_trust_status,
            s.auto_publish_enabled as source_auto_publish_enabled,
            s.confirmed_at as source_confirmed_at
        from talent_import_items i
        join talent_import_batches b using (batch_id)
        join talent_sources s using (source_id)
        where i.item_id = p_item_id
    ) detail;
$$ language sql stable;
