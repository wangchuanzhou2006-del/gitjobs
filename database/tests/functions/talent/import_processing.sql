begin;
select plan(27);

\set sourceCreator '00000000-0000-0000-0000-000000000511'

insert into "user" (auth_hash, email, name, user_id, username)
values (
    decode('02', 'hex'),
    'talent-import@example.com',
    'Talent Import',
    :'sourceCreator',
    'talent_import_creator'
);

select upsert_talent_source('other', 'Synthetic Import Source', :'sourceCreator') as source_id
\gset

select lives_ok(
    format(
        $sql$
        select create_talent_import_batch(
            jsonb_build_object(
                'source_id', %L::uuid,
                'imported_by', %L::uuid,
                'raw_text_ciphertext', '010203',
                'encryption_key_version', 1,
                'items', jsonb_build_array(
                    jsonb_build_object(
                        'position', 0,
                        'raw_fragment_ciphertext', '0a0b0c',
                        'encryption_key_version', 1,
                        'kind', 'hiring'
                    ),
                    jsonb_build_object(
                        'position', 1,
                        'raw_fragment_ciphertext', '0d0e0f',
                        'encryption_key_version', 1,
                        'kind', 'seeking'
                    )
                )
            )
        )
        $sql$,
        :'source_id',
        :'sourceCreator'
    ),
    'batch and encrypted fragments are created together'
);

select results_eq(
    $$
    select item_count, processed_count, failed_count, status
    from talent_import_batches
    order by created_at
    limit 1
    $$,
    $$ values (2, 0, 0, 'queued'::text) $$,
    'new batch counters match its fragments'
);

select is(
    (select count(*) from talent_import_items),
    2::bigint,
    'all batch fragments are persisted'
);

select throws_ok(
    format(
        $sql$
        select create_talent_import_batch(
            jsonb_build_object(
                'source_id', %L::uuid,
                'raw_text_ciphertext', '111213',
                'encryption_key_version', 1,
                'items', jsonb_build_array(
                    jsonb_build_object(
                        'position', 0,
                        'raw_fragment_ciphertext', '14',
                        'encryption_key_version', 1,
                        'kind', 'hiring'
                    ),
                    jsonb_build_object(
                        'position', 0,
                        'raw_fragment_ciphertext', '15',
                        'encryption_key_version', 1,
                        'kind', 'seeking'
                    )
                )
            )
        )
        $sql$,
        :'source_id'
    ),
    '23505',
    null,
    'invalid fragments reject the whole batch'
);

select is(
    (select count(*) from talent_import_batches),
    1::bigint,
    'a failed fragment insert leaves no partial batch'
);

select throws_ok(
    format(
        $sql$
        select create_talent_import_batch(
            jsonb_build_object(
                'source_id', %L::uuid,
                'raw_text_ciphertext', '212223',
                'encryption_key_version', 1,
                'items', (
                    select jsonb_agg(
                        jsonb_build_object(
                            'position', position,
                            'raw_fragment_ciphertext', '24',
                            'encryption_key_version', 1,
                            'kind', 'hiring'
                        )
                    )
                    from generate_series(0, 500) position
                )
            )
        )
        $sql$,
        :'source_id'
    ),
    '22023',
    'talent import must contain between 1 and 500 items',
    'database boundary rejects more than five hundred fragments'
);

select throws_ok(
    format(
        $sql$
        select create_talent_import_batch(
            jsonb_build_object(
                'source_id', %L::uuid,
                'raw_text_ciphertext', '252627',
                'encryption_key_version', 1,
                'items', jsonb_build_array()
            )
        )
        $sql$,
        :'source_id'
    ),
    '22023',
    'talent import must contain between 1 and 500 items',
    'database boundary rejects an empty fragment collection'
);

select claim_talent_import_item() as first_claim
\gset

select is(
    :'first_claim'::jsonb->>'status',
    'extracting',
    'claim atomically moves an item to extracting'
);

select claim_talent_import_item() as second_claim
\gset

select isnt(
    :'first_claim'::jsonb->>'item_id',
    :'second_claim'::jsonb->>'item_id',
    'two claims cannot return the same item'
);

select is(
    (select status from talent_import_batches limit 1),
    'processing',
    'claim marks its batch as processing'
);

select throws_ok(
    format(
        $sql$
        select complete_talent_import_item(
            jsonb_build_object(
                'item_id', %L::uuid,
                'status', 'auto_published',
                'extraction', jsonb_build_object('kind', 'hiring')
            )
        )
        $sql$,
        :'first_claim'::jsonb->>'item_id'
    ),
    '22023',
    'talent import completion can only require review',
    'worker completion cannot bypass automatic publication policy'
);

select throws_ok(
    format(
        $sql$
        select complete_talent_import_item(
            jsonb_build_object(
                'item_id', %L::uuid,
                'extraction', '{}'::jsonb,
                'raw_fragment_ciphertext', 'fictional-private-fragment'
            )
        )
        $sql$,
        :'first_claim'::jsonb->>'item_id'
    ),
    '22023',
    'talent import completion contains an unknown field',
    'worker completion rejects unknown raw or contact-bearing root fields'
);

select throws_ok(
    format(
        $sql$
        select complete_talent_import_item(
            jsonb_build_object(
                'item_id', %L::uuid,
                'extraction', '{}'::jsonb,
                'extraction_confidence', '0.95'
            )
        )
        $sql$,
        :'first_claim'::jsonb->>'item_id'
    ),
    '22023',
    'invalid talent extraction confidence',
    'worker completion rejects incorrectly typed numeric root fields'
);

