create extension if not exists pgcrypto;
create extension if not exists postgis;
create extension if not exists pg_trgm;

create or replace function i_array_to_string(text[], text)
returns text language sql immutable as $$select array_to_string($1, $2)$$;

create table if not exists foundation (
    foundation_id uuid primary key default gen_random_uuid(),
    name text not null check (name <> '') unique
);

insert into foundation (name) values ('cncf');

create table member (
    member_id uuid primary key default gen_random_uuid(),
    foundation text not null references foundation (name) on delete restrict,

    name text not null check (name <> ''),
    level text not null check (level <> ''),
    logo_url text not null check (logo_url <> ''),

    unique (name, foundation)
);

create index member_foundation_idx on member (foundation);

create table project (
    project_id uuid primary key default gen_random_uuid(),
    foundation text not null references foundation (name) on delete restrict,

    name text not null check (name <> ''),
    maturity text not null check (maturity <> ''),
    logo_url text not null check (logo_url <> ''),

    unique (name, foundation)
);

create index project_foundation_idx on project (foundation);

create table location (
    location_id uuid primary key,

    city text not null check (city <> ''),
    country text not null check (country <> ''),
    tsdoc tsvector not null
        generated always as (
            setweight(to_tsvector('simple', city), 'A') ||
            setweight(to_tsvector('simple', country), 'B') ||
            setweight(to_tsvector('simple', coalesce(state, '')), 'B')
        ) stored,

    coordinates geography(point, 4326),
    state text check (state <> '')
);

create index location_coordinates_idx on location using gist (coordinates);
create index location_tsdoc_idx on location using gin (tsdoc);

create table "user" (
    user_id uuid primary key default gen_random_uuid(),

    auth_hash bytea not null check (auth_hash <> ''),
    created_at timestamptz not null default current_timestamp,
    email text not null check (email <> '') unique,
    email_verified boolean not null default false,
    name text not null check (name <> ''),
    username text not null check (username <> '') unique,

    password text check (password <> '')
);

create table image (
    image_id uuid not null primary key,
    created_by uuid references "user" on delete set null
);

create index image_created_by_idx on image (created_by);

create table image_version (
    image_id uuid not null references image on delete cascade,
    version text not null check (version <> ''),
    data bytea not null,
    primary key (image_id, version)
);

create table job_seeker_profile (
    job_seeker_profile_id uuid primary key default gen_random_uuid(),
    user_id uuid not null unique references "user" on delete cascade,
    location_id uuid references location on delete set null,
    photo_id uuid references image (image_id) on delete set null,

    email text not null check (email <> ''),
    name text not null check (name <> ''),
    public boolean not null default false,
    summary text not null check (summary <> ''),

    certifications jsonb,
    education jsonb,
    experience jsonb,
    facebook_url text check (facebook_url <> ''),
    github_url text check (github_url <> ''),
    linkedin_url text check (linkedin_url <> ''),
    open_to_relocation boolean,
    open_to_remote boolean,
    phone text check (phone <> ''),
    projects jsonb,
    resume_url text check (resume_url <> ''),
    skills text[],
    twitter_url text check (twitter_url <> ''),
    website_url text check (website_url <> '')
);

create index job_seeker_profile_location_id_idx on job_seeker_profile (location_id);
create index job_seeker_profile_photo_id_idx on job_seeker_profile (photo_id);
create index job_seeker_profile_user_id_idx on job_seeker_profile (user_id);

create table employer (
    employer_id uuid primary key default gen_random_uuid(),
    location_id uuid references location on delete set null,
    logo_id uuid references image (image_id) on delete set null,
    member_id uuid references member on delete set null,

    company text not null check (company <> ''),
    created_at timestamptz not null default current_timestamp,
    description text not null check (description <> ''),
    public boolean not null default false,

    updated_at timestamptz,
    website_url text check (website_url <> '')
);

create index employer_member_id_idx on employer (member_id);
create index employer_location_id_idx on employer (location_id);
create index employer_logo_id_idx on employer (logo_id);

create table employer_team (
    employer_id uuid not null references employer on delete cascade,
    user_id uuid not null references "user" on delete cascade,

    primary key (employer_id, user_id)
);

