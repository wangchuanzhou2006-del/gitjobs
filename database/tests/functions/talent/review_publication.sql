begin;
select plan(82);

\set moderatorId '00000000-0000-0000-0000-000000000601'
\set secondModeratorId '00000000-0000-0000-0000-000000000602'
\set sourceId '00000000-0000-0000-0000-000000000611'
\set batchId '00000000-0000-0000-0000-000000000621'
\set publishItemId '00000000-0000-0000-0000-000000000631'
\set highRiskItemId '00000000-0000-0000-0000-000000000632'
\set mergedItemId '00000000-0000-0000-0000-000000000633'
\set rejectedItemId '00000000-0000-0000-0000-000000000634'
\set rollbackItemId '00000000-0000-0000-0000-000000000635'
\set autoItemId '00000000-0000-0000-0000-000000000636'
\set incompleteItemId '00000000-0000-0000-0000-000000000637'
\set gateItemId '00000000-0000-0000-0000-000000000638'
\set oldStructuredItemId '00000000-0000-0000-0000-000000000639'
\set targetRecordId '00000000-0000-0000-0000-000000000641'
\set failedReprocessItemId '00000000-0000-0000-0000-000000000653'

insert into "user" (auth_hash, email, name, user_id, username)
values
    (decode('61', 'hex'), 'talent-moderator-601@example.com', 'Talent Moderator 601', :'moderatorId', 'talent_moderator_601'),
    (decode('62', 'hex'), 'talent-moderator-602@example.com', 'Talent Moderator 602', :'secondModeratorId', 'talent_moderator_602');

insert into talent_sources (
    source_id,
    source_type,
    name,
    trust_status,
    auto_publish_enabled,
    confirmed_by,
    confirmed_at
)
values (
    :'sourceId',
    'other',
    'Synthetic Review Source',
    'trusted',
    true,
    :'moderatorId',
    current_timestamp
);

update talent_runtime_settings
set auto_publish_enabled = true, updated_by = :'moderatorId';

insert into talent_import_batches (
    batch_id,
    source_id,
    raw_text_ciphertext,
    encryption_key_version,
    status,
    item_count,
    processed_count
)
values (
    :'batchId',
    :'sourceId',
    '616263',
    1,
    'completed',
    8,
    8
);

insert into talent_import_items (
    item_id,
    batch_id,
    position,
    raw_fragment_ciphertext,
    encryption_key_version,
    contact_candidates_ciphertext,
    contact_hashes,
    contact_masked,
    normalized_fingerprint,
    kind,
    status,
    extraction,
    extraction_confidence,
    validation_errors,
    risk_score,
    risk_level,
    risk_reasons,
    duplicate_candidates,
    processed_at
)
values
    (
        :'publishItemId', :'batchId', 0, '6161', 1, '6262',
        array['fictional_complete_contact_601'],
        '[{"contact_type":"wechat","masked_value":"fic***601"}]'::jsonb,
        'synthetic-fingerprint-601',
        'hiring', 'needs_review',
        '{"kind":"hiring","title":"Synthetic Rust Role","city":"Test City","salaryMin":1000,"salaryMax":2000,"lodging":"\u63d0\u4f9b","housingAllowance":1200,"missingFields":[]}'::jsonb,
        0.95, '[]'::jsonb, 10, 'low', '[]'::jsonb, '[]'::jsonb, current_timestamp
    ),
    (
        :'highRiskItemId', :'batchId', 1, '6363', 1, '6464',
        array['synthetic-high-risk-hash'],
        '[{"contact_type":"wechat","masked_value":"ris***602"}]'::jsonb,
        'synthetic-fingerprint-602',
        'hiring', 'needs_review',
        '{"kind":"hiring","title":"High Risk Role","city":"Test City","salaryMin":1000,"salaryMax":2000,"roles":["Synthetic Role"],"publishedAt":"2026-07-23T00:00:00Z","missingFields":[]}'::jsonb,
        0.99, '[]'::jsonb, 90, 'high',
        '[{"code":"synthetic_hard_block","explanation":"Synthetic hard block","evidence":null,"hard_block":true}]'::jsonb,
        '[]'::jsonb, current_timestamp
    ),
    (
        :'mergedItemId', :'batchId', 2, '6565', 1, null,
        '{}'::text[], '[]'::jsonb, 'synthetic-fingerprint-603',
        'seeking', 'needs_review',
        '{"kind":"seeking","title":"Synthetic Candidate","city":"Test City","missingFields":[]}'::jsonb,
        0.75, '[]'::jsonb, 20, 'low', '[]'::jsonb, '[]'::jsonb, current_timestamp
    ),
    (
        :'rejectedItemId', :'batchId', 3, '6666', 1, null,
        '{}'::text[], '[]'::jsonb, null,
        'irrelevant', 'rejected', '{}'::jsonb,
        0.99, '[]'::jsonb, 0, 'low', '[]'::jsonb, '[]'::jsonb, current_timestamp
    ),
    (
        :'rollbackItemId', :'batchId', 4, '6767', 1, '6868',
        array['synthetic-rollback-hash'],
        '[{"contact_type":"wechat","masked_value":"rol***605"}]'::jsonb,
        'synthetic-fingerprint-605',
        'hiring', 'needs_review',
        '{"kind":"hiring","title":"Rollback Role","city":"Test City","salaryMin":1000,"salaryMax":2000,"missingFields":[]}'::jsonb,
        0.95, '[]'::jsonb, 10, 'low', '[]'::jsonb, '[]'::jsonb, current_timestamp
    ),
    (
        :'autoItemId', :'batchId', 5, '6969', 1, '7070',
        array['synthetic-auto-hash'],
        '[{"contact_type":"wechat","masked_value":"aut***606"}]'::jsonb,
        'synthetic-fingerprint-606',
        'seeking', 'needs_review',
        '{"kind":"seeking","title":"Automatic Candidate","city":"Test City","roles":["Synthetic Role"],"lodging":"\u53ef\u534f\u5546","housingAllowance":900,"publishedAt":"2026-07-23T00:00:00Z","missingFields":[]}'::jsonb,
        0.96, '[]'::jsonb, 5, 'low', '[]'::jsonb, '[]'::jsonb, current_timestamp
    ),
    (
        :'incompleteItemId', :'batchId', 6, '7171', 1, '7272',
        array['synthetic-incomplete-hash'],
        '[{"contact_type":"wechat","masked_value":"inc***607"}]'::jsonb,
        'synthetic-fingerprint-602',
        'hiring', 'needs_review',
        '{"kind":"hiring","title":null,"city":null,"salaryMin":null,"salaryMax":null,"publishedAt":null,"missingFields":["title","city","salaryMin","salaryMax","publishedAt"]}'::jsonb,
        0.97, '[]'::jsonb, 5, 'low', '[]'::jsonb, '[]'::jsonb, current_timestamp
    ),
    (
        :'gateItemId', :'batchId', 7, '7373', 1, '7474',
        array['synthetic-gate-hash'],
        '[{"contact_type":"wechat","masked_value":"gat***608"}]'::jsonb,
        'synthetic-fingerprint-608',
        'hiring', 'needs_review',
        '{"kind":"hiring","title":"Gate Role","city":"Test City","salaryMin":1000,"salaryMax":2000,"roles":["Synthetic Role"],"publishedAt":"2026-07-23T00:00:00Z","missingFields":[]}'::jsonb,
        0.96, '[]'::jsonb, 5, 'low', '[]'::jsonb, '[]'::jsonb, current_timestamp
    );

