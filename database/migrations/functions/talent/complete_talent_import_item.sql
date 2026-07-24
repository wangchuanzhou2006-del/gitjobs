-- Completes extraction and advances batch counters in the same transaction.
create or replace function complete_talent_import_item(p_completion jsonb)
returns void as $$
declare
    v_batch_id uuid;
begin
    if p_completion is null or jsonb_typeof(p_completion) <> 'object' then
        raise exception 'talent import completion must be a JSON object'
            using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_object_keys(p_completion) as fields(field)
        where field <> all(
            array[
                'item_id',
                'status',
                'extraction',
                'extraction_confidence',
                'contact_candidates_ciphertext',
                'contact_hashes',
                'contact_masked',
                'normalized_fingerprint',
                'validation_errors',
                'risk_score',
                'risk_level',
                'risk_reasons',
                'duplicate_candidates'
            ]
        )
    ) then
        raise exception 'talent import completion contains an unknown field'
            using errcode = '22023';
    end if;
    if not (p_completion ? 'item_id')
       or jsonb_typeof(p_completion->'item_id') <> 'string'
       or coalesce(p_completion->>'item_id', '')
           !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$' then
        raise exception 'invalid talent import completion item ID' using errcode = '22023';
    end if;
    if p_completion ? 'status'
       and (
           jsonb_typeof(p_completion->'status') <> 'string'
           or p_completion->>'status' <> 'needs_review'
       ) then
        raise exception 'talent import completion can only require review' using errcode = '22023';
    end if;
    if jsonb_typeof(coalesce(p_completion->'extraction', 'null'::jsonb)) <> 'object' then
        raise exception 'talent extraction must be a JSON object' using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_object_keys(p_completion->'extraction') as fields(field)
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
                'contacts',
                'missingFields',
                'riskCandidates'
            ]
        )
    ) then
        raise exception 'talent extraction contains an unknown field' using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_each(p_completion->'extraction') as fields(field, value)
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
            when field in (
                'roles', 'skills', 'contacts', 'missingFields', 'riskCandidates'
            ) then jsonb_typeof(value) <> 'array'
            else true
        end
    ) then
        raise exception 'talent extraction contains an invalid field type'
            using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_each(p_completion->'extraction') as arrays(field, value)
        cross join lateral jsonb_array_elements(
            case when jsonb_typeof(value) = 'array' then value else '[]'::jsonb end
        ) as entries(entry)
        where field in ('roles', 'skills', 'missingFields')
          and (
              jsonb_typeof(entry) <> 'string'
              or coalesce(entry #>> '{}', '') = ''
          )
    ) then
        raise exception 'talent extraction contains an invalid array entry'
            using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_array_elements(
            case
                when jsonb_typeof(p_completion->'extraction'->'contacts') = 'array'
                    then p_completion->'extraction'->'contacts'
                else '[]'::jsonb
            end
        ) as contacts(contact)
        where jsonb_typeof(contact) <> 'object'
           or not (contact ?& array['contact_type', 'value'])
           or exists (
               select 1
               from jsonb_object_keys(
                   case when jsonb_typeof(contact) = 'object' then contact else '{}'::jsonb end
               ) as keys(key)
               where key <> all(array['contact_type', 'value'])
           )
           or coalesce(contact->>'contact_type', '') not in ('phone', 'wechat', 'qq', 'other')
           or jsonb_typeof(contact->'value') <> 'string'
           or coalesce(contact->>'value', '') = ''
    ) then
        raise exception 'talent extraction contains an invalid contact candidate'
            using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_array_elements(
            case
                when jsonb_typeof(p_completion->'extraction'->'riskCandidates') = 'array'
                    then p_completion->'extraction'->'riskCandidates'
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
        raise exception 'talent extraction contains an invalid risk candidate'
            using errcode = '22023';
    end if;
    if p_completion ? 'extraction_confidence'
       and (
           jsonb_typeof(p_completion->'extraction_confidence') <> 'number'
           or case
               when jsonb_typeof(p_completion->'extraction_confidence') = 'number'
                   then (p_completion->>'extraction_confidence')::numeric not between 0 and 1
               else false
           end
    ) then
        raise exception 'invalid talent extraction confidence' using errcode = '22023';
    end if;
    if p_completion ? 'contact_candidates_ciphertext'
       and jsonb_typeof(p_completion->'contact_candidates_ciphertext')
           not in ('string', 'null') then
        raise exception 'invalid encrypted talent contact payload' using errcode = '22023';
    end if;
    if nullif(p_completion->>'contact_candidates_ciphertext', '') is not null
       and (p_completion->>'contact_candidates_ciphertext') !~ '^(?:[0-9a-fA-F]{2})+$' then
        raise exception 'invalid encrypted talent contact payload' using errcode = '22023';
    end if;
    if p_completion ? 'contact_hashes'
       and jsonb_typeof(p_completion->'contact_hashes') <> 'array' then
        raise exception 'talent completion contact hashes must be a JSON array'
            using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_array_elements(
            case
                when jsonb_typeof(p_completion->'contact_hashes') = 'array'
                    then p_completion->'contact_hashes'
                else '[]'::jsonb
            end
        ) as hashes(hash)
        where jsonb_typeof(hash) <> 'string'
           or coalesce(hash #>> '{}', '') = ''
    ) then
        raise exception 'talent completion contact hashes contain an invalid entry'
            using errcode = '22023';
    end if;
    if p_completion ? 'contact_masked'
       and jsonb_typeof(p_completion->'contact_masked') <> 'array' then
        raise exception 'talent completion masked contacts must be a JSON array'
            using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_array_elements(
            case
                when jsonb_typeof(p_completion->'contact_masked') = 'array'
                    then p_completion->'contact_masked'
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
        raise exception 'invalid masked talent completion contact' using errcode = '22023';
    end if;
    if p_completion ? 'normalized_fingerprint'
       and jsonb_typeof(p_completion->'normalized_fingerprint')
           not in ('string', 'null') then
        raise exception 'invalid talent completion fingerprint' using errcode = '22023';
    end if;
    if p_completion ? 'validation_errors'
       and jsonb_typeof(p_completion->'validation_errors') <> 'array' then
        raise exception 'talent completion validation errors must be a JSON array'
            using errcode = '22023';
    end if;
    if p_completion ? 'risk_score'
       and (
           jsonb_typeof(p_completion->'risk_score') <> 'number'
           or case
               when jsonb_typeof(p_completion->'risk_score') = 'number'
                   then (p_completion->>'risk_score')::numeric not between 0 and 100
               else false
           end
       ) then
        raise exception 'invalid talent completion risk score' using errcode = '22023';
    end if;
    if p_completion ? 'risk_level'
       and (
           jsonb_typeof(p_completion->'risk_level') <> 'string'
           or p_completion->>'risk_level' not in ('low', 'medium', 'high')
       ) then
        raise exception 'invalid talent completion risk level' using errcode = '22023';
    end if;
    if p_completion ? 'risk_reasons'
       and jsonb_typeof(p_completion->'risk_reasons') <> 'array' then
        raise exception 'talent completion risk reasons must be a JSON array'
            using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_array_elements(
            case
                when jsonb_typeof(p_completion->'risk_reasons') = 'array'
                    then p_completion->'risk_reasons'
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
        raise exception 'talent completion risk reasons contain an invalid entry'
            using errcode = '22023';
    end if;
    if p_completion ? 'duplicate_candidates'
       and jsonb_typeof(p_completion->'duplicate_candidates') <> 'array' then
        raise exception 'talent completion duplicate candidates must be a JSON array'
            using errcode = '22023';
    end if;
    if exists (
        select 1
        from jsonb_array_elements(
            case
                when jsonb_typeof(p_completion->'duplicate_candidates') = 'array'
                    then p_completion->'duplicate_candidates'
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
        raise exception 'talent completion duplicate candidates contain an invalid entry'
            using errcode = '22023';
    end if;

    update talent_import_items
    set
        status = 'needs_review',
        extraction = (p_completion->'extraction') - 'contacts',
        extraction_confidence = nullif(p_completion->>'extraction_confidence', '')::numeric,
        contact_candidates_ciphertext = nullif(p_completion->>'contact_candidates_ciphertext', ''),
        contact_hashes = coalesce(
            array(select jsonb_array_elements_text(coalesce(p_completion->'contact_hashes', '[]'::jsonb))),
            '{}'::text[]
        ),
        contact_masked = coalesce(p_completion->'contact_masked', '[]'::jsonb),
        normalized_fingerprint = nullif(p_completion->>'normalized_fingerprint', ''),
        validation_errors = coalesce(p_completion->'validation_errors', '[]'::jsonb),
        risk_score = nullif(p_completion->>'risk_score', '')::numeric,
        risk_level = nullif(p_completion->>'risk_level', ''),
        risk_reasons = coalesce(p_completion->'risk_reasons', '[]'::jsonb),
        duplicate_candidates = coalesce(p_completion->'duplicate_candidates', '[]'::jsonb),
        processed_at = current_timestamp,
        updated_at = current_timestamp
    where item_id = (p_completion->>'item_id')::uuid
      and status = 'extracting'
    returning batch_id into v_batch_id;

    if v_batch_id is null then
        raise exception 'talent import item is not extracting';
    end if;

    update talent_import_batches
    set
        processed_count = processed_count + 1,
        status = case
            when processed_count + 1 = item_count and failed_count > 0 then 'partially_failed'
            when processed_count + 1 = item_count then 'completed'
            else 'processing'
        end,
        completed_at = case
            when processed_count + 1 = item_count then current_timestamp
            else null
        end
    where batch_id = v_batch_id
      and processed_count < item_count;

    if not found then
        raise exception 'talent import batch counters are invalid';
    end if;
end;
$$ language plpgsql;
