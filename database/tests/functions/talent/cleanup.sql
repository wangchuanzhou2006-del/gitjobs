begin;
select plan(12);

\set sourceId '00000000-0000-0000-0000-000000000811'
\set batchId '00000000-0000-0000-0000-000000000821'
\set itemId '00000000-0000-0000-0000-000000000831'
\set recordId '00000000-0000-0000-0000-000000000841'
\set cappedBatchId '00000000-0000-0000-0000-000000000822'
\set cappedItemId '00000000-0000-0000-0000-000000000832'

insert into talent_sources (source_id, source_type, name)
values (:'sourceId', 'other', 'Synthetic Cleanup Source');

insert into talent_import_batches (
    batch_id, source_id, raw_text_ciphertext, encryption_key_version, status,
    item_count, processed_count, created_at, raw_delete_after
)
values (
    :'batchId', :'sourceId', '6f6c642d6261746368', 1, 'completed',
    1, 1, current_timestamp - interval '91 days', current_timestamp - interval '1 day'
);

insert into talent_records (
    record_id, kind, title, source_id, status, creation_source, published_at
)
values (
    :'recordId', 'hiring', 'Preserved Cleanup Record', :'sourceId',
    'published', 'manual_review', current_timestamp - interval '91 days'
);

insert into talent_import_items (
    item_id, batch_id, position, raw_fragment_ciphertext, encryption_key_version,
    contact_candidates_ciphertext, contact_hashes, contact_masked,
    normalized_fingerprint, kind, status, extraction, extraction_confidence,
    validation_errors, risk_score, risk_level, risk_reasons, duplicate_candidates,
    published_record_id, created_at
)
values (
    :'itemId', :'batchId', 0, '6f6c642d667261676d656e74', 1,
    '6f6c642d636f6e74616374',
    array['preserved-cleanup-contact-hash'],
    '[{"contact_type":"wechat","masked_value":"pre***801"}]'::jsonb,
    'preserved-cleanup-fingerprint',
    'hiring', 'published',
    '{"kind":"hiring","title":"Preserved Cleanup Record"}'::jsonb,
    0.95, '[]'::jsonb, 10, 'low', '[]'::jsonb, '[]'::jsonb,
    :'recordId', current_timestamp - interval '91 days'
);

insert into talent_review_events (
    item_id, record_id, actor_type, action, after_snapshot
)
values (
    :'itemId', :'recordId', 'system', 'published',
    '{"kind":"hiring","title":"Preserved Cleanup Record"}'::jsonb
);

insert into talent_import_batches (
    batch_id, source_id, raw_text_ciphertext, encryption_key_version, status,
    item_count, processed_count, created_at, raw_delete_after
)
values (
    :'cappedBatchId', :'sourceId', '6c6174652d646561646c696e65', 1, 'completed',
    1, 1, current_timestamp - interval '91 days', current_timestamp + interval '30 days'
);

insert into talent_import_items (
    item_id, batch_id, position, raw_fragment_ciphertext, encryption_key_version,
    contact_candidates_ciphertext, kind, status, created_at
)
values (
    :'cappedItemId', :'cappedBatchId', 0, '6c6174652d667261676d656e74', 1,
    '6c6174652d636f6e74616374', 'hiring', 'failed', current_timestamp - interval '91 days'
);

select lives_ok(
    $$select cleanup_talent_raw_data(current_timestamp)$$,
    'cleanup accepts an explicit clock'
);

select is(
    (select raw_text_ciphertext from talent_import_batches where batch_id = :'batchId'),
    null::text,
    'cleanup nulls old batch ciphertext'
);

select is(
    (select raw_text_ciphertext from talent_import_batches where batch_id = :'cappedBatchId'),
    null::text,
    'cleanup enforces the ninety-day cap despite a later explicit deadline'
);

select is(
    (select raw_fragment_ciphertext from talent_import_items where item_id = :'itemId'),
    null::text,
    'cleanup nulls old fragment ciphertext'
);

select is(
    (select contact_candidates_ciphertext from talent_import_items where item_id = :'itemId'),
    null::text,
    'cleanup nulls old contact-candidate ciphertext'
);

select ok(
    (select raw_deleted_at is not null from talent_import_batches where batch_id = :'batchId'),
    'cleanup timestamps raw-data removal'
);

select results_eq(
    $$
    select contact_hashes, normalized_fingerprint, extraction
    from talent_import_items
    where item_id = '00000000-0000-0000-0000-000000000831'::uuid
    $$,
    $$
    values (
        array['preserved-cleanup-contact-hash']::text[],
        'preserved-cleanup-fingerprint'::text,
        '{"kind":"hiring","title":"Preserved Cleanup Record"}'::jsonb
    )
    $$,
    'cleanup preserves hashes, fingerprints, and safe extraction'
);

select is(
    (select count(*) from talent_review_events where item_id = :'itemId'),
    1::bigint,
    'cleanup preserves review events'
);

select is(
    (select count(*) from talent_records where record_id = :'recordId'),
    1::bigint,
    'cleanup preserves published records'
);

select is(
    (select count(*) from talent_review_events where resource_scope = 'cleanup'),
    2::bigint,
    'cleanup writes one scoped audit event per cleaned batch'
);

select results_eq(
    $$
    select
        actor_type,
        action,
        reason,
        (before_snapshot->>'raw_text_present')::boolean,
        (before_snapshot->>'item_raw_payload_count')::integer,
        (after_snapshot->>'raw_text_present')::boolean,
        (after_snapshot->>'item_raw_payload_count')::integer
    from talent_review_events
    where resource_scope = 'cleanup'
      and batch_id = '00000000-0000-0000-0000-000000000821'::uuid
    $$,
    $$ values ('system'::text, 'raw_data_cleaned'::text, 'retention_expired'::text,
               true, 1::integer, false, 0::integer) $$,
    'cleanup audit records privacy-safe before and after retention state'
);

select cleanup_talent_raw_data(current_timestamp) as retry_cleanup_outcome
\gset

select is(
    (select count(*) from talent_review_events where resource_scope = 'cleanup'),
    2::bigint,
    'retrying cleanup creates no duplicate audit event'
);

select * from finish();
rollback;
