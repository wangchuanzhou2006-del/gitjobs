-- Saves editable safe review fields with optimistic concurrency.
create or replace function update_talent_review_item(
    p_item_id uuid,
    p_lock_version integer,
    p_updates jsonb,
    p_actor_user_id uuid
)
returns table(item_id uuid, lock_version integer) as $$
declare
    v_item_id uuid;
    v_lock_version integer;
begin
    if p_updates is null or jsonb_typeof(p_updates) <> 'object' then
        raise exception 'talent review update must be a JSON object' using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_object_keys(p_updates) as fields(field)
        where field <> all(
            array[
                'kind',
                'extraction',
                'extraction_confidence',
                'contact_masked',
                'validation_errors',
                'risk_score',
                'risk_level',
                'risk_reasons',
                'duplicate_candidates'
            ]
        )
    ) then
        raise exception 'talent review update contains an unknown field' using errcode = '22023';
    end if;
    if jsonb_typeof(coalesce(p_updates->'extraction', '{}'::jsonb)) <> 'object' then
        raise exception 'talent review extraction must be a JSON object' using errcode = '22023';
    end if;
    if coalesce(p_updates->'extraction', '{}'::jsonb) ? 'contacts' then
        raise exception 'complete contacts are forbidden in safe talent extraction' using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_object_keys(coalesce(p_updates->'extraction', '{}'::jsonb)) as fields(field)
        where field <> all(
            array[
                'kind',
                'confidence',
                'title',
                'scene',
                'region',
                'city',
                'district',
                'salaryMin',
                'salaryMax',
                'roles',
                'skills',
                'experienceYears',
                'hoursPerDay',
                'restDaysPerMonth',
                'lodging',
                'housingAllowance',
                'availability',
                'urgency',
                'publishedAt',
                'missingFields',
                'riskCandidates'
            ]
        )
    ) then
        raise exception 'talent review extraction contains an unknown field' using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_each(coalesce(p_updates->'extraction', '{}'::jsonb)) as fields(field, value)
        where case
            when field = 'kind' then
                jsonb_typeof(value) <> 'string'
                or value #>> '{}' not in ('hiring', 'seeking', 'irrelevant', 'uncertain')
            when field = 'confidence' then
                jsonb_typeof(value) <> 'number'
                or case
                    when jsonb_typeof(value) = 'number'
                        then (value #>> '{}')::numeric not between 0 and 1
                    else false
                end
            when field in (
                'title', 'scene', 'region', 'city', 'district',
                'lodging', 'availability', 'urgency', 'publishedAt'
            ) then jsonb_typeof(value) not in ('string', 'null')
            when field in (
                'salaryMin', 'salaryMax', 'experienceYears', 'hoursPerDay',
                'restDaysPerMonth', 'housingAllowance'
            ) then jsonb_typeof(value) not in ('number', 'null')
            when field in ('roles', 'skills', 'missingFields', 'riskCandidates')
                then jsonb_typeof(value) <> 'array'
            else true
        end
    ) then
        raise exception 'talent review extraction contains an invalid field type'
            using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_each(coalesce(p_updates->'extraction', '{}'::jsonb)) as arrays(field, value)
        cross join lateral jsonb_array_elements(
            case when jsonb_typeof(value) = 'array' then value else '[]'::jsonb end
        ) as entries(entry)
        where field in ('roles', 'skills', 'missingFields')
          and (
              jsonb_typeof(entry) <> 'string'
              or coalesce(entry #>> '{}', '') = ''
          )
    ) then
        raise exception 'talent review extraction contains an invalid array entry'
            using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_array_elements(
            case
                when jsonb_typeof(p_updates->'extraction'->'riskCandidates') = 'array'
                    then p_updates->'extraction'->'riskCandidates'
                else '[]'::jsonb
            end
        ) as risks(risk)
        where jsonb_typeof(risk) <> 'object'
           or not (risk ?& array['code', 'explanation', 'evidence', 'hard_block'])
           or exists (
               select 1
               from jsonb_object_keys(
                   case when jsonb_typeof(risk) = 'object' then risk else '{}'::jsonb end
               ) as keys(key)
               where key <> all(array['code', 'explanation', 'evidence', 'hard_block'])
           )
           or jsonb_typeof(risk->'code') <> 'string'
           or coalesce(risk->>'code', '') = ''
           or jsonb_typeof(risk->'explanation') <> 'string'
           or coalesce(risk->>'explanation', '') = ''
           or jsonb_typeof(risk->'evidence') not in ('string', 'null')
           or jsonb_typeof(risk->'hard_block') <> 'boolean'
    ) then
        raise exception 'talent review extraction contains an invalid risk candidate'
            using errcode = '22023';
    end if;
    if p_updates ? 'contact_masked'
       and jsonb_typeof(p_updates->'contact_masked') <> 'array' then
        raise exception 'talent review masked contacts must be a JSON array' using errcode = '22023';
    end if;
    if p_updates ? 'contact_masked'
       and exists (
           select 1
           from jsonb_array_elements(
               case
                   when jsonb_typeof(p_updates->'contact_masked') = 'array'
                       then p_updates->'contact_masked'
                   else '[]'::jsonb
               end
           ) as contacts(contact)
           where jsonb_typeof(contact) <> 'object'
              or not (contact ?& array['contact_type', 'masked_value'])
              or exists (
                  select 1
                  from jsonb_object_keys(
                      case when jsonb_typeof(contact) = 'object' then contact else '{}'::jsonb end
                  ) as keys(key)
                  where key <> all(array['contact_type', 'masked_value'])
              )
              or coalesce(contact->>'contact_type', '') not in ('phone', 'wechat', 'qq', 'other')
              or coalesce(contact->>'masked_value', '')
                  !~ '^[[:alnum:]]{0,4}\*{3,}[[:alnum:]]{0,4}$'
       ) then
        raise exception 'invalid masked talent review contact' using errcode = '22023';
    end if;
    if p_updates ? 'validation_errors'
       and jsonb_typeof(p_updates->'validation_errors') <> 'array' then
        raise exception 'talent review validation errors must be a JSON array' using errcode = '22023';
    end if;
    if p_updates ? 'risk_reasons'
       and jsonb_typeof(p_updates->'risk_reasons') <> 'array' then
        raise exception 'talent review risk reasons must be a JSON array' using errcode = '22023';
    end if;
    if p_updates ? 'risk_reasons'
       and exists (
           select 1
           from jsonb_array_elements(
               case
                   when jsonb_typeof(p_updates->'risk_reasons') = 'array'
                       then p_updates->'risk_reasons'
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
       ) then
        raise exception 'talent review risk reasons contain an invalid entry'
            using errcode = '22023';
    end if;
    if p_updates ? 'duplicate_candidates'
       and jsonb_typeof(p_updates->'duplicate_candidates') <> 'array' then
        raise exception 'talent review duplicate candidates must be a JSON array' using errcode = '22023';
    end if;
    if p_updates ? 'duplicate_candidates'
       and exists (
           select 1
           from jsonb_array_elements(
               case
                   when jsonb_typeof(p_updates->'duplicate_candidates') = 'array'
                       then p_updates->'duplicate_candidates'
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
       ) then
        raise exception 'talent review duplicate candidates contain an invalid entry'
            using errcode = '22023';
    end if;

    update talent_import_items as reviewed
    set
        kind = coalesce(nullif(p_updates->>'kind', ''), reviewed.kind),
        extraction = (
            coalesce(reviewed.extraction, '{}'::jsonb)
            || coalesce(p_updates->'extraction', '{}'::jsonb)
        ) - 'contacts',
        extraction_confidence = coalesce(
            nullif(p_updates->>'extraction_confidence', '')::numeric,
            reviewed.extraction_confidence
        ),
        contact_masked = coalesce(p_updates->'contact_masked', reviewed.contact_masked),
        validation_errors = coalesce(p_updates->'validation_errors', reviewed.validation_errors),
        risk_score = coalesce(nullif(p_updates->>'risk_score', '')::numeric, reviewed.risk_score),
        risk_level = coalesce(nullif(p_updates->>'risk_level', ''), reviewed.risk_level),
        risk_reasons = coalesce(p_updates->'risk_reasons', reviewed.risk_reasons),
        duplicate_candidates = coalesce(
            p_updates->'duplicate_candidates',
            reviewed.duplicate_candidates
        ),
        lock_version = reviewed.lock_version + 1,
        reviewed_at = current_timestamp,
        updated_at = current_timestamp
    where reviewed.item_id = p_item_id
      and reviewed.lock_version = p_lock_version
      and reviewed.status = 'needs_review'
    returning reviewed.item_id, reviewed.lock_version
    into v_item_id, v_lock_version;

    if v_item_id is null then
        return;
    end if;

    insert into talent_review_events (
        item_id,
        actor_type,
        actor_user_id,
        action,
        before_snapshot,
        after_snapshot
    )
    values (
        v_item_id,
        'moderator',
        p_actor_user_id,
        'review_updated',
        jsonb_build_object('lock_version', p_lock_version),
        jsonb_build_object('lock_version', v_lock_version)
    );

    item_id := v_item_id;
    lock_version := v_lock_version;
    return next;
end;
$$ language plpgsql;