create index employer_team_employer_id_idx on employer_team (employer_id);
create index employer_team_user_id_idx on employer_team (user_id);

create table job_kind (
    job_kind_id uuid primary key default gen_random_uuid(),

    name text not null unique check (name <> '')
);

insert into job_kind (name) values ('full-time');
insert into job_kind (name) values ('part-time');
insert into job_kind (name) values ('contractor');
insert into job_kind (name) values ('internship');

create table job_status (
    job_status_id uuid primary key default gen_random_uuid(),

    name text not null unique check (name <> '')
);

insert into job_status (name) values ('archived');
insert into job_status (name) values ('draft');
insert into job_status (name) values ('published');

create table seniority (
    seniority_id uuid primary key default gen_random_uuid(),

    name text not null unique check (name <> '')
);

insert into seniority (name) values ('entry');
insert into seniority (name) values ('junior');
insert into seniority (name) values ('mid');
insert into seniority (name) values ('senior');
insert into seniority (name) values ('lead');

create table workplace (
    workplace_id uuid primary key default gen_random_uuid(),

    name text not null unique check (name <> '')
);

insert into workplace (name) values ('hybrid');
insert into workplace (name) values ('on-site');
insert into workplace (name) values ('remote');

create table job (
    job_id uuid primary key default gen_random_uuid(),
    employer_id uuid not null references employer on delete cascade,
    kind text not null references job_kind (name) on delete restrict,
    seniority text references seniority (name) on delete restrict,
    status text not null references job_status (name) on delete restrict,
    location_id uuid references location on delete set null,
    workplace text not null references workplace (name) on delete restrict,

    created_at timestamptz not null default current_timestamp,
    description text not null check (description <> ''),
    title text not null check (title <> ''),
    tsdoc tsvector not null
        generated always as (
            setweight(to_tsvector('simple', title), 'A') ||
            setweight(to_tsvector('simple', i_array_to_string(coalesce(skills, '{}'), ' ')), 'B') ||
            setweight(to_tsvector('simple', description), 'C')
        ) stored,

    apply_instructions text check (apply_instructions <> ''),
    apply_url text check (apply_url <> ''),
    archived_at timestamptz,
    benefits text[],
    open_source int check (open_source >= 0 and open_source <= 100),
    published_at timestamptz,
    qualifications text check (qualifications <> ''),
    responsibilities text check (responsibilities <> ''),
    salary bigint check (salary >= 0),
    salary_currency text check (salary_currency <> ''),
    salary_max bigint check (salary_max >= 0),
    salary_min bigint check (salary_min >= 0),
    salary_period text check (salary_period <> ''),
    skills text[],
    updated_at timestamptz,
    upstream_commitment int check (upstream_commitment >= 0 and upstream_commitment <= 100)
);

create index job_employer_id_idx on job (employer_id);
create index job_kind_idx on job (kind);
create index job_location_id_idx on job (location_id);
create index job_status_idx on job (status);
create index job_workplace_idx on job (workplace);

create table job_project (
    job_id uuid not null references job on delete cascade,
    project_id uuid not null references project on delete cascade,

    primary key (job_id, project_id)
);

create table application (
    application_id uuid primary key default gen_random_uuid(),
    job_seeker_profile_id uuid not null references job_seeker_profile on delete cascade,
    job_id uuid not null references job on delete cascade,

    created_at timestamptz not null default current_timestamp,

    cover_letter text check (cover_letter <> ''),
    updated_at timestamptz
);

create index application_job_seeker_profile_id_idx on application (job_seeker_profile_id);
create index application_job_id_idx on application (job_id);

create table faq (
    faq_id uuid primary key default gen_random_uuid(),

    answer text not null check (answer <> ''),
    question text not null check (question <> '')
);

create table session (
    session_id text primary key,

    data jsonb not null,
    expires_at timestamptz not null
);

create table if not exists email_verification_code (
    email_verification_code_id uuid primary key default gen_random_uuid(),
    user_id uuid not null unique references "user" on delete cascade,
    created_at timestamptz default current_timestamp not null
);

