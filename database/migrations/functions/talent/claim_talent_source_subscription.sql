create or replace function claim_talent_source_subscription()
returns jsonb as $$
declare
    v_claim jsonb;
begin
    with due as (
        select subscription_id
        from talent_source_subscriptions
        where enabled
          and next_run_at <= current_timestamp
          and (claimed_at is null or claimed_at < current_timestamp - interval '15 minutes')
        order by next_run_at, subscription_id
        for update skip locked
        limit 1
    ), claimed as (
        update talent_source_subscriptions subscription
        set claimed_at = current_timestamp, updated_at = current_timestamp
        from due
        where subscription.subscription_id = due.subscription_id
        returning subscription.*
    ), run as (
        insert into talent_source_runs (subscription_id)
        select subscription_id from claimed
        returning run_id, subscription_id
    )
    select jsonb_build_object(
        'run_id', run.run_id,
        'subscription', to_jsonb(claimed) || jsonb_build_object('source_name', source.name)
    )
    into v_claim
    from claimed
    join run using (subscription_id)
    join talent_sources source on source.source_id = claimed.source_id;
    return v_claim;
end;
$$ language plpgsql;
