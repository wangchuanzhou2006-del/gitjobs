create or replace function release_talent_source_item(
    p_subscription_id uuid,
    p_source_item_id text,
    p_content_fingerprint text
)
returns boolean as $$
declare
    v_deleted integer;
begin
    delete from talent_source_seen_items
    where subscription_id = p_subscription_id
      and source_item_id = p_source_item_id
      and content_fingerprint = p_content_fingerprint;
    get diagnostics v_deleted = row_count;
    return v_deleted > 0;
end;
$$ language plpgsql;
