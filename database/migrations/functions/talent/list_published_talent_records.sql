-- Public projection: explicitly excludes raw text, ciphertext, hashes, and full contacts.
create or replace function list_published_talent_records(
    p_kind text default null,
    p_limit integer default 50
)
returns jsonb as $$
    select coalesce(
        jsonb_agg(
            jsonb_build_object(
                'record_id', records.record_id,
                'kind', records.kind,
                'title', records.title,
                'scene', records.scene,
                'region', records.region,
                'city', records.city,
                'district', records.district,
                'salary_min', records.salary_min,
                'salary_max', records.salary_max,
                'roles', records.roles,
                'skills', records.skills,
                'experience_years', records.experience_years,
                'hours_per_day', records.hours_per_day,
                'rest_days_per_month', records.rest_days_per_month,
                'lodging', records.lodging,
                'housing_allowance', records.housing_allowance,
                'availability', records.availability,
                'urgency', records.urgency,
                'source_label', records.source_label,
                'published_at', records.published_at,
                'expires_at', records.expires_at,
                'contact_masked', records.contact_masked
            )
            order by records.published_at desc, records.record_id
        ),
        '[]'::jsonb
    )
    from (
        select
            r.record_id,
            r.kind,
            r.title,
            r.scene,
            r.region,
            r.city,
            r.district,
            r.salary_min,
            r.salary_max,
            coalesce(r.roles, '{}'::text[]) as roles,
            coalesce(r.skills, '{}'::text[]) as skills,
            r.experience_years,
            r.hours_per_day,
            r.rest_days_per_month,
            r.lodging,
            r.housing_allowance,
            r.availability,
            r.urgency,
            r.source_label,
            coalesce(r.published_at, r.created_at) as published_at,
            r.expires_at,
            coalesce(
                (
                    select jsonb_agg(
                        jsonb_build_object(
                            'contact_type', c.contact_type,
                            'masked_value', c.masked_value
                        )
                        order by c.created_at, c.contact_id
                    )
                    from talent_contacts c
                    where c.record_id = r.record_id
                      and c.deleted_at is null
                      and char_length(c.masked_value) <= 64
                      and c.masked_value
                          ~ '^[^*[:space:][:cntrl:]]{0,4}\*{3,}[^*[:space:][:cntrl:]]{0,4}$'
                ),
                '[]'::jsonb
            ) as contact_masked
        from talent_records r
        where r.status = 'published'
          and (p_kind is null or r.kind = p_kind)
          and (r.expires_at is null or r.expires_at > current_timestamp)
        order by r.published_at desc, r.record_id
        limit greatest(0, least(coalesce(p_limit, 50), 500))
    ) records;
$$ language sql stable;
