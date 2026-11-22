create or replace function list_talent_source_subscriptions()
returns jsonb as $$
    select coalesce(jsonb_agg(row_data order by row_data->>'created_at'), '[]'::jsonb)
    from (
        select jsonb_build_object(
            'subscription_id', subscription.subscription_id,
            'source_id', subscription.source_id,
            'source_name', source.name,
            'public_url', subscription.public_url,
            'keywords', subscription.keywords,
            'interval_minutes', subscription.interval_minutes,
            'enabled', subscription.enabled,
            'next_run_at', subscription.next_run_at,
            'incremental_cursor', subscription.incremental_cursor,
            'etag', subscription.etag,
            'last_modified', subscription.last_modified,
            'consecutive_failures', subscription.consecutive_failures,
            'last_success_at', subscription.last_success_at,
            'last_error_at', subscription.last_error_at,
            'last_error_code', subscription.last_error_code,
            'created_by', subscription.created_by,
            'created_at', subscription.created_at
        ) row_data
        from talent_source_subscriptions subscription
        join talent_sources source using (source_id)
    ) rows;
$$ language sql stable;
