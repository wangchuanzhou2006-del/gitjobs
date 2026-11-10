begin;
select plan(39);

\set owner '00000000-0000-0000-0000-000000000601'
\set other '00000000-0000-0000-0000-000000000602'

insert into "user" (auth_hash, email, name, user_id, username)
values
    (decode('02', 'hex'), 'tender-owner@example.com', 'Tender Owner', :'owner', 'tender_owner'),
    (decode('03', 'hex'), 'tender-other@example.com', 'Tender Other', :'other', 'tender_other');

select create_tender_project(
    jsonb_build_object(
        'user_id', :'owner',
        'title', '某市第三中学食堂承包服务项目',
        'source_url', null
    )
) as project_id
\gset

select is(
    (select title from tender_projects where project_id = :'project_id'::uuid),
    '某市第三中学食堂承包服务项目',
    'create_tender_project stores the owned title'
);

select create_tender_document_job(
    jsonb_build_object(
        'user_id', :'owner',
        'project_id', :'project_id',
        'storage_key', 'docs/key1.pdf',
        'sha256', '0000000000000000000000000000000000000000000000000000000000000001',
        'size_bytes', 1234,
        'filename', 'tender.pdf',
        'analysis_config_hash', 'config-v1',
        'model_provider', 'openai',
        'model_name', 'gpt-4o',
        'prompt_version', 'tender-project-info-v1'
    )
) as job_result
\gset

select jsonb_extract_path_text(:'job_result'::jsonb, 'job_id') as job_id
\gset

select is(
    jsonb_extract_path_text(:'job_result'::jsonb, 'created'),
    'true',
    'first document job creation reports created=true'
);
select is(
    jsonb_extract_path_text(:'job_result'::jsonb, 'document_created'),
    'true',
    'first document upload reports document_created=true'
);

select create_tender_document_job(
    jsonb_build_object(
        'user_id', :'owner',
        'project_id', :'project_id',
        'storage_key', 'docs/key2.pdf',
        'sha256', '0000000000000000000000000000000000000000000000000000000000000001',
        'size_bytes', 9999,
        'filename', 'other.pdf',
        'analysis_config_hash', 'config-v1',
        'model_provider', 'openai',
        'model_name', 'gpt-4o',
        'prompt_version', 'tender-project-info-v1'
    )
) as reused_job
\gset

select is(
    jsonb_extract_path_text(:'reused_job'::jsonb, 'job_id'),
    :'job_id',
    'duplicate document job reuses the existing active job'
);
select is(
    jsonb_extract_path_text(:'reused_job'::jsonb, 'created'),
    'false',
    'duplicate document job reports created=false'
);
select is(
    jsonb_extract_path_text(:'reused_job'::jsonb, 'document_created'),
    'false',
    'duplicate upload reports document_created=false'
);

select is(
    (select storage_key from tender_documents where document_id = (
        select document_id from tender_analysis_jobs where job_id = :'job_id'::uuid
    )),
    'docs/key1.pdf',
    'duplicate upload keeps the original storage key'
);
select is(
    (select size_bytes from tender_documents where document_id = (
        select document_id from tender_analysis_jobs where job_id = :'job_id'::uuid
    )),
    1234::bigint,
    'duplicate upload keeps the original size'
);
select is(
    (select filename from tender_documents where document_id = (
        select document_id from tender_analysis_jobs where job_id = :'job_id'::uuid
    )),
    'tender.pdf',
    'duplicate upload keeps the original filename'
);

select claim_tender_analysis_job() as claimed_job
\gset

select jsonb_extract_path_text(:'claimed_job'::jsonb, 'claim_token') as claim_token,
       jsonb_extract_path_text(:'claimed_job'::jsonb, 'claim_version') as claim_version
\gset