insert into talent_records (
    record_id, kind, title, city, status, creation_source, published_at
)
values (
    :'targetRecordId', 'seeking', 'Existing Synthetic Candidate', 'Test City',
    'published', 'manual_review', current_timestamp
);

create function pg_temp.talent_duplicate_probe(p_item_id uuid)
returns jsonb as $$
    select jsonb_build_object(
        'item_id', item.item_id,
        'kind', item.kind,
        'title', item.extraction->'title',
        'roles', coalesce(item.extraction->'roles', '[]'::jsonb),
        'city', item.extraction->'city',
        'district', item.extraction->'district',
        'salary_min', item.extraction->'salaryMin',
        'salary_max', item.extraction->'salaryMax',
        'source_id', batch.source_id,
        'contact_hashes', to_jsonb(coalesce(item.contact_hashes, '{}'::text[])),
        'normalized_fingerprint', to_jsonb(item.normalized_fingerprint),
        'normalized_text', lower(
            concat_ws(
                ' ',
                item.extraction->>'title',
                array_to_string(
                    array(
                        select jsonb_array_elements_text(
                            coalesce(item.extraction->'roles', '[]'::jsonb)
                        )
                    ),
                    ' '
                ),
                item.extraction->>'city',
                item.extraction->>'district'
            )
        )
    )
    from talent_import_items item
    join talent_import_batches batch using (batch_id)
    where item.item_id = p_item_id;
$$ language sql stable;

select is(
    (
        select count(*)
        from jsonb_array_elements(list_talent_review_items(array['needs_review'], null, 50))
    ),
    6::bigint,
    'review list filters return only requested states'
);

select ok(
    not exists (
        select 1
        from jsonb_array_elements(list_talent_review_items(array['needs_review'], null, 50)) item
        where item->>'status' <> 'needs_review'
    ),
    'review list does not leak other states'
);

select is(
    jsonb_array_length(list_talent_review_items('{}'::text[], null, 50)),
    8,
    'an empty state filter returns all review items'
);

select is(
    jsonb_array_length(
        list_talent_review_items(
            array['needs_review'],
            :'sourceId',
            50,
            array['hiring'],
            array['high'],
            current_timestamp - interval '1 day',
            :'batchId'
        )
    ),
    1,
    'review list combines kind and risk filters'
);

select is(
    jsonb_array_length(
        list_talent_review_items(
            '{}', gen_random_uuid(), 50, '{}', '{}', null, null
        )
    ),
    0,
    'review list filters by source'
);

select is(
    jsonb_array_length(
        list_talent_review_items(
            '{}', null, 50, '{}', '{}', current_timestamp + interval '1 day', null
        )
    ),
    0,
    'review list filters by update time'
);

select is(
    jsonb_array_length(
        list_talent_review_items(
            '{}', null, 50, '{}', '{}', null, gen_random_uuid()
        )
    ),
    0,
    'review list filters by batch'
);

select is(
    jsonb_array_length(
        list_talent_review_items(
            '{}', :'sourceId', 50, array['seeking'], array['low'], null, :'batchId'
        )
    ),
    2,
    'review list combines source, batch, kind, and risk filters'
);

select is(
    (
        select count(*)
        from update_talent_review_item(
            :'publishItemId',
            99,
            '{"extraction":{"title":"Stale title"}}'::jsonb,
            :'moderatorId'
        )
    ),
    0::bigint,
    'stale review updates affect zero rows'
);

select throws_ok(
    format(
        $sql$
        select * from update_talent_review_item(
            %L::uuid,
            0,
            '{"risk_reasons":{"hard_block":true}}'::jsonb,
            %L::uuid
        )
        $sql$,
        :'incompleteItemId',
        :'moderatorId'
    ),
    '22023',
    'talent review risk reasons must be a JSON array',
    'review updates reject malformed risk safety data'
);

select throws_ok(
    format(
        $sql$
        select * from update_talent_review_item(
            %L::uuid,
            0,
            '{"risk_reasons":[{"hard_block":false}]}'::jsonb,
            %L::uuid
        )
        $sql$,
        :'incompleteItemId',
        :'moderatorId'
    ),
    '22023',
    'talent review risk reasons contain an invalid entry',
    'review updates require complete typed risk reasons'
);

select throws_ok(
    format(
        $sql$
        select * from update_talent_review_item(
            %L::uuid,
            0,
            '{"risk_reasons":null}'::jsonb,
            %L::uuid
        )
        $sql$,
        :'incompleteItemId',
        :'moderatorId'
    ),
    '22023',
    'talent review risk reasons must be a JSON array',
    'review updates reject null risk safety data'
);

select throws_ok(
    format(
        $sql$
        select * from update_talent_review_item(
            %L::uuid,
            0,
            '{"duplicate_candidates":[{"score":0.5,"high_similarity":false}]}'::jsonb,
            %L::uuid
        )
        $sql$,
        :'incompleteItemId',
        :'moderatorId'
    ),
    '22023',
    'talent review duplicate candidates contain an invalid entry',
    'review updates require complete typed duplicate candidates'
);

