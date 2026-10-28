begin;
select plan(19);

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
        'storage_key', 'docs/0000000000000000000000000000000000000000000000000000000000000001.pdf',
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

select create_tender_document_job(
    jsonb_build_object(
        'user_id', :'owner',
        'project_id', :'project_id',
        'storage_key', 'docs/0000000000000000000000000000000000000000000000000000000000000001.pdf',
        'sha256', '0000000000000000000000000000000000000000000000000000000000000001',
        'size_bytes', 1234,
        'filename', 'tender.pdf',
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

select throws_ok(
    $$
    select create_tender_document_job(
        jsonb_build_object(
            'user_id', '00000000-0000-0000-0000-000000000602',
            'project_id', '00000000-0000-0000-0000-000000000000',
            'storage_key', 'docs/x.pdf',
            'sha256', '0000000000000000000000000000000000000000000000000000000000000002',
            'size_bytes', 1,
            'filename', 'x.pdf',
            'analysis_config_hash', 'config-v1',
            'model_provider', null,
            'model_name', null,
            'prompt_version', null
        )
    )
    $$,
    '42501',
    null,
    'document creation is rejected for a project the caller does not own'
);

select is(
    get_tender_analysis_job(:'other', :'job_id'::uuid),
    null,
    'another user cannot read the job'
);
select is(
    list_tender_projects(:'other'),
    '[]'::jsonb,
    'another user lists no projects'
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
select is(
    jsonb_array_length(list_tender_projects(:'owner')),
    1,
    'the owner lists their single project'
);

select * from finish();
rollback;
