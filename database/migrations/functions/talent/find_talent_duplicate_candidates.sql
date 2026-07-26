-- Returns exact and structured candidates without raw text, ciphertext, hashes, or contacts.
drop function if exists find_talent_duplicate_candidates(uuid, integer);

create or replace function find_talent_duplicate_candidates(
    p_probe jsonb,
    p_limit integer default 20
)
returns jsonb as $$
begin
    if p_probe is null or jsonb_typeof(p_probe) <> 'object' then
        raise exception 'talent duplicate probe must be a JSON object' using errcode = '22023';
    end if;
    if not (
        p_probe ?& array[
            'item_id',
            'kind',
            'title',
            'roles',
            'city',
            'district',
            'salary_min',
            'salary_max',
            'source_id',
            'contact_hashes',
            'normalized_fingerprint',
            'normalized_text'
        ]
    ) or exists (
        select 1
        from jsonb_object_keys(p_probe) as fields(field)
        where field <> all(
            array[
                'item_id',
                'kind',
                'title',
                'roles',
                'city',
                'district',
                'salary_min',
                'salary_max',
                'source_id',
                'contact_hashes',
                'normalized_fingerprint',
                'normalized_text'
            ]
        )
    ) then
        raise exception 'talent duplicate probe has invalid fields' using errcode = '22023';
    end if;
    if jsonb_typeof(p_probe->'item_id') <> 'string'
       or (p_probe->>'item_id')
            !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
       or jsonb_typeof(p_probe->'source_id') <> 'string'
       or (p_probe->>'source_id')
            !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
       or jsonb_typeof(p_probe->'kind') <> 'string'
       or (p_probe->>'kind') not in ('hiring', 'seeking', 'irrelevant', 'uncertain')
       or jsonb_typeof(p_probe->'title') not in ('string', 'null')
       or jsonb_typeof(p_probe->'city') not in ('string', 'null')
       or jsonb_typeof(p_probe->'district') not in ('string', 'null')
       or jsonb_typeof(p_probe->'salary_min') not in ('number', 'null')
       or jsonb_typeof(p_probe->'salary_max') not in ('number', 'null')
       or jsonb_typeof(p_probe->'roles') <> 'array'
       or jsonb_typeof(p_probe->'contact_hashes') <> 'array'
       or jsonb_typeof(p_probe->'normalized_fingerprint') not in ('string', 'null')
       or jsonb_typeof(p_probe->'normalized_text') <> 'string'
    then
        raise exception 'talent duplicate probe has invalid field types' using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_array_elements(p_probe->'roles') as roles(role)
        where jsonb_typeof(role) <> 'string'
           or coalesce(role #>> '{}', '') = ''
    ) or exists (
        select 1
        from jsonb_array_elements(p_probe->'contact_hashes') as hashes(hash)
        where jsonb_typeof(hash) <> 'string'
           or coalesce(hash #>> '{}', '') = ''
    ) or (
        jsonb_typeof(p_probe->'normalized_fingerprint') = 'string'
        and coalesce(p_probe->>'normalized_fingerprint', '') = ''
    ) then
        raise exception 'talent duplicate probe has invalid array entries' using errcode = '22023';
    end if;
    if (
        jsonb_typeof(p_probe->'salary_min') = 'number'
        and (
            (p_probe->>'salary_min')::numeric % 1 <> 0
            or (p_probe->>'salary_min')::numeric not between 0 and 2147483647
        )
    ) or (
        jsonb_typeof(p_probe->'salary_max') = 'number'
        and (
            (p_probe->>'salary_max')::numeric % 1 <> 0
            or (p_probe->>'salary_max')::numeric not between 0 and 2147483647
        )
    ) then
        raise exception 'talent duplicate probe has invalid salary values' using errcode = '22023';
    end if;

    return (
with subject as (
    select
        (p_probe->>'item_id')::uuid as item_id,
        nullif(p_probe->>'normalized_fingerprint', '') as normalized_fingerprint,
        array(
            select hash #>> '{}'
            from jsonb_array_elements(p_probe->'contact_hashes') as hashes(hash)
        ) as contact_hashes,
        p_probe->>'kind' as kind,
        (p_probe->>'source_id')::uuid as source_id,
        p_probe->'roles' as roles,
        nullif(p_probe->>'city', '') as city,
        nullif(p_probe->>'district', '') as district,
        nullif(p_probe->>'salary_min', '')::integer as salary_min,
        nullif(p_probe->>'salary_max', '')::integer as salary_max,
        p_probe->>'normalized_text' as normalized_text
),
item_candidates as (
    select
        candidate.item_id as candidate_item_id,
        candidate.published_record_id as candidate_record_id,
        candidate.kind,
        candidate.status,
        case
            when jsonb_typeof(candidate.extraction->'title') = 'string'
                then candidate.extraction->>'title'
        end as title,
        coalesce(
            (
                select jsonb_agg(role #>> '{}')
                from jsonb_array_elements(
                    case
                        when jsonb_typeof(candidate.extraction->'roles') = 'array'
                            then candidate.extraction->'roles'
                        else '[]'::jsonb
                    end
                ) role
                where jsonb_typeof(role) = 'string'
            ),
            '[]'::jsonb
        ) as roles,
        case
            when jsonb_typeof(candidate.extraction->'city') = 'string'
                then candidate.extraction->>'city'
        end as city,
        case
            when jsonb_typeof(candidate.extraction->'district') = 'string'
                then candidate.extraction->>'district'
        end as district,
        case
            when jsonb_typeof(coalesce(
                candidate.extraction->'salaryMin',
                candidate.extraction->'salary_min'
            )) = 'number'
                then coalesce(
                    candidate.extraction->>'salaryMin',
                    candidate.extraction->>'salary_min'
                )::integer
        end as salary_min,
        case
            when jsonb_typeof(coalesce(
                candidate.extraction->'salaryMax',
                candidate.extraction->'salary_max'
            )) = 'number'
                then coalesce(
                    candidate.extraction->>'salaryMax',
                    candidate.extraction->>'salary_max'
                )::integer
        end as salary_max,
        candidate_batch.source_id,
        lower(
            regexp_replace(
                concat_ws(
                    ' ',
                    case
                        when jsonb_typeof(candidate.extraction->'title') = 'string'
                            then candidate.extraction->>'title'
                    end,
                    (
                        select string_agg(role #>> '{}', ' ')
                        from jsonb_array_elements(
                            case
                                when jsonb_typeof(candidate.extraction->'roles') = 'array'
                                    then candidate.extraction->'roles'
                                else '[]'::jsonb
                            end
                        ) role
                        where jsonb_typeof(role) = 'string'
                    ),
                    case
                        when jsonb_typeof(candidate.extraction->'city') = 'string'
                            then candidate.extraction->>'city'
                    end,
                    case
                        when jsonb_typeof(candidate.extraction->'district') = 'string'
                            then candidate.extraction->>'district'
                    end
                ),
                '\s+',
                ' ',
                'g'
            )
        ) as normalized_text,
        case
            when subject.normalized_fingerprint is not null
             and candidate.normalized_fingerprint is not null
             and candidate.normalized_fingerprint = subject.normalized_fingerprint then 1.0
            when coalesce(candidate.contact_hashes && subject.contact_hashes, false) then 1.0
            else 0.0
        end as score,
        case
            when subject.normalized_fingerprint is not null
             and candidate.normalized_fingerprint is not null
             and candidate.normalized_fingerprint = subject.normalized_fingerprint
                then jsonb_build_array('exact_fingerprint')
            when coalesce(candidate.contact_hashes && subject.contact_hashes, false)
                then jsonb_build_array('exact_contact_hash')
            else '[]'::jsonb
        end as reasons,
        (
            (
                subject.normalized_fingerprint is not null
                and candidate.normalized_fingerprint is not null
                and candidate.normalized_fingerprint = subject.normalized_fingerprint
            )
            or coalesce(candidate.contact_hashes && subject.contact_hashes, false)
        ) as high_similarity,
        candidate.updated_at
    from subject
    join talent_import_items candidate
      on candidate.item_id <> subject.item_id
    join talent_import_batches candidate_batch
      on candidate_batch.batch_id = candidate.batch_id
    where (
        (
            subject.normalized_fingerprint is not null
            and candidate.normalized_fingerprint = subject.normalized_fingerprint
        )
        or (
            coalesce(cardinality(subject.contact_hashes), 0) > 0
            and candidate.contact_hashes && subject.contact_hashes
        )
        or (
            candidate.extraction is not null
            and candidate.kind = subject.kind
        )
      )
),
record_candidates as (
    select
        null::uuid as candidate_item_id,
        record.record_id as candidate_record_id,
        record.kind,
        record.status,
        record.title,
        to_jsonb(coalesce(record.roles, '{}'::text[])) as roles,
        record.city,
        record.district,
        record.salary_min,
        record.salary_max,
        record.source_id,
        lower(
            regexp_replace(
                concat_ws(
                    ' ',
                    record.title,
                    array_to_string(record.roles, ' '),
                    record.city,
                    record.district
                ),
                '\s+',
                ' ',
                'g'
            )
        ) as normalized_text,
        case
            when exact.fingerprint_match or exact.contact_match then 1.0
            else 0.0
        end as score,
        case
            when exact.fingerprint_match then jsonb_build_array('exact_fingerprint')
            when exact.contact_match then jsonb_build_array('exact_contact_hash')
            else '[]'::jsonb
        end as reasons,
        (exact.fingerprint_match or exact.contact_match) as high_similarity,
        record.updated_at
    from subject
    join talent_records record
      on record.status <> 'withdrawn'
    cross join lateral (
        select
            exists (
                select 1
                from talent_import_items origin
                where origin.published_record_id = record.record_id
                  and subject.normalized_fingerprint is not null
                  and origin.normalized_fingerprint = subject.normalized_fingerprint
            ) as fingerprint_match,
            exists (
                select 1
                from talent_contacts contact
                where contact.record_id = record.record_id
                  and contact.deleted_at is null
                  and coalesce(cardinality(subject.contact_hashes), 0) > 0
                  and contact.normalized_hash = any(subject.contact_hashes)
            ) as contact_match
    ) exact
    where record.record_id is distinct from (
        select own.published_record_id
        from talent_import_items own
        where own.item_id = subject.item_id
    )
      and (record.kind = subject.kind or exact.fingerprint_match or exact.contact_match)
      and not exists (
        select 1
        from talent_import_items represented
        where represented.published_record_id = record.record_id
      )
),
candidates as (
    select * from item_candidates
    union all
    select * from record_candidates
),
-- Preselection uses a conservative text upper bound; Rust recomputes the authoritative score.
ranked_candidates as (
    select
        candidates.*,
        (
            case
                when exists (
                    select 1
                    from jsonb_array_elements_text(subject.roles) as subject_role(value)
                    join jsonb_array_elements_text(candidates.roles) as candidate_role(value)
                      on lower(btrim(subject_role.value)) = lower(btrim(candidate_role.value))
                     and btrim(subject_role.value) <> ''
                ) then 0.35
                else 0.0
            end
            + case
                when (
                    lower(btrim(subject.city)) = lower(btrim(candidates.city))
                    and btrim(subject.city) <> ''
                ) or (
                    lower(btrim(subject.district)) = lower(btrim(candidates.district))
                    and btrim(subject.district) <> ''
                ) then 0.25
                else 0.0
            end
            + case
                when subject.salary_min is not null
                 and subject.salary_max is not null
                 and candidates.salary_min is not null
                 and candidates.salary_max is not null
                 and subject.salary_min <= candidates.salary_max
                 and candidates.salary_min <= subject.salary_max then 0.20
                else 0.0
            end
            + case
                when candidates.source_id = subject.source_id then 0.10
                else 0.0
            end
            + case
                when coalesce(subject.normalized_text, '') <> ''
                 and coalesce(candidates.normalized_text, '') <> '' then 0.10
                else 0.0
            end
        ) as structured_upper_bound
    from candidates
    cross join subject
)
select coalesce(
    jsonb_agg(
        jsonb_build_object(
            'candidate_item_id', candidate_item_id,
            'candidate_record_id', candidate_record_id,
            'kind', kind,
            'status', status,
            'title', title,
            'roles', roles,
            'city', city,
            'district', district,
            'salary_min', salary_min,
            'salary_max', salary_max,
            'source_id', source_id,
            'normalized_text', normalized_text,
            'score', score,
            'reasons', reasons,
            'high_similarity', high_similarity
        )
        order by score desc, structured_upper_bound desc, updated_at desc
    ),
    '[]'::jsonb
)
from (
    select *
    from ranked_candidates
    order by score desc, structured_upper_bound desc, updated_at desc
    limit greatest(0, least(coalesce(p_limit, 20), 100))
) limited
    );
end;
$$ language plpgsql stable;