create index email_verification_code_user_id_index on email_verification_code(user_id);

create table notification_kind (
    notification_kind_id uuid primary key default gen_random_uuid(),

    name text not null unique check (name <> '')
);

insert into notification_kind (name) values ('email-verification');

create table notification (
    notification_id uuid primary key default gen_random_uuid(),
    kind text not null references notification_kind (name) on delete restrict,
    user_id uuid not null unique references "user" on delete cascade,
    processed boolean not null default false,
    created_at timestamptz default current_timestamp not null,

    error text check (error <> ''),
    processed_at timestamptz,
    template_data jsonb
);

create index notification_not_processed_idx on notification (notification_id) where processed = 'false';
create index notification_kind_idx on notification(kind);
create index notification_user_id_idx on notification(user_id);

alter table application add constraint application_job_seeker_profile_id_job_id_key unique (job_seeker_profile_id, job_id);


alter table job add column tz_start text;
alter table job add column tz_end text;


alter table job add column salary_usd_year bigint check (salary_usd_year >= 0);
alter table job add column salary_min_usd_year bigint check (salary_min_usd_year >= 0);
alter table job add column salary_max_usd_year bigint check (salary_max_usd_year >= 0);


insert into job_status (name) values ('pending-approval');
insert into job_status (name) values ('rejected');

alter table "user" add moderator boolean not null default false;

alter table job add review_notes text;
alter table job add reviewed_by uuid references "user" (user_id) on delete set null;
alter table job add reviewed_at timestamptz;

create index job_reviewed_by_idx on job (reviewed_by);


alter table job_seeker_profile add column bluesky_url text check (bluesky_url <> '');


alter table foundation add column landscape_url text check (landscape_url <> '');


alter table employer_team add column approved boolean not null default false;
update employer_team set approved = true;
alter table employer_team add column created_at timestamptz default current_timestamp;

alter table notification drop constraint notification_user_id_key;

insert into notification_kind (name) values ('team-invitation');


create table if not exists job_views (
    job_id uuid references job on delete set null,
    day date not null,
    total integer not null,
    unique (job_id, day)
);


alter table job add column first_published_at timestamptz;


insert into job_status (name) values ('deleted');

alter table job add column deleted_at timestamptz;


create table if not exists search_appearances (
    job_id uuid references job on delete set null,
    day date not null,
    total integer not null,
    unique (job_id, day)
);


create table certification (
    certification_id uuid primary key default gen_random_uuid(),

    name text not null unique check (name <> ''),
    provider text not null check (provider <> ''),
    short_name text not null unique check (short_name <> ''),

    description text check (description <> ''),
    logo_url text check (logo_url <> ''),
    url text check (url <> '')
);