select throws_ok(
    format(
        $sql$
        select * from update_talent_review_item(
            %L::uuid,
            0,
            '{"extraction":{"raw_fragment_ciphertext":"fictional-private-fragment"}}'::jsonb,
            %L::uuid
        )
        $sql$,
        :'incompleteItemId',
        :'moderatorId'
    ),
    '22023',
    'talent review extraction contains an unknown field',
    'review updates reject unknown raw or contact-bearing extraction fields'
);

select * from publish_talent_item(
    :'publishItemId',
    0,
    :'moderatorId',
    false,
    jsonb_build_object(
        'kind', 'hiring',
        'title', 'Synthetic Rust Role',
        'city', 'Test City',
        'salary_min', 1000,
        'salary_max', 2000,
        'lodging', U&'\63D0\4F9B',
        'housing_allowance', 1200,
        'published_at', current_timestamp
    ),
    jsonb_build_array(
        jsonb_build_object(
            'contact_type', 'wechat',
            'ciphertext', '6262',
            'encryption_key_version', 1,
            'normalized_hash', 'fictional_complete_contact_601',
            'masked_value', 'fic***601'
        )
    )
)
\gset first_publish_

select is(:'first_publish_created'::boolean, true, 'first publication creates a record');

select results_eq(
    format(
        $$select lodging, housing_allowance
          from talent_records
          where record_id = %L::uuid$$,
        :'first_publish_record_id'
    ),
    $$ values (U&'\63D0\4F9B'::text, 1200::bigint) $$,
    'manual publication preserves Chinese lodging and the allowance amount'
);

select * from publish_talent_item(
    :'publishItemId',
    0,
    :'moderatorId',
    false,
    '{}'::jsonb,
    '[]'::jsonb
)
\gset second_publish_

select is(
    :'second_publish_record_id'::uuid,
    :'first_publish_record_id'::uuid,
    'publishing the same item twice returns the same record ID'
);

select is(:'second_publish_created'::boolean, false, 'idempotent publication reports reuse');

select * from publish_talent_item(
    :'autoItemId',
    0,
    null,
    true,
    jsonb_build_object(
        'kind', 'seeking',
        'title', 'Automatic Candidate',
        'city', 'Test City',
        'published_at', current_timestamp
    ),
    '[{"contact_type":"wechat","ciphertext":"7070","encryption_key_version":1,"normalized_hash":"synthetic-auto-hash","masked_value":"aut***606"}]'::jsonb
)
\gset auto_publish_

select is(
    :'auto_publish_created'::boolean,
    true,
    'automatic publication creates a formal record only after all gates pass'
);

select results_eq(
    format(
        $$select lodging, housing_allowance
          from talent_records
          where record_id = %L::uuid$$,
        :'auto_publish_record_id'
    ),
    $$ values (U&'\53EF\534F\5546'::text, 900::bigint) $$,
    'automatic publication preserves lodging and the allowance amount'
);

select results_eq(
    $$
    select i.status, e.actor_type, e.actor_user_id, e.action
    from talent_import_items i
    join talent_review_events e using (item_id)
    where i.item_id = '00000000-0000-0000-0000-000000000636'::uuid
      and e.action = 'auto_published'
    $$,
    $$ values ('auto_published'::text, 'system'::text, null::uuid, 'auto_published'::text) $$,
    'automatic publication is the audited path to auto_published'
);

select throws_ok(
    format(
        $sql$
        select * from publish_talent_item(
            %L::uuid,
            0,
            null,
            true,
            '{"kind":"hiring","title":"Caller Supplied Complete Role","city":"Test City","salary_min":1000,"salary_max":2000,"published_at":"2026-07-23T00:00:00Z"}'::jsonb,
            '[{"contact_type":"wechat","ciphertext":"7272","encryption_key_version":1,"normalized_hash":"synthetic-incomplete-hash","masked_value":"inc***607"}]'::jsonb
        )
        $sql$,
        :'incompleteItemId'
    ),
    '22023',
    'automatic talent publication gates failed',
    'caller-supplied draft fields cannot bypass incomplete persisted extraction'
);

select ok(
    jsonb_array_length(
        find_talent_duplicate_candidates(
            pg_temp.talent_duplicate_probe(:'incompleteItemId'),
            20
        )
    ) > 0,
    'duplicate lookup returns exact persisted fingerprint candidates'
);

select ok(
    exists (
        select 1
        from jsonb_array_elements(
            find_talent_duplicate_candidates(
                pg_temp.talent_duplicate_probe(:'highRiskItemId'),
                20
            )
        ) candidate
        where candidate->>'candidate_item_id' = :'gateItemId'
          and candidate->'roles' = '["Synthetic Role"]'::jsonb
          and candidate->>'city' = 'Test City'
          and (candidate->>'salary_min')::integer = 1000
          and (candidate->>'salary_max')::integer = 2000
          and candidate->>'source_id' = :'sourceId'
          and candidate->>'normalized_text' <> ''
    ),
    'duplicate lookup returns safe structured fields for non-exact candidates'
);

select ok(
    not exists (
        select 1
        from jsonb_array_elements(
            find_talent_duplicate_candidates(
                pg_temp.talent_duplicate_probe(:'highRiskItemId'),
                20
            )
        ) candidate
        where candidate ?| array[
            'raw_fragment',
            'raw_fragment_ciphertext',
            'contact_hashes',
            'contact_candidates_ciphertext',
            'contacts'
        ]
    ),
    'duplicate lookup never returns raw or complete contact material'
);

select ok(
    exists (
        select 1
        from jsonb_array_elements(
            find_talent_duplicate_candidates(
                jsonb_build_object(
                    'item_id', '00000000-0000-0000-0000-000000000649',
                    'kind', 'hiring',
                    'title', 'New Exact Contact Subject',
                    'roles', jsonb_build_array('Different Role'),
                    'city', 'Different City',
                    'district', null,
                    'salary_min', 9000,
                    'salary_max', 10000,
                    'source_id', :'sourceId',
                    'contact_hashes', jsonb_build_array('synthetic-high-risk-hash'),
                    'normalized_fingerprint', 'new-subject-fingerprint',
                    'normalized_text', 'new exact contact subject'
                ),
                20
            )
        ) candidate
        where candidate->>'candidate_item_id' = :'highRiskItemId'
          and candidate->'reasons' = '["exact_contact_hash"]'::jsonb
    ),
    'initial lookup uses computed contact hashes before the subject is persisted'
);

