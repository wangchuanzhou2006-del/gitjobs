begin;
select plan(24);

insert into "user" (user_id, auth_hash, email, name, username, moderator)
values ('00000000-0000-0000-0000-000000000701', decode('07', 'hex'), 'radar@example.test', 'Radar Moderator', 'radar_moderator', true);

select upsert_talent_source('web', 'Synthetic Public Feed', '00000000-0000-0000-0000-000000000701') source_id \gset

select throws_ok(
    format($sql$ select upsert_talent_source_subscription(jsonb_build_object(
        'source_id', %L, 'public_url', 'http://example.test/feed',
        'keywords', jsonb_build_array('cook'), 'interval_minutes', 720,
        'actor_user_id', '00000000-0000-0000-0000-000000000701'
    )) $sql$, :'source_id'),
    '22023', 'source subscription requires a public HTTPS URL',
    'non-HTTPS subscriptions are rejected'
);

select upsert_talent_source_subscription(jsonb_build_object(
    'source_id', :'source_id',
    'public_url', 'https://feeds.example.test/talent.xml',
    'keywords', jsonb_build_array(' Cook ', 'cook', 'Kitchen'),
    'interval_minutes', 720,
    'actor_user_id', '00000000-0000-0000-0000-000000000701'
)) subscription_id \gset

select is((select keywords from talent_source_subscriptions where subscription_id = :'subscription_id'), array['cook', 'kitchen'], 'keywords are normalized and deduplicated');
select is((select interval_minutes from talent_source_subscriptions where subscription_id = :'subscription_id'), 720, 'plain update interval is stored');

select upsert_talent_source_subscription(jsonb_build_object(
    'source_id', :'source_id', 'public_url', 'https://feeds.example.test/talent.xml',
    'keywords', jsonb_build_array('chef'), 'interval_minutes', 1440,
    'actor_user_id', '00000000-0000-0000-0000-000000000701'
)) updated_subscription_id \gset

select is(:'updated_subscription_id'::uuid, :'subscription_id'::uuid, 'upsert reuses the same source subscription');
select is((select count(*) from talent_source_subscriptions), 1::bigint, 'upsert creates no duplicate row');

select upsert_talent_source_subscription(jsonb_build_object(
    'subscription_id', :'subscription_id', 'source_id', :'source_id',
    'public_url', 'https://feeds.example.test/updated.xml',
    'keywords', jsonb_build_array('chef'), 'interval_minutes', 1440,
    'actor_user_id', '00000000-0000-0000-0000-000000000701'
)) edited_subscription_id \gset
select is(:'edited_subscription_id'::uuid, :'subscription_id'::uuid, 'focused edit preserves the subscription identity');
select is((select public_url from talent_source_subscriptions where subscription_id = :'subscription_id'), 'https://feeds.example.test/updated.xml', 'focused edit can change the public URL');
select ok(reserve_talent_source_item(:'subscription_id', 'entry-1', repeat('a', 64)), 'first source item is reserved');
select is(reserve_talent_source_item(:'subscription_id', 'entry-1', repeat('b', 64)), false, 'same source item ID is a duplicate');
select is(reserve_talent_source_item(:'subscription_id', 'entry-2', repeat('a', 64)), false, 'same content fingerprint is a duplicate');
select is((select count(*) from talent_source_seen_items), 1::bigint, 'duplicate reservations create no extra row');
select ok(release_talent_source_item(:'subscription_id', 'entry-1', repeat('a', 64)), 'failed import can release its reservation');
select is((select count(*) from talent_source_seen_items), 0::bigint, 'released item can be retried later');

select claim_talent_source_subscription() claim \gset
select is(:'claim'::jsonb->'subscription'->>'subscription_id', :'subscription_id', 'due subscription is claimed atomically');
select is(claim_talent_source_subscription(), null::jsonb, 'an active claim cannot be claimed twice');

select ok(complete_talent_source_run(jsonb_build_object(
    'run_id', :'claim'::jsonb->>'run_id', 'discovered_count', 3,
    'accepted_count', 0, 'failed_count', 1, 'error_code', 'fetch_timeout',
    'incremental_cursor', 'must-not-advance'
)), 'failed run completes');
select is((select incremental_cursor from talent_source_subscriptions where subscription_id = :'subscription_id'), null::text, 'failure leaves cursor unchanged');
select is((select consecutive_failures from talent_source_subscriptions where subscription_id = :'subscription_id'), 1, 'failure count increases');

update talent_source_subscriptions set next_run_at = current_timestamp where subscription_id = :'subscription_id';
select claim_talent_source_subscription() retry_claim \gset
select ok(complete_talent_source_run(jsonb_build_object(
    'run_id', :'retry_claim'::jsonb->>'run_id', 'discovered_count', 4,
    'accepted_count', 2, 'duplicate_count', 2, 'incremental_cursor', 'cursor-4',
    'etag', 'synthetic-etag'
)), 'successful retry completes');
select is((select incremental_cursor from talent_source_subscriptions where subscription_id = :'subscription_id'), 'cursor-4', 'success advances cursor');
select is((select consecutive_failures from talent_source_subscriptions where subscription_id = :'subscription_id'), 0, 'success clears failure count');

select ok(set_talent_source_subscription_enabled(:'subscription_id', false, '00000000-0000-0000-0000-000000000701'), 'moderator can pause a source');
select is((select enabled from talent_source_subscriptions where subscription_id = :'subscription_id'), false, 'paused source is disabled');
select is(claim_talent_source_subscription(), null::jsonb, 'paused source cannot be claimed');

select * from finish();
rollback;
