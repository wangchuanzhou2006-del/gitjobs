-- Preserves audit structure while replacing every free-text leaf with a digest.
create or replace function talent_audit_safe_jsonb(p_value jsonb)
returns jsonb as $$
declare
    v_kind text;
    v_result jsonb;
    v_text text;
begin
    if p_value is null then
        return null;
    end if;

    v_kind := jsonb_typeof(p_value);
    case v_kind
        when 'object' then
            select coalesce(
                jsonb_object_agg(entry.key, talent_audit_safe_jsonb(entry.value)),
                '{}'::jsonb
            )
            into v_result
            from jsonb_each(p_value) as entry;
            return v_result;
        when 'array' then
            select coalesce(
                jsonb_agg(talent_audit_safe_jsonb(entry.value) order by entry.ordinality),
                '[]'::jsonb
            )
            into v_result
            from jsonb_array_elements(p_value) with ordinality as entry(value, ordinality);
            return v_result;
        when 'string' then
            v_text := p_value #>> '{}';
            return jsonb_build_object(
                'redacted', true,
                'sha256', encode(digest(convert_to(v_text, 'UTF8'), 'sha256'), 'hex'),
                'character_count', char_length(v_text)
            );
        when 'number', 'boolean', 'null' then
            return p_value;
        else
            raise exception 'unsupported talent audit JSON value' using errcode = '22023';
    end case;
end;
$$ language plpgsql immutable parallel safe;

create or replace function talent_review_state_snapshot(p_item talent_import_items)
returns jsonb as $$
    select jsonb_build_object(
        'kind', p_item.kind,
        'extraction', talent_audit_safe_jsonb(coalesce(p_item.extraction, '{}'::jsonb)),
        'extraction_confidence', p_item.extraction_confidence,
        'contact_masked', talent_audit_safe_jsonb(coalesce(p_item.contact_masked, '[]'::jsonb)),
        'validation_errors', talent_audit_safe_jsonb(coalesce(p_item.validation_errors, '[]'::jsonb)),
        'risk_score', p_item.risk_score,
        'risk_level', p_item.risk_level,
        'risk_reasons', talent_audit_safe_jsonb(coalesce(p_item.risk_reasons, '[]'::jsonb)),
        'duplicate_candidates', talent_audit_safe_jsonb(
            coalesce(p_item.duplicate_candidates, '[]'::jsonb)
        ),
        'normalized_fingerprint_present', nullif(p_item.normalized_fingerprint, '') is not null,
        'lock_version', p_item.lock_version,
        'status', p_item.status
    );
$$ language sql immutable parallel safe;
