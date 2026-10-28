create table tender_projects (
    project_id uuid primary key default gen_random_uuid(),
    user_id uuid not null references "user" (user_id) on delete cascade,
    title text not null check (title <> ''),
    source_url text,
    created_at timestamptz not null default current_timestamp,
    updated_at timestamptz not null default current_timestamp
);

create table tender_documents (
    document_id uuid primary key default gen_random_uuid(),
    project_id uuid not null references tender_projects (project_id) on delete cascade,
    storage_key text not null check (
        storage_key <> ''
        and storage_key !~ '^[/\\]'
        and storage_key !~ '^[A-Za-z]:'
    ),
    sha256 text not null check (sha256 ~ '^[0-9a-f]{64}$'),
    size_bytes bigint not null check (size_bytes between 0 and 52428800),
    filename text,
    uploaded_at timestamptz not null default current_timestamp
);

create unique index tender_documents_project_sha256_idx
on tender_documents (project_id, sha256);

create table tender_analysis_jobs (
    job_id uuid primary key default gen_random_uuid(),
    document_id uuid not null references tender_documents (document_id) on delete cascade,
    analysis_config_hash text not null check (analysis_config_hash <> ''),
    status text not null default 'queued' check (
        status in ('queued', 'parsing', 'analyzing', 'partial', 'completed', 'failed')
    ),
    claim_token uuid,
    claim_version bigint not null default 0 check (claim_version >= 0),
    claim_expires_at timestamptz,
    attempt_count integer not null default 0 check (attempt_count >= 0),
    error_code text,
    error_message text,
    model_provider text,
    model_name text,
    prompt_version text,
    created_at timestamptz not null default current_timestamp,
    updated_at timestamptz not null default current_timestamp,
    completed_at timestamptz
);

create unique index tender_analysis_jobs_active_config_idx
on tender_analysis_jobs (document_id, analysis_config_hash)
where status in ('queued', 'parsing', 'analyzing');

create table tender_analysis_reports (
    report_id uuid primary key default gen_random_uuid(),
    job_id uuid not null unique references tender_analysis_jobs (job_id) on delete cascade,
    document_id uuid not null references tender_documents (document_id) on delete cascade,
    analysis_config_hash text not null check (analysis_config_hash <> ''),
    result jsonb not null check (jsonb_typeof(result) = 'object'),
    contract_version text not null check (contract_version = 'tender-analysis.v1'),
    created_at timestamptz not null default current_timestamp
);

create unique index tender_analysis_reports_document_config_idx
on tender_analysis_reports (document_id, analysis_config_hash);

---- create above / drop below ----

drop table tender_analysis_reports;
drop table tender_analysis_jobs;
drop table tender_documents;
drop table tender_projects;
