alter table talent_import_items
    add column claim_token uuid,
    add column claim_version bigint not null default 0,
    add column claim_expires_at timestamptz;

update talent_import_items
set
    claim_token = gen_random_uuid(),
    claim_version = 1,
    claim_expires_at = updated_at + interval '2 minutes'
where status = 'extracting';

alter table talent_import_items
    add constraint talent_import_items_claim_version_nonnegative
    check (claim_version >= 0),
    add constraint talent_import_items_claim_lease_consistent
    check (
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
    );

alter table talent_records
    alter column lodging type text using (
        case
            when lodging is null then null
            when lodging then U&'\63D0\4F9B'
            else U&'\4E0D\63D0\4F9B'
        end
    ),
    alter column housing_allowance type bigint using (
        case
            when housing_allowance is null then null
            when housing_allowance then 1::bigint
            else 0::bigint
        end
    );

alter table talent_records
    add constraint talent_records_lodging_length
    check (lodging is null or char_length(lodging) <= 256),
    add constraint talent_records_housing_allowance_u32
    check (housing_allowance is null or housing_allowance between 0 and 4294967295);

alter table talent_review_events
    alter column item_id drop not null,
    add column resource_scope text not null default 'item',
    add column source_id uuid references talent_sources (source_id) on delete restrict,
    add column batch_id uuid references talent_import_batches (batch_id) on delete restrict;

alter table talent_review_events
    add constraint talent_review_events_resource_scope_check
    check (
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
    );

create function reject_talent_review_event_mutation()
returns trigger as $$
begin
    raise exception 'talent audit history is append-only' using errcode = '55000';
end;
$$ language plpgsql;

create trigger talent_review_events_append_only
before update or delete on talent_review_events
for each row execute function reject_talent_review_event_mutation();

---- create above / drop below ----

do $$
begin
    if exists (
        select 1 from talent_review_events where resource_scope <> 'item'
    ) then
        raise exception 'cannot roll back scoped talent audit history'
            using errcode = '22003';
    end if;
    if exists (
        select 1
        from talent_records
        where lodging is not null
          and lodging not in (U&'\63D0\4F9B', U&'\4E0D\63D0\4F9B')
    ) then
        raise exception 'cannot roll back non-boolean talent lodging values'
            using errcode = '22003';
    end if;
    if exists (
        select 1
        from talent_records
        where housing_allowance is not null
          and housing_allowance not in (0, 1)
    ) then
        raise exception 'cannot roll back talent housing allowance amounts'
            using errcode = '22003';
    end if;
end;
$$;

drop trigger talent_review_events_append_only on talent_review_events;
drop function reject_talent_review_event_mutation();

alter table talent_review_events
    drop constraint talent_review_events_resource_scope_check,
    drop column batch_id,
    drop column source_id,
    drop column resource_scope,
    alter column item_id set not null;

alter table talent_records
    drop constraint talent_records_housing_allowance_u32,
    drop constraint talent_records_lodging_length,
    alter column housing_allowance type boolean using (housing_allowance = 1),
    alter column lodging type boolean using (lodging = U&'\63D0\4F9B');

alter table talent_import_items
    drop constraint talent_import_items_claim_lease_consistent,
    drop constraint talent_import_items_claim_version_nonnegative,
    drop column claim_expires_at,
    drop column claim_version,
    drop column claim_token;
