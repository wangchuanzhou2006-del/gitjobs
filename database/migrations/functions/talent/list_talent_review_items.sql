-- Lists review-safe import summaries without raw or complete-contact material.
create or replace function list_talent_review_items(
    p_statuses text[] default null,
    p_source_id uuid default null,
    p_limit integer default 50
)
returns jsonb as $$
    select coalesce(
        jsonb_agg(to_jsonb(summary) order by summary.updated_at desc, summary.item_id),
        '[]'::jsonb
    )
    from (
        select
            i.item_id,
            i.batch_id,
            i.position,
            i.kind,
            i.status,
            i.extraction,
            i.extraction_confidence,
            i.contact_masked,
            coalesce(cardinality(i.contact_hashes), 0) > 0 as has_contact,
            i.validation_errors,
            i.risk_score,
            i.risk_level,
            i.risk_reasons,
            i.duplicate_candidates,
            i.lock_version,
            i.processed_at,
            i.reviewed_at,
            i.updated_at,
            s.source_id,
            s.source_type,
            s.name as source_name,
            s.trust_status as source_trust_status
        from talent_import_items i
        join talent_import_batches b using (batch_id)
        join talent_sources s using (source_id)
        where (
            p_statuses is null
            or cardinality(p_statuses) = 0
            or i.status = any(p_statuses)
        )
          and (p_source_id is null or s.source_id = p_source_id)
        order by i.updated_at desc, i.item_id
        limit greatest(0, least(coalesce(p_limit, 50), 500))
    ) summary;
$$ language sql stable;