select ok(
    exists (
        select 1
        from jsonb_array_elements(
            find_talent_duplicate_candidates(
                jsonb_build_object(
                    'item_id', '00000000-0000-0000-0000-000000000650',
                    'kind', 'seeking',
                    'title', 'New Exact Fingerprint Subject',
                    'roles', jsonb_build_array('Different Role'),
                    'city', 'Different City',
                    'district', null,
                    'salary_min', null,
                    'salary_max', null,
                    'source_id', :'sourceId',
                    'contact_hashes', jsonb_build_array(),
                    'normalized_fingerprint', 'synthetic-fingerprint-602',
                    'normalized_text', 'new exact fingerprint subject'
                ),
                20
            )
        ) candidate
        where candidate->>'candidate_item_id' in (:'highRiskItemId', :'incompleteItemId')
          and candidate->'reasons' = '["exact_fingerprint"]'::jsonb
    ),
    'initial lookup uses the computed fingerprint before the subject is persisted'
);

select ok(
    exists (
        select 1
        from jsonb_array_elements(
            find_talent_duplicate_candidates(
                jsonb_build_object(
                    'item_id', '00000000-0000-0000-0000-000000000651',
                    'kind', 'hiring',
                    'title', 'New Structured Subject',
                    'roles', jsonb_build_array('Synthetic Role'),
                    'city', 'Test City',
                    'district', null,
                    'salary_min', 1000,
                    'salary_max', 2000,
                    'source_id', :'sourceId',
                    'contact_hashes', jsonb_build_array(),
                    'normalized_fingerprint', null,
                    'normalized_text', 'new structured subject synthetic role test city'
                ),
                20
            )
        ) candidate
        where candidate->>'candidate_item_id' = :'highRiskItemId'
    ),
    'initial lookup ranks structured candidates from the supplied safe probe'
);

insert into talent_import_items (
    item_id,
    batch_id,
    position,
    contact_hashes,
    normalized_fingerprint,
    kind,
    status,
    extraction,
    extraction_confidence,
    validation_errors,
    risk_score,
    risk_level,
    risk_reasons,
    duplicate_candidates,
    processed_at,
    updated_at
)
values (
    :'oldStructuredItemId',
    :'batchId',
    20,
    array['old-structured-safe-hash'],
    'old-structured-fingerprint',
    'hiring',
    'needs_review',
    '{"kind":"hiring","title":"Old Structured Match","city":"Test City","salaryMin":1000,"salaryMax":2000,"roles":["Synthetic Role"],"missingFields":[]}'::jsonb,
    0.98,
    '[]'::jsonb,
    5,
    'low',
    '[]'::jsonb,
    '[]'::jsonb,
    current_timestamp - interval '30 days',
    current_timestamp - interval '30 days'
);

insert into talent_import_items (
    item_id,
    batch_id,
    position,
    contact_hashes,
    normalized_fingerprint,
    kind,
    status,
    extraction,
    extraction_confidence,
    validation_errors,
    risk_score,
    risk_level,
    risk_reasons,
    duplicate_candidates,
    processed_at,
    updated_at
)
select
    gen_random_uuid(),
    :'batchId',
    20 + decoy,
    array[format('selection-decoy-hash-%s', decoy)],
    format('selection-decoy-fingerprint-%s', decoy),
    'hiring',
    'needs_review',
    jsonb_build_object(
        'kind', 'hiring',
        'title', format('New Unrelated Candidate %s', decoy),
        'city', format('Other City %s', decoy),
        'salaryMin', 20000 + decoy,
        'salaryMax', 30000 + decoy,
        'roles', jsonb_build_array(format('Other Role %s', decoy)),
        'missingFields', '[]'::jsonb
    ),
    0.98,
    '[]'::jsonb,
    5,
    'low',
    '[]'::jsonb,
    '[]'::jsonb,
    current_timestamp + make_interval(secs => decoy),
    current_timestamp + make_interval(secs => decoy)
from generate_series(1, 6) decoy;

select ok(
    exists (
        select 1
        from jsonb_array_elements(
            find_talent_duplicate_candidates(
                pg_temp.talent_duplicate_probe(:'highRiskItemId'),
                3
            )
        ) candidate
        where candidate->>'candidate_item_id' = :'oldStructuredItemId'
    ),
    'safe structured ranking keeps an old potential high match ahead of newer unrelated rows'
);

delete from talent_import_items
where item_id = :'oldStructuredItemId'
   or normalized_fingerprint like 'selection-decoy-fingerprint-%';

update talent_import_items
set extraction = jsonb_set(extraction, '{roles}', '[]'::jsonb)
where item_id = :'gateItemId';

select throws_ok(
    format(
        $$select * from publish_talent_item(%L::uuid, 0, null, true, '{}'::jsonb, '[{"contact_type":"wechat","ciphertext":"7474","encryption_key_version":1,"normalized_hash":"synthetic-gate-hash","masked_value":"gat***608"}]'::jsonb)$$,
        :'gateItemId'
    ),
    '22023',
    'automatic talent publication gates failed',
    'automatic publication requires a nonempty persisted roles array'
);

update talent_import_items
set extraction = jsonb_set(extraction, '{roles}', '{"role":"Synthetic Role"}'::jsonb)
where item_id = :'gateItemId';

select throws_ok(
    format(
        $$select * from publish_talent_item(%L::uuid, 0, null, true, '{}'::jsonb, '[{"contact_type":"wechat","ciphertext":"7474","encryption_key_version":1,"normalized_hash":"synthetic-gate-hash","masked_value":"gat***608"}]'::jsonb)$$,
        :'gateItemId'
    ),
    '22023',
    'automatic talent publication gates failed',
    'malformed persisted roles fail closed without an array-length error'
);

update talent_import_items
set extraction = jsonb_set(extraction, '{roles}', '["Synthetic Role"]'::jsonb)
where item_id = :'gateItemId';

update talent_import_items
set validation_errors = '{"code":"synthetic_malformed"}'::jsonb
where item_id = :'gateItemId';

