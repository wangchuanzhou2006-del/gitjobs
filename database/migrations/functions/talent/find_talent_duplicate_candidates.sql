-- Finds exact persisted contact/fingerprint matches without returning contact material.
create or replace function find_talent_duplicate_candidates(
    p_item_id uuid,
    p_limit integer default 20
)
returns jsonb as $$
with subject as (
    select normalized_fingerprint, contact_hashes
    from talent_import_items
    where item_id = p_item_id
),
item_matches as (
    select
        candidate.item_id as candidate_item_id,
        candidate.published_record_id as candidate_record_id,
        candidate.kind,
        candidate.status,
        candidate.extraction->>'title' as title,
        case
            when candidate.normalized_fingerprint is not null
             and candidate.normalized_fingerprint = subject.normalized_fingerprint then 1.0
            when candidate.contact_hashes && subject.contact_hashes then 1.0
            else 0.0
        end as score,
        case
            when candidate.normalized_fingerprint is not null
             and candidate.normalized_fingerprint = subject.normalized_fingerprint
                then jsonb_build_array('exact_fingerprint')
            else jsonb_build_array('exact_contact_hash')
        end as reasons,
        true as high_similarity,
        candidate.updated_at
    from subject
    join talent_import_items candidate
      on candidate.item_id <> p_item_id
     and (
        (
            subject.normalized_fingerprint is not null
            and candidate.normalized_fingerprint = subject.normalized_fingerprint
        )
        or (
            coalesce(cardinality(subject.contact_hashes), 0) > 0
            and candidate.contact_hashes && subject.contact_hashes
        )
     )
),
record_matches as (
    select distinct on (r.record_id)
        null::uuid as candidate_item_id,
        r.record_id as candidate_record_id,
        r.kind,
        r.status,
        r.title,
        1.0::numeric as score,
        jsonb_build_array('exact_contact_hash') as reasons,
        true as high_similarity,
        r.updated_at
    from subject
    join talent_contacts c
      on c.deleted_at is null
     and c.normalized_hash = any(subject.contact_hashes)
    join talent_records r using (record_id)
    where r.status <> 'withdrawn'
      and not exists (
          select 1
          from talent_import_items i
          where i.item_id = p_item_id
            and i.published_record_id = r.record_id
      )
    order by r.record_id, r.updated_at desc
),
candidates as (
    select * from item_matches
    union all
    select * from record_matches
)
select coalesce(
    jsonb_agg(
        jsonb_build_object(
            'candidate_item_id', candidate_item_id,
            'candidate_record_id', candidate_record_id,
            'kind', kind,
            'status', status,
            'title', title,
            'score', score,
            'reasons', reasons,
            'high_similarity', high_similarity
        )
        order by score desc, updated_at desc
    ),
    '[]'::jsonb
)
from (
    select *
    from candidates
    order by score desc, updated_at desc
    limit greatest(0, least(coalesce(p_limit, 20), 100))
) limited;
$$ language sql stable;