insert into certification (name, provider, short_name, description, url, logo_url) values
('Certified Kubernetes Administrator', 'CNCF', 'CKA', 'Performance-based exam where candidates interact with the command line to solve real-world challenges', 'https://www.cncf.io/training/certification/cka/', 'https://www.cncf.io/wp-content/uploads/2021/09/kubernetes-cka-color.svg'),
('Certified Kubernetes Application Developer', 'CNCF', 'CKAD', 'Proves you can design, build, and manage cloud-native applications on Kubernetes', 'https://www.cncf.io/training/certification/ckad/', 'https://www.cncf.io/wp-content/uploads/2021/09/kubernetes-ckad-color.svg'),
('Certified Kubernetes Security Specialist', 'CNCF', 'CKS', 'Covers best practices for securing container-based applications and Kubernetes platforms', 'https://www.cncf.io/training/certification/cks/', 'https://www.cncf.io/wp-content/uploads/2020/11/kubernetes-security-specialist-logo.svg'),
('Kubernetes and Cloud Native Associate', 'CNCF', 'KCNA', 'Entry-level credential focusing on Kubernetes and broader cloud native ecosystem', 'https://www.cncf.io/training/certification/kcna/', 'https://www.cncf.io/wp-content/uploads/2021/09/kcna_color.svg'),
('Kubernetes and Cloud Security Associate', 'CNCF', 'KCSA', 'Validates skills in evaluating Kubernetes cluster security configurations and compliance', 'https://www.cncf.io/training/certification/kcsa/', 'https://www.cncf.io/wp-content/uploads/2024/03/kubernetes-kcsa-color.svg'),
('Prometheus Certified Associate', 'CNCF', 'PCA', 'Entry-level certification demonstrating foundational knowledge of observability and Prometheus monitoring', 'https://www.cncf.io/training/certification/pca/', 'https://www.cncf.io/wp-content/uploads/2023/11/PCA-Prometheus-Certified-Associate-logo-color.svg'),
('Istio Certified Associate', 'CNCF', 'ICA', 'Pre-professional certification demonstrating foundational knowledge of Istio principles, terminology, and best practices', 'https://www.cncf.io/training/certification/ica/', 'https://www.cncf.io/wp-content/uploads/2024/03/ica-icon-color.svg'),
('Cilium Certified Associate', 'CNCF', 'CCA', 'Entry-level certification designed for platform or cloud engineers interested in networking, security, and observability', 'https://www.cncf.io/training/certification/cca/', 'https://www.cncf.io/wp-content/uploads/2024/03/cca-icon-color.svg'),
('Certified Argo Project Associate', 'CNCF', 'CAPA', 'Designed for engineers, data scientists, and others interested in demonstrating their understanding of the Argo Project ecosystem', 'https://www.cncf.io/training/certification/capa/', 'https://www.cncf.io/wp-content/uploads/2024/03/capa-icon-color.svg'),
('GitOps Certified Associate', 'CNCF', 'CGOA', 'For DevOps engineers and team members, platform and software engineers, CI/CD practitioners interested in GitOps', 'https://www.cncf.io/training/certification/cgoa/', 'https://www.cncf.io/wp-content/uploads/2024/03/gitops_associate.svg'),
('Certified Backstage Associate', 'CNCF', 'CBA', 'Proves you have the skills & the mindset to work with Backstage to advance your career, your team & your organization', 'https://www.cncf.io/training/certification/cba/', 'https://www.cncf.io/wp-content/uploads/2024/11/lft_badge_backstage_associate1.svg'),
('OpenTelemetry Certified Associate', 'CNCF', 'OTCA', 'Prove your expertise in OpenTelemetry 鈥?the industry standard for tracing, metrics & logs', 'https://www.cncf.io/training/certification/otca/', 'https://www.cncf.io/wp-content/uploads/2024/11/lft_badge_opentelemetry_associate1.svg'),
('Kyverno Certified Associate', 'CNCF', 'KCA', 'Position yourself as an expert in managing and securing Kubernetes environments. Kyverno expertise shows you understand the advanced aspects of cloud management and security', 'https://www.cncf.io/training/certification/kca/', 'https://www.cncf.io/wp-content/uploads/2024/11/lft_badge_kyverno_associate1.svg');

create table job_certification (
    job_id uuid not null references job on delete cascade,
    certification_id uuid not null references certification on delete restrict,

    primary key (job_id, certification_id)
);

create index job_certification_job_id_idx on job_certification (job_id);
create index job_certification_certification_id_idx on job_certification (certification_id);


-- job_project table
create index job_project_job_id_idx on job_project (job_id);
create index job_project_project_id_idx on job_project (project_id);

-- job table
create index job_published_at_idx on job (published_at);
create index job_published_at_desc_idx on job (published_at DESC) where status = 'published';
create index job_created_at_idx on job (created_at);
create index job_first_published_at_idx on job (first_published_at);
create index job_salary_max_usd_year_idx on job (salary_max_usd_year);
create index job_open_source_idx on job (open_source);
create index job_upstream_commitment_idx on job (upstream_commitment);
create index job_seniority_idx on job (seniority);

-- job_views table
create index job_views_job_id_idx on job_views (job_id);
create index job_views_day_idx on job_views (day);
create index job_views_job_id_day_idx on job_views (job_id, day);

-- search_appearances table
create index search_appearances_job_id_idx on search_appearances (job_id);
create index search_appearances_day_idx on search_appearances (day);
create index search_appearances_job_id_day_idx on search_appearances (job_id, day);

