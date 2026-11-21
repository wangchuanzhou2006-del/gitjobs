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

---- create above / drop below ----

drop table talent_source_seen_items;
drop table talent_source_runs;
drop table talent_source_subscriptions;
