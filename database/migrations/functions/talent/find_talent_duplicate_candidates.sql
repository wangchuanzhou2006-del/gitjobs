-- Returns exact and structured candidates without raw text, ciphertext, hashes, or contacts.
create or replace function find_talent_duplicate_candidates(
    p_item_id uuid,
    p_limit integer default 20
)
returns jsonb as $$
with subject as (
    select
        item.item_id,
        item.normalized_fingerprint,
        item.contact_hashes,
        item.kind,
        batch.source_id,
        coalesce(
            (
                select jsonb_agg(role #>> '{}')
                from jsonb_array_elements(
                    case
                        when jsonb_typeof(item.extraction->'roles') = 'array'
                            then item.extraction->'roles'
                        else '[]'::jsonb
                    end
                ) role
                where jsonb_typeof(role) = 'string'
            ),
            '[]'::jsonb
        ) as roles,
        case
            when jsonb_typeof(item.extraction->'city') = 'string'
                then item.extraction->>'city'
        end as city,
        case
            when jsonb_typeof(item.extraction->'district') = 'string'
                then item.extraction->>'district'
        end as district,
        case
            when jsonb_typeof(coalesce(
                item.extraction->'salaryMin',
                item.extraction->'salary_min'
            )) = 'number'
                then coalesce(
                    item.extraction->>'salaryMin',
                    item.extraction->>'salary_min'
                )::integer
        end as salary_min,
        case
            when jsonb_typeof(coalesce(
                item.extraction->'salaryMax',
                item.extraction->'salary_max'
            )) = 'number'
                then coalesce(
                    item.extraction->>'salaryMax',
                    item.extraction->>'salary_max'
                )::integer
        end as salary_max,
        lower(
            regexp_replace(
                concat_ws(
                    ' ',
                    case
                        when jsonb_typeof(item.extraction->'title') = 'string'
                            then item.extraction->>'title'
                    end,
                    (
                        select string_agg(role #>> '{}', ' ')
                        from jsonb_array_elements(
                            case
                                when jsonb_typeof(item.extraction->'roles') = 'array'
                                    then item.extraction->'roles'
                                else '[]'::jsonb
                            end
                        ) role
                        where jsonb_typeof(role) = 'string'
                    ),
                    case
                        when jsonb_typeof(item.extraction->'city') = 'string'
                            then item.extraction->>'city'
                    end,
                    case
                        when jsonb_typeof(item.extraction->'district') = 'string'
                            then item.extraction->>'district'
                    end
                ),
                '\s+',
                ' ',
                'g'
            )
        ) as normalized_text
    from talent_import_items item
    join talent_import_batches batch using (batch_id)
    where item.item_id = p_item_id
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
) limited;
$$ language sql stable;