-- employer_team table
create index employer_team_approved_idx on employer_team (approved) where approved = false;
create index employer_team_user_id_approved_idx on employer_team (user_id, approved);

-- application table
create index application_created_at_idx on application (created_at);

-- session table
create index session_expires_at_idx on session (expires_at);

-- user table
create index user_email_verified_idx on "user" (email_verified) where email_verified = false;


-- Create a junction table to support many-to-many employer-member relationships
create table employer_member (
    employer_id uuid not null references employer on delete cascade,
    member_id uuid not null references member on delete cascade,

    primary key (employer_id, member_id)
);

-- Add lookup indexes for both relationship directions
create index employer_member_employer_id_idx on employer_member (employer_id);
create index employer_member_member_id_idx on employer_member (member_id);

-- Backfill the junction table from the legacy single-member column
insert into employer_member (employer_id, member_id)
select employer_id, member_id
from employer
where member_id is not null;

-- Remove obsolete index and legacy column from employer
drop index if exists employer_member_id_idx;
alter table employer drop column member_id;

-- Create a table to deduplicate notification template payloads by content hash
create table notification_template_data (
    notification_template_data_id uuid primary key default gen_random_uuid(),
    created_at timestamptz default current_timestamp not null,
    data jsonb not null,
    hash text not null constraint notification_template_data_hash_idx unique
);

-- Add a foreign key from notification rows to normalized template payloads
alter table notification add column notification_template_data_id uuid references notification_template_data;

-- Backfill deduplicated template payload rows from existing notification data
with template_data as (
    select distinct
        n.template_data as data,
        encode(digest(convert_to(n.template_data::text, 'utf8'), 'sha256'), 'hex') as hash
    from notification n
    where n.template_data is not null
)
insert into notification_template_data (data, hash)
select
    t.data,
    t.hash
from template_data t
on conflict (hash) do nothing;

-- Link existing notifications to their deduplicated template payload rows
update notification n
set notification_template_data_id = ntd.notification_template_data_id
from notification_template_data ntd
where n.template_data is not null
and ntd.hash = encode(digest(convert_to(n.template_data::text, 'utf8'), 'sha256'), 'hex');

-- Drop inline template payload data after migration to normalized storage
alter table notification drop column template_data;

-- Remove legacy function signatures before reloading updated definitions
drop function if exists enqueue_notification(text, jsonb, uuid);
drop function if exists search_applications(uuid, jsonb);
drop function if exists search_jobs(jsonb);

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
    encryption_key_version bigint check (encryption_key_version >= 1 and encryption_key_version <= 4294967295),
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
    lodging text check (lodging is null or char_length(lodging) <= 256),
    housing_allowance bigint check (housing_allowance is null or housing_allowance between 0 and 4294967295),
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
    encryption_key_version bigint check (encryption_key_version >= 1 and encryption_key_version <= 4294967295),
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
    updated_at timestamptz not null default current_timestamp,
    claim_token uuid,
    claim_version bigint not null default 0 check (claim_version >= 0),
    claim_expires_at timestamptz,
    constraint talent_import_items_claim_lease_consistent check (
        (
            status = 'extracting'
            and claim_token is not null
            and claim_version > 0
            and claim_expires_at is not null
        )
        or (
            status <> 'extracting'
            and claim_token is null
            and claim_expires_at is null
        )
    )
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
    encryption_key_version bigint check (encryption_key_version >= 1 and encryption_key_version <= 4294967295),
    normalized_hash text,
    masked_value text,
    created_at timestamptz not null default current_timestamp,
    deleted_at timestamptz
);

create table talent_review_events (
    event_id uuid primary key default gen_random_uuid(),
    item_id uuid references talent_import_items (item_id) on delete restrict,
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
    created_at timestamptz not null default current_timestamp,
    resource_scope text not null default 'item',
    source_id uuid references talent_sources (source_id) on delete restrict,
    batch_id uuid references talent_import_batches (batch_id) on delete restrict,
    constraint talent_review_events_resource_scope_check check (
        (resource_scope = 'item' and item_id is not null and source_id is null and batch_id is null)
        or (
            resource_scope = 'source'
            and item_id is null
            and source_id is not null
            and batch_id is null
        )
        or (
            resource_scope = 'runtime'
            and item_id is null
            and source_id is null
            and batch_id is null
        )
        or (
            resource_scope in ('batch', 'cleanup')
            and item_id is null
            and source_id is null
            and batch_id is not null
        )
    )
);

