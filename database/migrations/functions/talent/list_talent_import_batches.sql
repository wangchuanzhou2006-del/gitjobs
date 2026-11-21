-- Lists recent imports with safe source metadata and processing counters.
create or replace function list_talent_import_batches(p_limit integer)
returns jsonb as $$
    select coalesce(
        jsonb_agg(to_jsonb(summary) order by summary.created_at desc, summary.batch_id desc),
        '[]'::jsonb
    )
    from (
        select
            b.batch_id,
            b.source_id,
            s.source_type,
            s.name as source_name,
            s.trust_status as source_trust_status,
            b.status,
            b.item_count,
            b.processed_count,
            b.failed_count,
            b.model_provider,
            b.model_name,
            b.prompt_version,
            b.created_at,
            b.completed_at
        from talent_import_batches b
        join talent_sources s using (source_id)
        order by b.created_at desc, b.batch_id desc
        limit greatest(0, least(coalesce(p_limit, 50), 500))
    ) summary;
$$ language sql stable;