select throws_ok(
    format(
        $$select * from publish_talent_item(%L::uuid, 0, null, true, '{}'::jsonb, '[{"contact_type":"wechat","ciphertext":"7474","encryption_key_version":1,"normalized_hash":"synthetic-gate-hash","masked_value":"gat***608"}]'::jsonb)$$,
        :'gateItemId'
    ),
    '22023',
    'automatic talent publication gates failed',
    'malformed persisted validation errors fail closed without an array-length error'
);

update talent_import_items
set
    validation_errors = '[]'::jsonb,
    extraction = jsonb_set(extraction, '{missingFields}', '{"field":"title"}'::jsonb)
where item_id = :'gateItemId';

select throws_ok(
    format(
        $$select * from publish_talent_item(%L::uuid, 0, null, true, '{}'::jsonb, '[{"contact_type":"wechat","ciphertext":"7474","encryption_key_version":1,"normalized_hash":"synthetic-gate-hash","masked_value":"gat***608"}]'::jsonb)$$,
        :'gateItemId'
    ),
    '22023',
    'automatic talent publication gates failed',
    'malformed persisted missing fields fail closed without an array-length error'
);

update talent_import_items
set extraction = jsonb_set(extraction, '{missingFields}', '[]'::jsonb)
where item_id = :'gateItemId';

update talent_import_items set risk_reasons = null where item_id = :'gateItemId';

select throws_ok(
    format(
        $$select * from publish_talent_item(%L::uuid, 0, null, true, '{}'::jsonb, '[{"contact_type":"wechat","ciphertext":"7474","encryption_key_version":1,"normalized_hash":"synthetic-gate-hash","masked_value":"gat***608"}]'::jsonb)$$,
        :'gateItemId'
    ),
    '22023',
    'automatic talent publication gates failed',
    'automatic publication fails closed when persisted risk reasons are null'
);

update talent_import_items
set risk_reasons = '{"code":"synthetic","explanation":"malformed","evidence":null,"hard_block":false}'::jsonb
where item_id = :'gateItemId';

select throws_ok(
    format(
        $$select * from publish_talent_item(%L::uuid, 0, null, true, '{}'::jsonb, '[{"contact_type":"wechat","ciphertext":"7474","encryption_key_version":1,"normalized_hash":"synthetic-gate-hash","masked_value":"gat***608"}]'::jsonb)$$,
        :'gateItemId'
    ),
    '22023',
    'automatic talent publication gates failed',
    'automatic publication rejects malformed risk safety data'
);

update talent_import_items
set risk_reasons = '[]'::jsonb, duplicate_candidates = null
where item_id = :'gateItemId';

select throws_ok(
    format(
        $$select * from publish_talent_item(%L::uuid, 0, null, true, '{}'::jsonb, '[{"contact_type":"wechat","ciphertext":"7474","encryption_key_version":1,"normalized_hash":"synthetic-gate-hash","masked_value":"gat***608"}]'::jsonb)$$,
        :'gateItemId'
    ),
    '22023',
    'automatic talent publication gates failed',
    'automatic publication fails closed when duplicate safety data is null'
);

update talent_import_items
set duplicate_candidates = '[{"score":0.5,"high_similarity":false}]'::jsonb
where item_id = :'gateItemId';

select throws_ok(
    format(
        $$select * from publish_talent_item(%L::uuid, 0, null, true, '{}'::jsonb, '[{"contact_type":"wechat","ciphertext":"7474","encryption_key_version":1,"normalized_hash":"synthetic-gate-hash","masked_value":"gat***608"}]'::jsonb)$$,
        :'gateItemId'
    ),
    '22023',
    'automatic talent publication gates failed',
    'automatic publication rejects incomplete duplicate safety entries'
);

update talent_import_items set duplicate_candidates = '[]'::jsonb where item_id = :'gateItemId';
select updated_at as global_expected_updated_at
from talent_runtime_settings
where singleton
\gset

select set_talent_auto_publish(
    false,
    :'moderatorId',
    :'global_expected_updated_at'
) as paused_global_outcome
\gset

select is(
    :'paused_global_outcome'::jsonb->>'outcome',
    'updated',
    'global pause accepts the current runtime settings token'
);

select updated_at as global_paused_updated_at
from talent_runtime_settings
where singleton
\gset

select set_talent_auto_publish(
    true,
    :'secondModeratorId',
    :'global_expected_updated_at'
) as stale_global_outcome
\gset

select is(
    :'stale_global_outcome'::jsonb->>'outcome',
    'conflict',
    'global auto-publish rejects a stale runtime settings token'
);

select is(
    (select updated_at from talent_runtime_settings where singleton),
    :'global_paused_updated_at'::timestamptz,
    'stale global auto-publish leaves the resource token unchanged'
);

select is(
    (select count(*) from talent_review_events where resource_scope = 'runtime'),
    1::bigint,
    'a stale runtime control write creates no audit event'
);

select throws_ok(
    format(
        $$select * from publish_talent_item(%L::uuid, 0, null, true, '{}'::jsonb, '[{"contact_type":"wechat","ciphertext":"7474","encryption_key_version":1,"normalized_hash":"synthetic-gate-hash","masked_value":"gat***608"}]'::jsonb)$$,
        :'gateItemId'
    ),
    '22023',
    'automatic talent publication gates failed',
    'global pause blocks automatic publication'
);

select set_talent_auto_publish(
    true,
    :'moderatorId',
    (select updated_at from talent_runtime_settings where singleton)
) as resumed_global_outcome
\gset

select updated_at as global_resumed_updated_at
from talent_runtime_settings
where singleton
\gset

select set_talent_auto_publish(
    true,
    :'moderatorId',
    :'global_resumed_updated_at'
) as noop_global_outcome
\gset

select results_eq(
    $$
    select
        :'noop_global_outcome'::jsonb->>'outcome',
        updated_at = :'global_resumed_updated_at'::timestamptz,
        (select count(*) from talent_review_events where resource_scope = 'runtime')
    from talent_runtime_settings
    where singleton
    $$,
    $$ values ('updated'::text, true, 2::bigint) $$,
    'runtime no-op preserves the token and creates no duplicate audit event'
);