comment on column talent_review_events.before_snapshot is
    'Complete contacts are forbidden in this JSONB audit snapshot.';
comment on column talent_review_events.after_snapshot is
    'Complete contacts are forbidden in this JSONB audit snapshot.';

create function reject_talent_review_event_mutation()
returns trigger as $$
begin
    raise exception 'talent audit history is append-only' using errcode = '55000';
end;
$$ language plpgsql;

create trigger talent_review_events_append_only
before update or delete on talent_review_events
for each row execute function reject_talent_review_event_mutation();

create table talent_runtime_settings (
    singleton boolean primary key default true constraint talent_runtime_settings_singleton_check check (singleton),
    auto_publish_enabled boolean not null default false,
    updated_by uuid references "user" (user_id) on delete set null,
    updated_at timestamptz not null default current_timestamp
);

insert into talent_runtime_settings (singleton) values (true);


-- Store the full positive u32 key-version contract used by the service.
alter table talent_import_batches
    alter column encryption_key_version type bigint
    using encryption_key_version::bigint;

alter table talent_import_batches
    add constraint talent_import_batches_encryption_key_version_u32
    check (encryption_key_version between 1 and 4294967295);

alter table talent_import_items
    alter column encryption_key_version type bigint
    using encryption_key_version::bigint;

alter table talent_import_items
    add constraint talent_import_items_encryption_key_version_u32
    check (encryption_key_version between 1 and 4294967295);

alter table talent_contacts
    alter column encryption_key_version type bigint
    using encryption_key_version::bigint;

alter table talent_contacts
    add constraint talent_contacts_encryption_key_version_u32
    check (encryption_key_version between 1 and 4294967295);

create table talent_source_subscriptions (
    subscription_id uuid primary key default gen_random_uuid(),
    source_id uuid not null references talent_sources (source_id) on delete cascade,
    public_url text not null check (
        public_url ~ '^https://[^/?#[:space:]]+(?:[/?][^#[:space:]]*)?$'
        and public_url !~ '@'
    ),
    keywords text[] not null check (cardinality(keywords) between 1 and 20),
    interval_minutes integer not null check (interval_minutes between 360 and 10080),
    enabled boolean not null default true,
    next_run_at timestamptz not null default current_timestamp,
    claimed_at timestamptz,
    incremental_cursor text,
    etag text,
    last_modified text,
    consecutive_failures integer not null default 0 check (consecutive_failures >= 0),
    last_success_at timestamptz,
    last_error_at timestamptz,
    last_error_code text,
    created_by uuid references "user" (user_id) on delete set null,
    created_at timestamptz not null default current_timestamp,
    updated_at timestamptz not null default current_timestamp,
    unique (source_id, public_url)
);

create index talent_source_subscriptions_due_idx
on talent_source_subscriptions (next_run_at, subscription_id)
where enabled;

create table talent_source_runs (
    run_id uuid primary key default gen_random_uuid(),
    subscription_id uuid not null references talent_source_subscriptions (subscription_id) on delete cascade,
    discovered_count integer not null default 0 check (discovered_count >= 0),
    accepted_count integer not null default 0 check (accepted_count >= 0),
    duplicate_count integer not null default 0 check (duplicate_count >= 0),
    failed_count integer not null default 0 check (failed_count >= 0),
    error_code text,
    started_at timestamptz not null default current_timestamp,
    completed_at timestamptz
);

create table talent_source_seen_items (
    subscription_id uuid not null references talent_source_subscriptions (subscription_id) on delete cascade,
    source_item_id text not null,
    content_fingerprint text not null check (content_fingerprint ~ '^[0-9a-f]{64}$'),
    first_seen_at timestamptz not null default current_timestamp,
    primary key (subscription_id, source_item_id),
    unique (subscription_id, content_fingerprint)
);