select is(
    jsonb_extract_path_text(:'claimed_job'::jsonb, 'job_id'),
    :'job_id',
    'claim selects the queued job'
);
select is(
    jsonb_extract_path_text(:'claimed_job'::jsonb, 'status'),
    'parsing',
    'claim moves the queued job to parsing'
);
select is(
    claim_tender_analysis_job(),
    null,
    'a claimed job is not claimable again until its lease expires'
);

select throws_ok(
    format(
        $sql$
        select transition_tender_analysis_job(
            jsonb_build_object(
                'job_id', %L::uuid,
                'claim_token', '00000000-0000-0000-0000-000000000000',
                'claim_version', %L::bigint,
                'next_status', 'analyzing'
            )
        )
        $sql$,
        :'job_id',
        :'claim_version'
    ),
    '40001',
    null,
    'transition with a wrong claim token is rejected as stale'
);

select transition_tender_analysis_job(
    jsonb_build_object(
        'job_id', :'job_id'::uuid,
        'claim_token', :'claim_token'::uuid,
        'claim_version', :'claim_version'::bigint,
        'next_status', 'analyzing'
    )
);

select is(
    (select status from tender_analysis_jobs where job_id = :'job_id'::uuid),
    'analyzing',
    'a valid claim advances parsing to analyzing'
);

select throws_ok(
    format(
        $sql$
        select transition_tender_analysis_job(
            jsonb_build_object(
                'job_id', %L::uuid,
                'claim_token', %L::uuid,
                'claim_version', %L::bigint,
                'next_status', 'parsing'
            )
        )
        $sql$,
        :'job_id',
        :'claim_token',
        :'claim_version'
    ),
    '22023',
    null,
    'transition rejects an invalid next status'
);

select throws_ok(
    $$
    select complete_tender_analysis_job(
        jsonb_build_object(
            'job_id', '00000000-0000-0000-0000-000000000000',
            'claim_token', '00000000-0000-0000-0000-000000000000',
            'claim_version', 1,
            'status', 'completed',
            'result', jsonb_build_object('schema_version', 'tender-analysis.v1')
        )
    )
    $$,
    '22023',
    null,
    'completion rejects a result missing top-level fields'
);

select complete_tender_analysis_job(
    jsonb_build_object(
        'job_id', :'job_id'::uuid,
        'claim_token', :'claim_token'::uuid,
        'claim_version', :'claim_version'::bigint,
        'status', 'completed',
        'result', jsonb_build_object(
            'schema_version', 'tender-analysis.v1',
            'project_info', jsonb_build_object(),
            'qualifications', jsonb_build_array(),
            'risks', jsonb_build_array(),
            'required_materials', jsonb_build_array(),
            'unknowns', jsonb_build_array(),
            'analysis_meta', jsonb_build_object()
        )
    )
) as report_id
\gset

select is(
    (select status from tender_analysis_jobs where job_id = :'job_id'::uuid),
    'completed',
    'completion marks the job completed'
);
select is(
    (select contract_version from tender_analysis_reports where report_id = :'report_id'::uuid),
    'tender-analysis.v1',
    'stored report records the tender-analysis.v1 contract version'
);
select isnt(
    get_tender_report(:'owner', :'report_id'::uuid),
    null,
    'the owner can read the completed report'
);
select is(
    get_tender_report(:'other', :'report_id'::uuid),
    null,
    'another user cannot read the report'
);

select create_tender_document_job(
    jsonb_build_object(
        'user_id', :'owner',
        'project_id', :'project_id',
        'storage_key', 'docs/key1.pdf',
        'sha256', '0000000000000000000000000000000000000000000000000000000000000001',
        'size_bytes', 1234,
        'filename', 'tender.pdf',
        'analysis_config_hash', 'config-v1',
        'model_provider', 'openai',
        'model_name', 'gpt-4o',
        'prompt_version', 'tender-project-info-v1'
    )
) as completed_reuse
\gset

