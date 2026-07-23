begin;
select plan(5);

\set moderatorId '00000000-0000-0000-0000-000000000701'
\set sourceId '00000000-0000-0000-0000-000000000711'
\set batchId '00000000-0000-0000-0000-000000000721'
\set itemId '00000000-0000-0000-0000-000000000731'

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
    array['fictional_complete_contact_701'],
    '[{"contact_type":"wechat","masked_value":"fic***701"}]'::jsonb,
    'seeking', 'needs_review',
    '{"kind":"seeking","title":"Synthetic Privacy Candidate","city":"Test City"}'::jsonb,
    0.91, '[]'::jsonb, 10, 'low', '[]'::jsonb, '[]'::jsonb
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
    '[{"contact_type":"wechat","ciphertext":"636f6e74616374","encryption_key_version":1,"normalized_hash":"fictional_complete_contact_701","masked_value":"fic***701"}]'::jsonb
)
\gset published_

select like(
    list_published_talent_records(null, 50)::text,
    '%fic***701%',
    'public records include masked contact display'
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

select lives_ok(
    format(
        $$select withdraw_talent_record(%L::uuid, %L::uuid, 'synthetic withdrawal')$$,
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

select * from finish();
rollback;