select results_eq(
    $$
    select
        reason,
        (before_snapshot->>'auto_publish_enabled')::boolean,
        (after_snapshot->>'auto_publish_enabled')::boolean,
        actor_user_id
    from talent_review_events
    where resource_scope = 'runtime'
    order by created_at, event_id
    $$,
    $$
    values
        ('auto_publish_paused'::text, true, false, '00000000-0000-0000-0000-000000000601'::uuid),
        ('auto_publish_resumed'::text, false, true, '00000000-0000-0000-0000-000000000601'::uuid)
    $$,
    'runtime audit history records privacy-safe before and after state with the actor'
);
update talent_sources set auto_publish_enabled = false where source_id = :'sourceId';

select throws_ok(
    format(
        $$select * from publish_talent_item(%L::uuid, 0, null, true, '{}'::jsonb, '[{"contact_type":"wechat","ciphertext":"7474","encryption_key_version":1,"normalized_hash":"synthetic-gate-hash","masked_value":"gat***608"}]'::jsonb)$$,
        :'gateItemId'
    ),
    '22023',
    'automatic talent publication gates failed',
    'source pause blocks automatic publication'
);

update talent_sources set auto_publish_enabled = true, confirmed_by = null, confirmed_at = null
where source_id = :'sourceId';

select throws_ok(
    format(
        $$select * from publish_talent_item(%L::uuid, 0, null, true, '{}'::jsonb, '[{"contact_type":"wechat","ciphertext":"7474","encryption_key_version":1,"normalized_hash":"synthetic-gate-hash","masked_value":"gat***608"}]'::jsonb)$$,
        :'gateItemId'
    ),
    '22023',
    'automatic talent publication gates failed',
    'unconfirmed source blocks automatic publication'
);

update talent_sources set confirmed_by = :'moderatorId', confirmed_at = current_timestamp
where source_id = :'sourceId';
update talent_import_items set extraction_confidence = 0.89 where item_id = :'gateItemId';

select throws_ok(
    format(
        $$select * from publish_talent_item(%L::uuid, 0, null, true, '{}'::jsonb, '[{"contact_type":"wechat","ciphertext":"7474","encryption_key_version":1,"normalized_hash":"synthetic-gate-hash","masked_value":"gat***608"}]'::jsonb)$$,
        :'gateItemId'
    ),
    '22023',
    'automatic talent publication gates failed',
    'confidence below threshold blocks automatic publication'
);

update talent_import_items
set
    extraction_confidence = 0.96,
    duplicate_candidates = '[{"candidate_item_id":"00000000-0000-0000-0000-000000000632","candidate_record_id":null,"score":0.90,"reasons":["synthetic_match"],"high_similarity":true}]'::jsonb
where item_id = :'gateItemId';

select throws_ok(
    format(
        $$select * from publish_talent_item(%L::uuid, 0, null, true, '{}'::jsonb, '[{"contact_type":"wechat","ciphertext":"7474","encryption_key_version":1,"normalized_hash":"synthetic-gate-hash","masked_value":"gat***608"}]'::jsonb)$$,
        :'gateItemId'
    ),
    '22023',
    'automatic talent publication gates failed',
    'high-similarity duplicate blocks automatic publication'
);

update talent_import_items set duplicate_candidates = '[]'::jsonb where item_id = :'gateItemId';

select throws_ok(
    format(
        $$select * from publish_talent_item(%L::uuid, 0, null, true, '{}'::jsonb, '[{"contact_type":"wechat","ciphertext":"7474","encryption_key_version":1,"normalized_hash":"synthetic-gate-hash","masked_value":"fictionalCompleteWechat608"}]'::jsonb)$$,
        :'gateItemId'
    ),
    '22023',
    'invalid masked talent publication contact',
    'unmasked contact display cannot enter publication'
);

select throws_ok(
    format(
        $$select merge_talent_import_item(%L::uuid, 0, %L::uuid, %L::uuid, 'fictional_complete_contact_608')$$,
        :'gateItemId',
        :'targetRecordId',
        :'moderatorId'
    ),
    '22023',
    'invalid talent audit reason',
    'merge audit reason rejects a plausible complete contact'
);

select throws_ok(
    format(
        $$select transition_talent_import_item(%L::uuid, 0, 'extracting', %L::uuid, 'fictional_complete_contact_609')$$,
        :'rejectedItemId',
        :'moderatorId'
    ),
    '22023',
    'invalid talent audit reason',
    'transition audit reason rejects a plausible complete contact'
);

select is(
    (select count(*) from talent_contacts where record_id = :'first_publish_record_id'),
    1::bigint,
    'publication inserts encrypted contacts atomically'
);

select results_eq(
    $$
    select actor_type, actor_user_id, action
    from talent_review_events
    where item_id = '00000000-0000-0000-0000-000000000631'::uuid
      and action = 'published'
    $$,
    $$
    values (
        'moderator'::text,
        '00000000-0000-0000-0000-000000000601'::uuid,
        'published'::text
    )
    $$,
    'manual publication records the Moderator'
);

select ok(
    (
        select before_snapshot::text not like '%fictional_complete_contact_601%'
           and after_snapshot::text not like '%fictional_complete_contact_601%'
        from talent_review_events
        where item_id = :'publishItemId'
          and action = 'published'
    ),
    'publication audit snapshots contain no complete contacts'
);

select throws_ok(
    format(
        $sql$
        select * from publish_talent_item(
            %L::uuid,
            0,
            null,
            true,
            jsonb_build_object(
                'kind', 'hiring',
                'title', 'High Risk Role',
                'city', 'Test City',
                'salary_min', 1000,
                'salary_max', 2000,
                'published_at', current_timestamp
            ),
            '[{"contact_type":"wechat","ciphertext":"6464","encryption_key_version":1,"normalized_hash":"synthetic-high-risk-hash","masked_value":"ris***602"}]'::jsonb
        )
        $sql$,
        :'highRiskItemId'
    ),
    '22023',
    'automatic talent publication gates failed',
    'high-risk items cannot use the automatic path'
);

select lives_ok(
    format(
        $$select merge_talent_import_item(%L::uuid, 0, %L::uuid, %L::uuid, 'duplicate')$$,
        :'mergedItemId',
        :'targetRecordId',
        :'moderatorId'
    ),
    'a review item can be merged atomically'
);