select is(
    jsonb_extract_path_text(:'completed_reuse'::jsonb, 'job_id'),
    :'job_id',
    're-creating a completed job reuses the existing result'
);
select is(
    jsonb_extract_path_text(:'completed_reuse'::jsonb, 'created'),
    'false',
    're-creating a completed job does not create a duplicate'
);
select is(
    jsonb_array_length(list_tender_projects(:'owner')),
    1,
    'the owner lists their single project'
);

-- The reusable-config unique index must reject a second non-failed job for the
-- same document and config. This is the database-level invariant that closes
-- the completion race: even if a create's lookup and insert interleave with a
-- completion, no active job can coexist with a completed/partial job.
select throws_ok(
    format(
        $sql$
        insert into tender_analysis_jobs (document_id, analysis_config_hash, status)
        values (
            (select document_id from tender_analysis_jobs where job_id = %L::uuid),
            'config-v1',
            'queued'
        )
        $sql$,
        :'job_id'
    ),
    '23505',
    null,
    'the reusable-config unique index rejects a second non-failed job'
);

-- Part B: failed job, error-code whitelist, and expired-claim fencing.

select create_tender_document_job(
    jsonb_build_object(
        'user_id', :'owner',
        'project_id', :'project_id',
        'storage_key', 'docs/key2.pdf',
        'sha256', '0000000000000000000000000000000000000000000000000000000000000002',
        'size_bytes', 10,
        'filename', 'second.pdf',
        'analysis_config_hash', 'config-v2',
        'model_provider', 'openai',
        'model_name', 'gpt-4o',
        'prompt_version', 'tender-project-info-v1'
    )
) as job2_result
\gset

select jsonb_extract_path_text(:'job2_result'::jsonb, 'job_id') as job2_id
\gset

select claim_tender_analysis_job() as claimed2
\gset

select jsonb_extract_path_text(:'claimed2'::jsonb, 'claim_token') as claim_token2,
       jsonb_extract_path_text(:'claimed2'::jsonb, 'claim_version') as claim_version2
\gset

select transition_tender_analysis_job(
    jsonb_build_object(
        'job_id', :'job2_id'::uuid,
        'claim_token', :'claim_token2'::uuid,
        'claim_version', :'claim_version2'::bigint,
        'next_status', 'analyzing'
    )
);

update tender_analysis_jobs
set claim_expires_at = current_timestamp - interval '1 second'
where job_id = :'job2_id'::uuid;

select claim_tender_analysis_job() as reclaimed2
\gset

select jsonb_extract_path_text(:'reclaimed2'::jsonb, 'claim_token') as claim_token3,
       jsonb_extract_path_text(:'reclaimed2'::jsonb, 'claim_version') as claim_version3
\gset

select is(
    jsonb_extract_path_text(:'reclaimed2'::jsonb, 'job_id'),
    :'job2_id',
    'an expired in-flight job can be reclaimed'
);
select is(
    :'claim_version3'::bigint,
    (:'claim_version2'::bigint + 1),
    're-claim increments the claim version'
);
select isnt(
    :'claim_token3'::uuid,
    :'claim_token2'::uuid,
    're-claim issues a fresh claim token'
);

select throws_ok(
    format(
        $sql$
        select complete_tender_analysis_job(
            jsonb_build_object(
                'job_id', %L::uuid,
                'claim_token', %L::uuid,
                'claim_version', %L::bigint,
                'status', 'completed',
                'result', jsonb_build_object(
                    'schema_version', 'tender-analysis.v1',
                    'project_info', jsonb_build_object(),
                    'qualifications', jsonb_build_array(),
                    'risks', jsonb_build_array(),
                    'required_materials', jsonb_build_array(),
                    'unknowns', jsonb_build_array(),
                    'analysis_meta', jsonb_build_object()
                )
            )
        )
        $sql$,
        :'job2_id',
        :'claim_token2',
        :'claim_version2'
    ),
    '40001',
    null,
    'an expired claim cannot complete the job'
);

