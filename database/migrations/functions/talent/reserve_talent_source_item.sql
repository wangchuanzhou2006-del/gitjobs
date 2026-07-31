create or replace function reserve_talent_source_item(
    p_subscription_id uuid,
    p_source_item_id text,
    p_content_fingerprint text
)
returns boolean as $$
declare
    v_inserted integer;
begin
    if trim(coalesce(p_source_item_id, '')) = ''
       or p_content_fingerprint !~ '^[0-9a-f]{64}$' then
        raise exception 'invalid source item identity' using errcode = '22023';
    end if;
    insert into talent_source_seen_items (subscription_id, source_item_id, content_fingerprint)
    values (p_subscription_id, p_source_item_id, p_content_fingerprint)
    on conflict do nothing;
    get diagnostics v_inserted = row_count;
    return v_inserted > 0;
end;
$$ language plpgsql;
