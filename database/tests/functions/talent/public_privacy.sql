begin;
select plan(16);

\set moderatorId '00000000-0000-0000-0000-000000000701'
\set sourceId '00000000-0000-0000-0000-000000000711'
\set batchId '00000000-0000-0000-0000-000000000721'
\set itemId '00000000-0000-0000-0000-000000000731'
\set unmaskedItemId '00000000-0000-0000-0000-000000000732'

insert into "user" (auth_hash, email, name, user_id, username)
values (
    decode('71', 'hex'),
    'talent-privacy-moderator@example.com',
    'Talent Privacy Moderator',
    :'moderatorId',
    'talent_privacy_moderator'
);

insert into talent_sources (source_id, source_type, name, trust_status)
values (:'sourceId', 'other', 'Synthetic Privacy Source', 'trusted');

insert into talent_import_batches (
    batch_id, source_id, raw_text_ciphertext, encryption_key_version, status, item_count, processed_count
)
values (:'batchId', :'sourceId', '70726976617465', 1, 'completed', 1, 1);

insert into talent_import_items (
    item_id, batch_id, position, raw_fragment_ciphertext, encryption_key_version,
    contact_candidates_ciphertext, contact_hashes, contact_masked, kind, status,
    extraction, extraction_confidence, validation_errors, risk_score, risk_level,
    risk_reasons, duplicate_candidates
)
values (
    :'itemId', :'batchId', 0, '667261676d656e74', 1,
    '636f6e74616374',
    array['fictional_complete_contact_701', 'synthetic-wechat-hash-701'],
    '[{"contact_type":"wechat","masked_value":"fic***701"}]'::jsonb,
    'seeking', 'needs_review',
    '{"kind":"seeking","title":"Synthetic Privacy Candidate","city":"Test City"}'::jsonb,
    0.91, '[]'::jsonb, 10, 'low', '[]'::jsonb, '[]'::jsonb
);

insert into talent_import_items (
    item_id, batch_id, position, raw_fragment_ciphertext, encryption_key_version,
    contact_candidates_ciphertext, contact_hashes, contact_masked, kind, status,
    extraction, extraction_confidence, validation_errors, risk_score, risk_level,
    risk_reasons, duplicate_candidates
)
values (
    :'unmaskedItemId', :'batchId', 1, '756e6d61736b6564', 1,
    '756e6d61736b65642d636f6e74616374',
    array['synthetic-unmasked-hash'],
    '[{"contact_type":"wechat","masked_value":"unm***702"}]'::jsonb,
    'seeking', 'needs_review',
    '{"kind":"seeking","title":"Synthetic Unmasked Candidate","city":"Test City"}'::jsonb,
    0.91, '[]'::jsonb, 10, 'low', '[]'::jsonb, '[]'::jsonb
);

select throws_ok(
    format(
        $sql$
        select * from publish_talent_item(
            %L::uuid,
            0,
            %L::uuid,
            false,
            '{"kind":"seeking","title":"Synthetic Unmasked Candidate","city":"Test City"}'::jsonb,
            '[{"contact_type":"wechat","ciphertext":"756e6d61736b65642d636f6e74616374","encryption_key_version":1,"normalized_hash":"synthetic-unmasked-hash","masked_value":"fictionalCompleteWechat702"}]'::jsonb
        )
        $sql$,
        :'unmaskedItemId',
        :'moderatorId'
    ),
    '22023',
    'invalid masked talent publication contact',
    'publication rejects a plausible complete contact in masked display'
);

select * from publish_talent_item(
    :'itemId',
    0,
    :'moderatorId',
    false,
    jsonb_build_object(
        'kind', 'seeking',
        'title', 'Synthetic Privacy Candidate',
        'city', 'Test City',
        'published_at', current_timestamp
    ),
    '[
        {"contact_type":"phone","ciphertext":"636f6e74616374","encryption_key_version":1,"normalized_hash":"fictional_complete_contact_701","masked_value":"+86****0001"},
        {"contact_type":"wechat","ciphertext":"776563686174","encryption_key_version":1,"normalized_hash":"synthetic-wechat-hash-701","masked_value":"tes***_01"}
    ]'::jsonb
)
\gset published_

insert into talent_contacts (
    record_id,
    contact_type,
    ciphertext,
    encryption_key_version,
    normalized_hash,
    masked_value
)
values (
    :'published_record_id',
    'wechat',
    '636f72727570746564',
    1,
    'synthetic-corrupt-contact-hash',
    'fictionalCompleteWechat703'
);

select like(
    list_published_talent_records(null, 50)::text,
    '%+86****0001%',
    'public records include canonical international phone masking'
);

select like(
    list_published_talent_records(null, 50)::text,
    '%tes***_01%',
    'public records include canonical WeChat underscore masking'
);

select unlike(
    list_published_talent_records(null, 50)::text,
    '%fictional_complete_contact_701%',
    'public records exclude complete contact and contact hash'
);

select unlike(
    list_published_talent_records(null, 50)::text,
    '%636f6e74616374%',
    'public records exclude ciphertext'
);

select unlike(
    list_published_talent_records(null, 50)::text,
    '%fictionalCompleteWechat703%',
    'public records fail closed around corrupt unmasked contact rows'
);

select throws_ok(
    format(
        $$select withdraw_talent_record(%L::uuid, %L::uuid, 1, %L::uuid, 'fictional_complete_contact_704')$$,
        :'itemId',
        :'published_record_id',
        :'moderatorId'
    ),
    '22023',
    'invalid talent audit reason',
    'withdrawal audit reason rejects a plausible complete contact'
);

select is(
    withdraw_talent_record(
        :'itemId',
        :'published_record_id',
        0,
        :'moderatorId',
        'withdrawn'
    ),
    null::integer,
    'stale withdrawal is rejected without changing the public record'
);

select is(
    (select status from talent_records where record_id = :'published_record_id'),
    'published',
    'stale withdrawal leaves the record published'
);

select lives_ok(
    format(
        $$select withdraw_talent_record(%L::uuid, %L::uuid, 1, %L::uuid, 'withdrawn')$$,
        :'itemId',
        :'published_record_id',
        :'moderatorId'
    ),
    'published record can be withdrawn'
);

select is(
    jsonb_array_length(list_published_talent_records(null, 50)),
    0,
    'withdrawal hides the record from public results'
);

select is(
    withdraw_talent_record(
        :'itemId',
        :'published_record_id',
        1,
        :'moderatorId',
        'withdrawn'
    ),
    2,
    'replaying the original withdrawal version returns the existing outcome'
);

select is(
    jsonb_array_length(list_published_talent_records(null, 50)),
    0,
    'replaying the original withdrawal keeps the record out of public results'
);

select is(
    (select lock_version from talent_import_items where item_id = :'itemId'),
    2,
    'repeating a withdrawal does not advance the item version'
);

select is(
    (
        select count(*)
        from talent_review_events
        where item_id = :'itemId' and action = 'withdrawn'
    ),
    1::bigint,
    'repeating a withdrawal does not create another audit event'
);

select is(
    (select withdrawn_count from talent_sources where source_id = :'sourceId'),
    1,
    'replaying a withdrawal does not increment the source counter again'
);

select * from finish();
rollback;