select throws_ok(
    format(
        $sql$
        select complete_talent_import_item(
            jsonb_build_object(
                'item_id', %L::uuid,
                'extraction', '{}'::jsonb,
                'contact_hashes', jsonb_build_object('hash', 'synthetic')
            )
        )
        $sql$,
        :'first_claim'::jsonb->>'item_id'
    ),
    '22023',
    'talent completion contact hashes must be a JSON array',
    'worker completion rejects incorrectly typed array root fields'
);

select throws_ok(
    format(
        $sql$
        select complete_talent_import_item(
            jsonb_build_object(
                'item_id', %L::uuid,
                'extraction', jsonb_build_object(
                    'raw_fragment_ciphertext', 'fictional-private-fragment'
                )
            )
        )
        $sql$,
        :'first_claim'::jsonb->>'item_id'
    ),
    '22023',
    'talent extraction contains an unknown field',
    'worker completion rejects unknown raw or contact-bearing extraction fields'
);

select throws_ok(
    format(
        $sql$
        select complete_talent_import_item(
            jsonb_build_object(
                'item_id', %L::uuid,
                'extraction', '{}'::jsonb,
                'contact_masked', jsonb_build_array(
                    jsonb_build_object(
                        'contact_type', 'wechat',
                        'masked_value', 'fictionalCompleteWechat001'
                    )
                )
            )
        )
        $sql$,
        :'first_claim'::jsonb->>'item_id'
    ),
    '22023',
    'invalid masked talent completion contact',
    'worker completion rejects an unmasked complete contact'
);

select throws_ok(
    format(
        $sql$
        select complete_talent_import_item(
            jsonb_build_object(
                'item_id', %L::uuid,
                'extraction', '{}'::jsonb,
                'risk_reasons', jsonb_build_array(
                    jsonb_build_object('hard_block', false)
                )
            )
        )
        $sql$,
        :'first_claim'::jsonb->>'item_id'
    ),
    '22023',
    'talent completion risk reasons contain an invalid entry',
    'worker completion requires complete typed risk reasons'
);

select throws_ok(
    format(
        $sql$
        select complete_talent_import_item(
            jsonb_build_object(
                'item_id', %L::uuid,
                'extraction', '{}'::jsonb,
                'duplicate_candidates', jsonb_build_array(
                    jsonb_build_object('score', 0.5, 'high_similarity', false)
                )
            )
        )
        $sql$,
        :'first_claim'::jsonb->>'item_id'
    ),
    '22023',
    'talent completion duplicate candidates contain an invalid entry',
    'worker completion requires complete typed duplicate candidates'
);

select lives_ok(
    format(
        $sql$
        select complete_talent_import_item(
            jsonb_build_object(
                'item_id', %L::uuid,
                'status', 'needs_review',
                'extraction', jsonb_build_object(
                    'kind', 'hiring',
                    'title', 'Synthetic Rust Role',
                    'contacts', jsonb_build_array(
                        jsonb_build_object(
                            'contact_type', 'wechat',
                            'value', 'fictional_complete_contact_001'
                        )
                    )
                ),
                'extraction_confidence', 0.82,
                'contact_candidates_ciphertext', '161718',
                'contact_hashes', jsonb_build_array('synthetic-contact-hash'),
                'contact_masked', jsonb_build_array(
                    jsonb_build_object('contact_type', 'wechat', 'masked_value', 'syn***001')
                ),
                'validation_errors', jsonb_build_array(),
                'risk_score', 10,
                'risk_level', 'low',
                'risk_reasons', jsonb_build_array(),
                'duplicate_candidates', jsonb_build_array()
            )
        )
        $sql$,
        :'first_claim'::jsonb->>'item_id'
    ),
    'an extracting item can be completed'
);

select results_eq(
    $$
    select processed_count, failed_count
    from talent_import_batches
    limit 1
    $$,
    $$ values (1, 0) $$,
    'completion increments the batch processed counter'
);

select ok(
    (
        select not extraction ? 'contacts'
        from talent_import_items
        where item_id = (:'first_claim'::jsonb->>'item_id')::uuid
    ),
    'stored extraction contains no contacts key'
);

select ok(
    (
        select extraction::text not like '%fictional_complete_contact_001%'
        from talent_import_items
        where item_id = (:'first_claim'::jsonb->>'item_id')::uuid
    ),
    'stored extraction contains no complete contact value'
);

select lives_ok(
    format(
        $sql$
        select fail_talent_import_item(
            jsonb_build_object(
                'item_id', %L::uuid,
                'validation_errors', jsonb_build_array(
                    jsonb_build_object('code', 'synthetic_failure', 'field', 'fragment')
                )
            )
        )
        $sql$,
        :'second_claim'::jsonb->>'item_id'
    ),
    'an extracting item can fail'
);

select results_eq(
    $$
    select status, processed_count, failed_count
    from talent_import_batches
    limit 1
    $$,
    $$ values ('partially_failed'::text, 2, 1) $$,
    'one item failure produces a partially failed completed batch'
);

select throws_ok(
    format(
        $sql$
        select complete_talent_import_item(
            jsonb_build_object(
                'item_id', %L::uuid,
                'status', 'needs_review',
                'extraction', '{}'::jsonb
            )
        )
        $sql$,
        :'first_claim'::jsonb->>'item_id'
    ),
    'P0001',
    'talent import item is not extracting',
    'completion rejects an invalid state transition'
);

select throws_ok(
    format(
        $sql$
        select fail_talent_import_item(
            jsonb_build_object(
                'item_id', %L::uuid,
                'validation_errors', jsonb_build_array()
            )
        )
        $sql$,
        :'first_claim'::jsonb->>'item_id'
    ),
    'P0001',
    'talent import item is not extracting',
    'failure rejects an invalid state transition'
);

select is(
    claim_talent_import_item(),
    null::jsonb,
    'claim returns null when no queued item remains'
);

select * from finish();
rollback;
