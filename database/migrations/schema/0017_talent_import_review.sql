create table talent_sources (
    source_id uuid primary key default gen_random_uuid(),
    source_type text not null check (source_type in ('wechat_group', 'qq_group', 'web', 'other')),
    name text not null check (name <> ''),
    trust_status text not null default 'new' check (trust_status in ('new', 'trusted', 'watchlist', 'blocked')),
    auto_publish_enabled boolean not null default false,
    published_count integer not null default 0 check (published_count >= 0),
    reported_count integer not null default 0 check (reported_count >= 0),
    withdrawn_count integer not null default 0 check (withdrawn_count >= 0),
    corrected_count integer not null default 0 check (corrected_count >= 0),
    created_by uuid references "user" (user_id) on delete set null,
    created_at timestamptz not null default current_timestamp,
    confirmed_by uuid references "user" (user_id) on delete set null,
    confirmed_at timestamptz,
    updated_at timestamptz not null default current_timestamp
);

create unique index talent_source_type_name_idx
on talent_sources (source_type, lower(name));

create table talent_import_batches (
    batch_id uuid primary key default gen_random_uuid(),
    source_id uuid not null references talent_sources (source_id) on delete restrict,
    imported_by uuid references "user" (user_id) on delete set null,
    raw_text_ciphertext text,
    encryption_key_version integer check (encryption_key_version >= 1),
    status text not null default 'queued' check (status in ('queued', 'processing', 'completed', 'partially_failed', 'failed')),
    model_provider text,
    model_name text,
    prompt_version text,
    item_count integer not null default 0 check (item_count >= 0),
    processed_count integer not null default 0 check (processed_count >= 0),
    failed_count integer not null default 0 check (failed_count >= 0),
    created_at timestamptz not null default current_timestamp,
    completed_at timestamptz,
    raw_delete_after timestamptz,
    raw_deleted_at timestamptz
);

create table talent_records (
    record_id uuid primary key default gen_random_uuid(),
    kind text not null check (kind in ('hiring', 'seeking')),
    title text not null check (title <> ''),
    scene text,
    region text,
    city text,
    district text,
    salary_min integer check (salary_min >= 0),
    salary_max integer check (salary_max >= 0 and (salary_min is null or salary_max >= salary_min)),
    roles text[],
    skills text[],
    experience_years integer check (experience_years >= 0),
    hours_per_day integer check (hours_per_day >= 0),
    rest_days_per_month integer check (rest_days_per_month >= 0),
    lodging boolean,
    housing_allowance boolean,
    availability text,
    urgency text,
    source_id uuid references talent_sources (source_id) on delete set null,
    source_label text,
    published_at timestamptz,
    expires_at timestamptz,
    status text not null check (status in ('published', 'archived', 'withdrawn')),
    creation_source text not null check (creation_source in ('manual_review', 'auto_publish')),
    created_at timestamptz not null default current_timestamp,
    updated_at timestamptz not null default current_timestamp,
    withdrawn_at timestamptz
);

create table talent_import_items (
    item_id uuid primary key default gen_random_uuid(),
    batch_id uuid not null references talent_import_batches (batch_id) on delete restrict,
    position integer not null check (position >= 0),
    raw_fragment_ciphertext text,
    encryption_key_version integer check (encryption_key_version >= 1),
    contact_candidates_ciphertext text,
    contact_hashes text[],
    contact_masked jsonb,
    normalized_fingerprint text,
    kind text not null check (kind in ('hiring', 'seeking', 'irrelevant', 'uncertain')),
    status text not null default 'queued' check (status in ('queued', 'extracting', 'needs_review', 'auto_published', 'published', 'rejected', 'merged', 'failed')),
    extraction jsonb,
    extraction_confidence numeric check (extraction_confidence >= 0 and extraction_confidence <= 1),
    validation_errors jsonb,
    risk_score numeric check (risk_score >= 0 and risk_score <= 100),
    risk_level text check (risk_level in ('low', 'medium', 'high')),
    risk_reasons jsonb,
    duplicate_candidates jsonb,
    published_record_id uuid references talent_records (record_id) on delete set null,
    lock_version integer not null default 0 check (lock_version >= 0),
    created_at timestamptz not null default current_timestamp,
    processed_at timestamptz,
    reviewed_at timestamptz,
    updated_at timestamptz not null default current_timestamp
);

create unique index talent_import_item_batch_position_idx
on talent_import_items (batch_id, position);

create unique index talent_import_item_published_record_idx
on talent_import_items (published_record_id)
where published_record_id is not null;

create table talent_contacts (
    contact_id uuid primary key default gen_random_uuid(),
    record_id uuid not null references talent_records (record_id) on delete restrict,
    contact_type text not null check (contact_type in ('phone', 'wechat', 'qq', 'other')),
    ciphertext text,
    encryption_key_version integer check (encryption_key_version >= 1),
    normalized_hash text,
    masked_value text,
    created_at timestamptz not null default current_timestamp,
    deleted_at timestamptz
);

create table talent_review_events (
    event_id uuid primary key default gen_random_uuid(),
    item_id uuid not null references talent_import_items (item_id) on delete restrict,
    record_id uuid references talent_records (record_id) on delete set null,
    actor_type text not null check (actor_type in ('moderator', 'system')),
    actor_user_id uuid references "user" (user_id) on delete set null,
    action text not null check (action <> ''),
    reason text,
    before_snapshot jsonb,
    after_snapshot jsonb,
    model_provider text,
    model_name text,
    prompt_version text,
    rule_version text,
    policy_version text,
    automatic_reason text,
    created_at timestamptz not null default current_timestamp
);

comment on column talent_review_events.before_snapshot is
    'Complete contacts are forbidden in this JSONB audit snapshot.';
comment on column talent_review_events.after_snapshot is
    'Complete contacts are forbidden in this JSONB audit snapshot.';

create table talent_runtime_settings (
    singleton boolean primary key default true constraint talent_runtime_settings_singleton_check check (singleton),
    auto_publish_enabled boolean not null default false,
    updated_by uuid references "user" (user_id) on delete set null,
    updated_at timestamptz not null default current_timestamp
);

insert into talent_runtime_settings (singleton) values (true);

---- create above / drop below ----

drop table talent_runtime_settings;
drop table talent_review_events;
drop table talent_contacts;
drop table talent_import_items;
drop table talent_records;
drop table talent_import_batches;
drop table talent_sources;