select results_eq(
    $$
    select
        i.status,
        e.record_id,
        e.action,
        (
            jsonb_array_length(i.duplicate_candidates) = 1
            and i.duplicate_candidates->0->'candidate_item_id' = 'null'::jsonb
            and i.duplicate_candidates->0->>'candidate_record_id'
                = '00000000-0000-0000-0000-000000000641'
            and (i.duplicate_candidates->0->>'score')::numeric = 1
            and i.duplicate_candidates->0->'reasons' = '["moderator_merge"]'::jsonb
            and (i.duplicate_candidates->0->>'high_similarity')::boolean
            and not (i.duplicate_candidates->0 ? 'merge_target')
            and (
                select count(*)
                from jsonb_object_keys(i.duplicate_candidates->0)
            ) = 5
        )
    from talent_import_items i
    join talent_review_events e using (item_id)
    where i.item_id = '00000000-0000-0000-0000-000000000633'::uuid
      and e.action = 'merged'
    $$,
    $$
    values (
        'merged'::text,
        '00000000-0000-0000-0000-000000000641'::uuid,
        'merged'::text,
        true
    )
    $$,
    'merge records its target and event in one transaction'
);

select throws_ok(
    format(
        $sql$
        select * from publish_talent_item(
            %L::uuid,
            0,
            '00000000-0000-0000-0000-000000009999'::uuid,
            false,
            '{"kind":"hiring","title":"Rollback Role","city":"Test City","salary_min":1000,"salary_max":2000}'::jsonb,
            '[{"contact_type":"wechat","ciphertext":"6868","encryption_key_version":1,"normalized_hash":"synthetic-rollback-hash","masked_value":"rol***605"}]'::jsonb
        )
        $sql$,
        :'rollbackItemId'
    ),
    '23503',
    null,
    'an audit-stage failure rolls back publication'
);

select is(
    (select count(*) from talent_records where title = 'Rollback Role'),
    0::bigint,
    'rollback leaves no formal record'
);

select updated_at as source_expected_updated_at
from talent_sources
where source_id = :'sourceId'
\gset

select set_talent_source_status(
    :'sourceId',
    'trusted',
    :'secondModeratorId',
    true,
    :'source_expected_updated_at'
)
as trusted_source_outcome
\gset

select is(
    :'trusted_source_outcome'::jsonb->>'outcome',
    'updated',
    'source trust accepts the current source token'
);

select updated_at as trusted_source_updated_at
from talent_sources
where source_id = :'sourceId'
\gset

select set_talent_source_status(
    :'sourceId',
    'blocked',
    :'moderatorId',
    false,
    :'source_expected_updated_at'
) as stale_source_outcome
\gset

select is(
    :'stale_source_outcome'::jsonb->>'outcome',
    'conflict',
    'source trust rejects a stale source token'
);

select is(
    (select updated_at from talent_sources where source_id = :'sourceId'),
    :'trusted_source_updated_at'::timestamptz,

select is(
    (select count(*) from talent_review_events where resource_scope = 'source'),
    1::bigint,
    'a stale source policy write creates no audit event'
);

select set_talent_source_status(
    :'sourceId',
    'trusted',
    :'secondModeratorId',
    true,
    :'trusted_source_updated_at'
) as noop_source_outcome
\gset

select results_eq(
    $$
    select
        :'noop_source_outcome'::jsonb->>'outcome',
        updated_at = :'trusted_source_updated_at'::timestamptz,
        (select count(*) from talent_review_events where resource_scope = 'source')
    from talent_sources
    where source_id = :'sourceId'::uuid
    $$,
    $$ values ('updated'::text, true, 1::bigint) $$,
    'source policy no-op preserves the token and creates no duplicate audit event'
);
    'stale source trust leaves the resource token unchanged'
);

select throws_ok(
    format(
        $$select set_talent_source_status(%L::uuid, 'trusted', null, true, %L::timestamptz)$$,
        :'sourceId',
        :'trusted_source_updated_at'
    ),
    '22023',
    'trusted talent source requires a Moderator confirmer',
    'a source cannot be confirmed without a Moderator'
);

select results_eq(
    $$
    select trust_status, confirmed_by, confirmed_at is not null, auto_publish_enabled
    from talent_sources
    where source_id = '00000000-0000-0000-0000-000000000611'::uuid
    $$,
    $$
    values (
        'trusted'::text,
        '00000000-0000-0000-0000-000000000602'::uuid,
        true,
        true
    )
    $$,
    'trust confirmation records the confirmer and time'
);

select * from set_talent_source_status(
    :'sourceId',
    'watchlist',
    :'secondModeratorId',
    true,
    (select updated_at from talent_sources where source_id = :'sourceId')
)
\gset

select is(
    (select auto_publish_enabled from talent_sources where source_id = :'sourceId'),
    false,
    'watchlist status disables source auto-publish'
);

select * from set_talent_source_status(
    :'sourceId',
    'blocked',
    :'secondModeratorId',
    true,
    (select updated_at from talent_sources where source_id = :'sourceId')
)
\gset

select is(
    (select auto_publish_enabled from talent_sources where source_id = :'sourceId'),
    false,
    'blocked status disables source auto-publish'
);

select is(
    (get_talent_runtime_settings()->>'auto_publish_enabled')::boolean,
    true,
    'runtime settings expose the global switch'
);

select is(
    (get_talent_review_item(:'publishItemId')->>'item_id')::uuid,
    :'publishItemId'::uuid,
    'review detail returns the requested item'
);

select * from set_talent_source_status(
    :'sourceId',
    'trusted',
    :'secondModeratorId',
    true,
    (select updated_at from talent_sources where source_id = :'sourceId')
);