select throws_ok(
    format(
        $sql$
        select fail_tender_analysis_job(
            jsonb_build_object(
                'job_id', %L::uuid,
                'claim_token', %L::uuid,
                'claim_version', %L::bigint,
                'error_code', 'analysis_unavailable'
            )
        )
        $sql$,
        :'job2_id',
        :'claim_token2',
        :'claim_version2'
    ),
    '40001',
    null,
    'an expired claim cannot fail the job'
);

-- A missing, null, non-string, or non-whitelisted error code is rejected with
-- 22023 even when the job and claim are valid, and leaves the job untouched.
select throws_ok(
    format(
        $sql$
        select fail_tender_analysis_job(
            jsonb_build_object(
                'job_id', %L::uuid,
                'claim_token', %L::uuid,
                'claim_version', %L::bigint
            )
        )
        $sql$,
        :'job2_id',
        :'claim_token3',
        :'claim_version3'
    ),
    '22023',
    null,
    'fail rejects a missing error code on a real in-flight job'
);
select is(
    (select status from tender_analysis_jobs where job_id = :'job2_id'::uuid),
    'analyzing',
    'a rejected fail leaves the job status unchanged'
);
select is(
    (select claim_token from tender_analysis_jobs where job_id = :'job2_id'::uuid),
    :'claim_token3'::uuid,
    'a rejected fail leaves the claim token unchanged'
);
select is(
    (select claim_version from tender_analysis_jobs where job_id = :'job2_id'::uuid),
    :'claim_version3'::bigint,
    'a rejected fail leaves the claim version unchanged'
);

select throws_ok(
    format(
        $sql$
        select fail_tender_analysis_job(
            jsonb_build_object(
                'job_id', %L::uuid,
                'claim_token', %L::uuid,
                'claim_version', %L::bigint,
                'error_code', null
            )
        )
        $sql$,
        :'job2_id',
        :'claim_token3',
        :'claim_version3'
    ),
    '22023',
    null,
    'fail rejects a null error code on a real in-flight job'
);

select throws_ok(
    format(
        $sql$
        select fail_tender_analysis_job(
            jsonb_build_object(
                'job_id', %L::uuid,
                'claim_token', %L::uuid,
                'claim_version', %L::bigint,
                'error_code', 123
            )
        )
        $sql$,
        :'job2_id',
        :'claim_token3',
        :'claim_version3'
    ),
    '22023',
    null,
    'fail rejects a non-string error code on a real in-flight job'
);

select throws_ok(
    format(
        $sql$
        select fail_tender_analysis_job(
            jsonb_build_object(
                'job_id', %L::uuid,
                'claim_token', %L::uuid,
                'claim_version', %L::bigint,
                'error_code', 'arbitrary_sentinel'
            )
        )
        $sql$,
        :'job2_id',
        :'claim_token3',
        :'claim_version3'
    ),
    '22023',
    null,
    'fail rejects a non-whitelisted error code on a real in-flight job'
);

select fail_tender_analysis_job(
    jsonb_build_object(
        'job_id', :'job2_id'::uuid,
        'claim_token', :'claim_token3'::uuid,
        'claim_version', :'claim_version3'::bigint,
        'error_code', 'analysis_unavailable',
        'error_message', 'TOP_SECRET_SENTINEL'
    )
);

select is(
    (select status from tender_analysis_jobs where job_id = :'job2_id'::uuid),
    'failed',
    'a valid claim fails the job'
);
select is(
    (select error_code from tender_analysis_jobs where job_id = :'job2_id'::uuid),
    'analysis_unavailable',
    'fail stores only the approved error code'
);
select is(
    (get_tender_analysis_job(:'owner', :'job2_id'::uuid))::text like '%TOP_SECRET_SENTINEL%',
    false,
    'fail never persists caller error text'
);

select * from finish();
rollback;
