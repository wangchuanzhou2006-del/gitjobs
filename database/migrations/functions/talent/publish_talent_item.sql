-- Publishes an item and its encrypted contacts atomically and idempotently.
create or replace function publish_talent_item(
    p_item_id uuid,
    p_lock_version integer,
    p_actor_user_id uuid,
    p_automatic boolean,
    p_record jsonb,
    p_contacts jsonb
)
returns table(record_id uuid, created boolean) as $$
declare
    v_item talent_import_items%rowtype;
    v_batch talent_import_batches%rowtype;
    v_source talent_sources%rowtype;
    v_record_id uuid;
    v_contact_count integer;
    v_global_enabled boolean;
    v_publication jsonb;
    v_required_complete boolean;
    v_safety_valid boolean;
    v_hard_block boolean;
    v_high_duplicate boolean;
begin
    select *
    into v_item
    from talent_import_items
    where item_id = p_item_id
    for update;

    if not found then
        raise exception 'talent import item not found';
    end if;
    if p_lock_version is null then
        raise exception 'talent import item lock version is required' using errcode = '22023';
    end if;
    if p_automatic is null then
        raise exception 'talent publication mode is required' using errcode = '22023';
    end if;

    -- A retry may carry the original lock version. The durable source link wins.
    if v_item.published_record_id is not null
       and v_item.status in ('published', 'auto_published') then
        record_id := v_item.published_record_id;
        created := false;
        return next;
        return;
    end if;

    if v_item.lock_version <> p_lock_version then
        raise exception 'talent import item lock conflict' using errcode = '40001';
    end if;
    if v_item.status <> 'needs_review' then
        raise exception 'talent import item is not awaiting review';
    end if;
    if p_record is null or jsonb_typeof(p_record) <> 'object' then
        raise exception 'talent publication record must be a JSON object' using errcode = '22023';
    end if;
    if p_contacts is null or jsonb_typeof(p_contacts) <> 'array' then
        raise exception 'talent publication contacts must be a JSON array' using errcode = '22023';
    end if;
    if p_record ? 'contacts' then
        raise exception 'complete contacts are forbidden in talent publication record' using errcode = '22023';
    end if;
    if not p_automatic and p_actor_user_id is null then
        raise exception 'manual talent publication requires a Moderator' using errcode = '22023';
    end if;

    select *
    into v_batch
    from talent_import_batches
    where batch_id = v_item.batch_id;

    select *
    into v_source
    from talent_sources
    where source_id = v_batch.source_id
    for update;

    select auto_publish_enabled
    into v_global_enabled
    from talent_runtime_settings
    where singleton
    for update;

    v_publication := case
        when p_automatic then jsonb_build_object(
            'kind', v_item.kind,
            'title', v_item.extraction->>'title',
            'scene', v_item.extraction->>'scene',
            'region', v_item.extraction->>'region',
            'city', v_item.extraction->>'city',
            'district', v_item.extraction->>'district',
            'salary_min', coalesce(
                v_item.extraction->'salaryMin',
                v_item.extraction->'salary_min'
            ),
            'salary_max', coalesce(
                v_item.extraction->'salaryMax',
                v_item.extraction->'salary_max'
            ),
            'roles', coalesce(v_item.extraction->'roles', '[]'::jsonb),
            'skills', coalesce(v_item.extraction->'skills', '[]'::jsonb),
            'experience_years', coalesce(
                v_item.extraction->'experienceYears',
                v_item.extraction->'experience_years'
            ),
            'hours_per_day', coalesce(
                v_item.extraction->'hoursPerDay',
                v_item.extraction->'hours_per_day'
            ),
            'rest_days_per_month', coalesce(
                v_item.extraction->'restDaysPerMonth',
                v_item.extraction->'rest_days_per_month'
            ),
            'availability', v_item.extraction->>'availability',
            'urgency', v_item.extraction->>'urgency',
            'published_at', coalesce(
                v_item.extraction->'publishedAt',
                v_item.extraction->'published_at'
            ),
            'expires_at', coalesce(
                v_item.extraction->'expiresAt',
                v_item.extraction->'expires_at'
            )
        )
        else p_record
    end;

    v_contact_count := jsonb_array_length(p_contacts);
    v_safety_valid :=
        jsonb_typeof(v_item.risk_reasons) = 'array'
        and not exists (
            select 1
            from jsonb_array_elements(
                case
                    when jsonb_typeof(v_item.risk_reasons) = 'array' then v_item.risk_reasons
                    else '[]'::jsonb
                end
            ) as reasons(reason)
            where jsonb_typeof(reason) <> 'object'
               or not (reason ?& array['code', 'explanation', 'evidence', 'hard_block'])
               or exists (
                   select 1
                   from jsonb_object_keys(
                       case when jsonb_typeof(reason) = 'object' then reason else '{}'::jsonb end
                   ) as keys(key)
                   where key <> all(array['code', 'explanation', 'evidence', 'hard_block'])
               )
               or jsonb_typeof(reason->'code') <> 'string'
               or coalesce(reason->>'code', '') = ''
               or jsonb_typeof(reason->'explanation') <> 'string'
               or coalesce(reason->>'explanation', '') = ''
               or jsonb_typeof(reason->'evidence') not in ('string', 'null')
               or jsonb_typeof(reason->'hard_block') <> 'boolean'
        )
        and jsonb_typeof(v_item.duplicate_candidates) = 'array'
        and not exists (
            select 1
            from jsonb_array_elements(
                case
                    when jsonb_typeof(v_item.duplicate_candidates) = 'array'
                        then v_item.duplicate_candidates
                    else '[]'::jsonb
                end
            ) as candidates(candidate)
            where jsonb_typeof(candidate) <> 'object'
               or not (
                   candidate ?& array[
                       'candidate_item_id',
                       'candidate_record_id',
                       'score',
                       'reasons',
                       'high_similarity'
                   ]
               )
               or exists (
                   select 1
                   from jsonb_object_keys(
                       case
                           when jsonb_typeof(candidate) = 'object' then candidate
                           else '{}'::jsonb
                       end
                   ) as keys(key)
                   where key <> all(
                       array[
                           'candidate_item_id',
                           'candidate_record_id',
                           'score',
                           'reasons',
                           'high_similarity'
                       ]
                   )
               )
               or jsonb_typeof(candidate->'candidate_item_id') not in ('string', 'null')
               or jsonb_typeof(candidate->'candidate_record_id') not in ('string', 'null')
               or (
                   jsonb_typeof(candidate->'candidate_item_id') = 'null'
                   and jsonb_typeof(candidate->'candidate_record_id') = 'null'
               )
               or (
                   jsonb_typeof(candidate->'candidate_item_id') = 'string'
                   and (candidate->>'candidate_item_id')
                       !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
               )
               or (
                   jsonb_typeof(candidate->'candidate_record_id') = 'string'
                   and (candidate->>'candidate_record_id')
                       !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
               )
               or jsonb_typeof(candidate->'score') <> 'number'
               or case
                   when jsonb_typeof(candidate->'score') = 'number'
                       then (candidate->>'score')::numeric not between 0 and 1
                   else false
               end
               or jsonb_typeof(candidate->'reasons') <> 'array'
               or exists (
                   select 1
                   from jsonb_array_elements(
                       case
                           when jsonb_typeof(candidate->'reasons') = 'array'
                               then candidate->'reasons'
                           else '[]'::jsonb
                       end
                   ) as duplicate_reasons(reason)
                   where jsonb_typeof(reason) <> 'string'
                      or coalesce(reason #>> '{}', '') = ''
               )
               or jsonb_typeof(candidate->'high_similarity') <> 'boolean'
        );

    v_hard_block := case
        when v_safety_valid then jsonb_path_exists(
            v_item.risk_reasons,
            '$[*] ? (@.hard_block == true)'
        )
        else true
    end;
    v_high_duplicate := case
        when v_safety_valid then jsonb_path_exists(
            v_item.duplicate_candidates,
            '$[*] ? (@.high_similarity == true || @.score >= 0.85)'
        )
        else true
    end;
    v_required_complete :=
        coalesce(v_item.extraction->>'kind', '') = v_item.kind
        and coalesce(nullif(v_publication->>'title', ''), '') <> ''
        and coalesce(nullif(v_publication->>'published_at', ''), '') <> ''
        and case
            when jsonb_typeof(v_publication->'roles') = 'array'
                then jsonb_array_length(v_publication->'roles') > 0
            else false
        end
        and not exists (
            select 1
            from jsonb_array_elements(
                case
                    when jsonb_typeof(v_publication->'roles') = 'array'
                        then v_publication->'roles'
                    else '[]'::jsonb
                end
            ) as roles(role)
            where jsonb_typeof(role) <> 'string'
               or coalesce(role #>> '{}', '') = ''
        )
        and (
            coalesce(nullif(v_publication->>'city', ''), '') <> ''
            or coalesce(nullif(v_publication->>'district', ''), '') <> ''
        )
        and v_contact_count > 0
        and coalesce(cardinality(v_item.contact_hashes), 0) > 0
        and not exists (
            select 1
            from jsonb_array_elements(p_contacts) as required_contacts(contact)
            where nullif(contact->>'normalized_hash', '') is null
               or not (
                   (contact->>'normalized_hash') = any(
                       coalesce(v_item.contact_hashes, '{}'::text[])
                   )
               )
        )
        and case
            when v_item.kind = 'hiring' then
                nullif(v_publication->>'salary_min', '') is not null
                and nullif(v_publication->>'salary_max', '') is not null
            when v_item.kind = 'seeking' then true
            else false
        end
        and (
            v_item.validation_errors is null
            or case
                when jsonb_typeof(v_item.validation_errors) = 'array'
                    then jsonb_array_length(v_item.validation_errors) = 0
                else false
            end
        )
        and (
            coalesce(
                v_item.extraction->'missingFields',
                v_item.extraction->'missing_fields'
            ) is null
            or case
                when jsonb_typeof(
                    coalesce(
                        v_item.extraction->'missingFields',
                        v_item.extraction->'missing_fields'
                    )
                ) = 'array'
                    then jsonb_array_length(
                    coalesce(
                        v_item.extraction->'missingFields',
                        v_item.extraction->'missing_fields'
                    )
                ) = 0
                else false
            end
        );

    if p_automatic and not coalesce((
        v_item.risk_level = 'low'
        and not v_hard_block
        and v_source.trust_status = 'trusted'
        and v_source.auto_publish_enabled
        and v_source.confirmed_by is not null
        and v_source.confirmed_at is not null
        and coalesce(v_global_enabled, false)
        and coalesce(v_item.extraction_confidence, 0) >= 0.90
        and v_safety_valid
        and v_required_complete
        and not v_high_duplicate
    ), false) then
        raise exception 'automatic talent publication gates failed' using errcode = '22023';
    end if;

    if exists (
        select 1
        from jsonb_array_elements(p_contacts) as contacts(contact)
        where jsonb_typeof(contact) <> 'object'
           or contact ?| array['value', 'complete_contact', 'raw']
           or coalesce(contact->>'contact_type', '') not in ('phone', 'wechat', 'qq', 'other')
           or coalesce(contact->>'ciphertext', '') !~ '^(?:[0-9a-fA-F]{2})+$'
           or coalesce(nullif(contact->>'encryption_key_version', '')::integer, 0) < 1
           or coalesce(contact->>'masked_value', '')
               !~ '^[[:alnum:]]{0,4}\*{3,}[[:alnum:]]{0,4}$'
    ) then
        if exists (
            select 1
            from jsonb_array_elements(p_contacts) as masked_contacts(contact)
            where coalesce(contact->>'masked_value', '')
                !~ '^[[:alnum:]]{0,4}\*{3,}[[:alnum:]]{0,4}$'
        ) then
            raise exception 'invalid masked talent publication contact' using errcode = '22023';
        end if;
        raise exception 'invalid encrypted talent publication contact' using errcode = '22023';
    end if;

    insert into talent_records (
        kind,
        title,
        scene,
        region,
        city,
        district,
        salary_min,
        salary_max,
        roles,
        skills,
        experience_years,
        hours_per_day,
        rest_days_per_month,
        lodging,
        housing_allowance,
        availability,
        urgency,
        source_id,
        source_label,
        published_at,
        expires_at,
        status,
        creation_source
    )
    values (
        coalesce(v_publication->>'kind', v_item.kind),
        v_publication->>'title',
        nullif(v_publication->>'scene', ''),
        nullif(v_publication->>'region', ''),
        nullif(v_publication->>'city', ''),
        nullif(v_publication->>'district', ''),
        nullif(v_publication->>'salary_min', '')::integer,
        nullif(v_publication->>'salary_max', '')::integer,
        coalesce(
            array(select jsonb_array_elements_text(coalesce(v_publication->'roles', '[]'::jsonb))),
            '{}'::text[]
        ),
        coalesce(
            array(select jsonb_array_elements_text(coalesce(v_publication->'skills', '[]'::jsonb))),
            '{}'::text[]
        ),
        nullif(v_publication->>'experience_years', '')::integer,
        nullif(v_publication->>'hours_per_day', '')::integer,
        nullif(v_publication->>'rest_days_per_month', '')::integer,
        nullif(v_publication->>'lodging', '')::boolean,
        nullif(v_publication->>'housing_allowance', '')::boolean,
        nullif(v_publication->>'availability', ''),
        nullif(v_publication->>'urgency', ''),
        v_source.source_id,
        coalesce(nullif(v_publication->>'source_label', ''), v_source.name),
        coalesce(nullif(v_publication->>'published_at', '')::timestamptz, current_timestamp),
        nullif(v_publication->>'expires_at', '')::timestamptz,
        'published',
        case when p_automatic then 'auto_publish' else 'manual_review' end
    )
    returning talent_records.record_id into v_record_id;

    insert into talent_contacts (
        record_id,
        contact_type,
        ciphertext,
        encryption_key_version,
        normalized_hash,
        masked_value
    )
    select
        v_record_id,
        contact->>'contact_type',
        lower(contact->>'ciphertext'),
        (contact->>'encryption_key_version')::integer,
        nullif(contact->>'normalized_hash', ''),
        contact->>'masked_value'
    from jsonb_array_elements(p_contacts) as contacts(contact);

    insert into talent_review_events (
        item_id,
        record_id,
        actor_type,
        actor_user_id,
        action,
        before_snapshot,
        after_snapshot,
        model_provider,
        model_name,
        prompt_version,
        rule_version,
        policy_version,
        automatic_reason
    )
    values (
        p_item_id,
        v_record_id,
        case when p_automatic then 'system' else 'moderator' end,
        case when p_automatic then null else p_actor_user_id end,
        case when p_automatic then 'auto_published' else 'published' end,
        jsonb_build_object(
            'status', v_item.status,
            'lock_version', v_item.lock_version,
            'risk_level', v_item.risk_level,
            'risk_score', v_item.risk_score
        ),
        jsonb_build_object(
            'status', case when p_automatic then 'auto_published' else 'published' end,
            'record_id', v_record_id,
            'kind', coalesce(v_publication->>'kind', v_item.kind),
            'title', v_publication->>'title',
            'source_id', v_source.source_id
        ),
        v_batch.model_provider,
        v_batch.model_name,
        v_batch.prompt_version,
        'talent-risk-v1',
        'talent-auto-publish-v1',
        case when p_automatic
            then 'all automatic gates passed'
            else null
        end
    );

    update talent_import_items
    set
        status = case when p_automatic then 'auto_published' else 'published' end,
        published_record_id = v_record_id,
        lock_version = lock_version + 1,
        reviewed_at = current_timestamp,
        updated_at = current_timestamp
    where item_id = p_item_id;

    update talent_sources
    set published_count = published_count + 1, updated_at = current_timestamp
    where source_id = v_source.source_id;

    record_id := v_record_id;
    created := true;
    return next;
end;
$$ language plpgsql;