select results_eq(
    $$
    select
        reason,
        before_snapshot->>'trust_status',
        after_snapshot->>'trust_status',
        (before_snapshot->>'auto_publish_enabled')::boolean,
        (after_snapshot->>'auto_publish_enabled')::boolean,
        actor_user_id
    from talent_review_events
    where resource_scope = 'source'
    order by created_at, event_id
    $$,
    $$
    values
        ('source_reconfirmed'::text, 'trusted'::text, 'trusted'::text, true, true,
            '00000000-0000-0000-0000-000000000602'::uuid),
        ('source_watchlisted'::text, 'trusted'::text, 'watchlist'::text, true, false,
            '00000000-0000-0000-0000-000000000602'::uuid),
        ('source_blocked'::text, 'watchlist'::text, 'blocked'::text, false, false,
            '00000000-0000-0000-0000-000000000602'::uuid),
        ('source_trusted'::text, 'blocked'::text, 'trusted'::text, false, true,
            '00000000-0000-0000-0000-000000000602'::uuid)
    $$,
    'source policy audit preserves the full scoped history'
);
insert into talent_import_items (
    item_id,
    batch_id,
    position,
    raw_fragment_ciphertext,
    encryption_key_version,
    contact_candidates_ciphertext,
    contact_hashes,
    contact_masked,
    normalized_fingerprint,
    kind,
    status,
    extraction,
    extraction_confidence,
    validation_errors,
    risk_score,
    risk_level,
    risk_reasons,
    duplicate_candidates,
    processed_at
)
values (
    '00000000-0000-0000-0000-000000000652',
    :'batchId',
    52,
    '8181',
    1,
    '8282',
    array['synthetic-auto-hash'],
    '[{"contact_type":"wechat","masked_value":"aut***652"}]'::jsonb,
    'synthetic-fingerprint-606',
    'seeking',
    'needs_review',
    '{"kind":"seeking","title":"Concurrent Automatic Candidate","city":"Test City","roles":["Synthetic Role"],"publishedAt":"2026-07-23T00:00:00Z","missingFields":[]}'::jsonb,
    0.96,
    '[]'::jsonb,
    5,
    'low',
    '[]'::jsonb,
    '[]'::jsonb,
    current_timestamp
);

select throws_ok(
    $$
    select * from publish_talent_item(
        '00000000-0000-0000-0000-000000000652'::uuid,
        0,
        null,
        true,
        '{}'::jsonb,
        '[{"contact_type":"wechat","ciphertext":"8282","encryption_key_version":1,"normalized_hash":"synthetic-auto-hash","masked_value":"aut***652"}]'::jsonb
    )
    $$,
    '22023',
    'automatic talent publication gates failed',
    'publication rechecks exact duplicates after serializing the dedupe key'
);

select lives_ok(
    format(
        $$select transition_talent_import_item(%L::uuid, 0, 'queued', %L::uuid, 'reprocess')$$,
        :'rejectedItemId',
        :'moderatorId'
    ),
    'reprocess transitions a reviewed item back to queued'
);

select results_eq(
    format(
        $$
        select item.status, batch.processed_count
        from talent_import_items item
        join talent_import_batches batch using (batch_id)
        where item.item_id = %L::uuid
        $$,
        :'rejectedItemId'
    ),
    $$ values ('queued'::text, 7::integer) $$,
    'reprocess restores the batch counter before another processing attempt'
);

select is(
    claim_talent_import_item()->>'item_id',
    :'rejectedItemId',
    'reprocessed queued item is claimed by the normal worker path'
);

insert into talent_import_items (
    item_id,
    batch_id,
    position,
    raw_fragment_ciphertext,
    encryption_key_version,
    kind,
    status,
    validation_errors,
    processed_at
)
values (
    :'failedReprocessItemId',
    :'batchId',
    53,
    '8383',
    1,
    'uncertain',
    'failed',
    '[{"code":"synthetic_failure"}]'::jsonb,
    current_timestamp
);

update talent_import_batches
set processed_count = 8, failed_count = 1, status = 'partially_failed'
where batch_id = :'batchId';

select results_eq(
    format(
        $$
        with transitioned as (
            select transition_talent_import_item(
                %L::uuid,
                0,
                'queued',
                %L::uuid,
                'reprocess'
            ) as saved
        )
        select item.status, batch.processed_count, batch.failed_count
        from transitioned
        join talent_import_items item on item.item_id = %L::uuid
        join talent_import_batches batch using (batch_id)
        where transitioned.saved
        $$,
        :'failedReprocessItemId',
        :'moderatorId',
        :'failedReprocessItemId'
    ),
    $$ values ('queued'::text, 7::integer, 0::integer) $$,
    'reprocessing a failed item also restores the batch failed counter'
);

select * from update_talent_review_item(
    :'gateItemId',
    0,
    '{"extraction":{"title":"Audit Revised Role"},"manual_policy_reasons":["source_not_trusted"]}'::jsonb,
    :'moderatorId'
update talent_import_items
set extraction = jsonb_set(extraction, '{title}', to_jsonb('13900000001'::text))
where item_id = :'gateItemId';

);

select results_eq(
    format(
        $$
        select item.lock_version, event.after_snapshot->'manual_policy_reasons'
        from talent_import_items item
        join talent_review_events event using (item_id)
        where item.item_id = %L::uuid
          and event.action = 'review_updated'
        order by event.created_at desc
        limit 1
        $$,
        :'gateItemId'
    ),
    $$ values (1::integer, '["source_not_trusted"]'::jsonb) $$,
    'manual policy override reasons are observable as fixed privacy-safe audit codes'
);

select * from finish();
rollback;

select ok(
    (
        select
            event.resource_scope = 'item'
            and event.before_snapshot ? 'review_state'
            and event.after_snapshot ? 'review_state'
            and event.before_snapshot->'review_state' <> event.after_snapshot->'review_state'
        from talent_review_events event
        where event.item_id = :'gateItemId'
          and event.action = 'review_updated'
        order by event.created_at desc, event.event_id desc
        limit 1
    ),
    'review audit stores the actual structured before and after states'
);

select ok(
    (
        select position('13900000001' in before_snapshot::text || after_snapshot::text) = 0
        from talent_review_events
        where item_id = :'gateItemId'
          and action = 'review_updated'
        order by created_at desc, event_id desc
        limit 1
    ),
    'review audit snapshots redact complete contacts from every structured field'
);

select throws_ok(
    $$update talent_review_events set action = 'tampered' where item_id is not null$$,
    '55000',
    'talent audit history is append-only',
    'audit events cannot be updated'
);

select throws_ok(
    $$delete from talent_review_events where item_id is not null$$,
    '55000',
    'talent audit history is append-only',
    'audit events cannot be deleted'
);

select throws_ok(
    format(
        $$insert into talent_review_events (resource_scope, item_id, actor_type, action)
          values ('runtime', %L::uuid, 'system', 'invalid_scope')$$,
        :'gateItemId'
    ),
    '23514',
    null,
    'audit event resource scope must match its bound identity'
);
