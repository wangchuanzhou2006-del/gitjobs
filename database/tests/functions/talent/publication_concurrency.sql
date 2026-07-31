begin;
select plan(5);

create extension if not exists dblink;

select dblink_connect(
    'talent_structured_a',
    format(
        'dbname=%L user=%L application_name=%L',
        current_database(),
        current_user,
        'talent-structured-a'
    )
);
select dblink_connect(
    'talent_structured_b',
    format(
        'dbname=%L user=%L application_name=%L',
        current_database(),
        current_user,
        'talent-structured-b'
    )
);

select dblink_exec(
    'talent_structured_a',
    $cleanup$
    delete from talent_review_events
    where item_id in (
        '00000000-0000-0000-0000-000000000731'::uuid,
        '00000000-0000-0000-0000-000000000732'::uuid
    );
    delete from talent_contacts
    where record_id in (
        select record_id
        from talent_records
        where title in ('Concurrent Structured Alpha', 'Concurrent Structured Beta')
    );
    delete from talent_import_items
    where item_id in (
        '00000000-0000-0000-0000-000000000731'::uuid,
        '00000000-0000-0000-0000-000000000732'::uuid
    );
    delete from talent_records
    where title in ('Concurrent Structured Alpha', 'Concurrent Structured Beta');
    delete from talent_import_batches
    where batch_id in (
        '00000000-0000-0000-0000-000000000721'::uuid,
        '00000000-0000-0000-0000-000000000722'::uuid
    );
    delete from talent_sources
    where source_id = '00000000-0000-0000-0000-000000000711'::uuid;
    delete from "user"
    where user_id = '00000000-0000-0000-0000-000000000701'::uuid;
    drop table if exists talent_structured_publish_outcomes;
    drop table if exists talent_structured_publish_settings;
    $cleanup$
);

select dblink_exec(
    'talent_structured_a',
    $setup$
    create table talent_structured_publish_settings as
    select auto_publish_enabled, updated_by, updated_at
    from talent_runtime_settings
    where singleton;

    create table talent_structured_publish_outcomes (
        session_name text primary key,
        outcome text not null
    );

    insert into "user" (auth_hash, email, name, user_id, username)
    values (
        decode('71', 'hex'),
        'talent-structured-concurrency@example.com',
        'Talent Structured Concurrency',
        '00000000-0000-0000-0000-000000000701',
        'talent_structured_concurrency'
    );

    insert into talent_sources (
        source_id,
        source_type,
        name,
        trust_status,
        auto_publish_enabled,
        confirmed_by,
        confirmed_at
    )
    values (
        '00000000-0000-0000-0000-000000000711',
        'other',
        'Synthetic Structured Concurrency Source',
        'trusted',
        true,
        '00000000-0000-0000-0000-000000000701',
        current_timestamp
    );

    update talent_runtime_settings
    set
        auto_publish_enabled = true,
        updated_by = '00000000-0000-0000-0000-000000000701',
        updated_at = current_timestamp
    where singleton;

    insert into talent_import_batches (
        batch_id,
        source_id,
        status,
        item_count,
        processed_count
    )
    values
        (
            '00000000-0000-0000-0000-000000000721',
            '00000000-0000-0000-0000-000000000711',
            'completed',
            1,
            1
        ),
        (
            '00000000-0000-0000-0000-000000000722',
            '00000000-0000-0000-0000-000000000711',
            'completed',
            1,
            1
        );

    insert into talent_import_items (
        item_id,
        batch_id,
        position,
        contact_hashes,
        normalized_fingerprint,
        kind,
        status,
        extraction,
        extraction_confidence,
        validation_errors,
        risk_score,
        risk_level,
        risk_reasons,
        duplicate_candidates,
        processed_at
    )
    values
        (
            '00000000-0000-0000-0000-000000000731',
            '00000000-0000-0000-0000-000000000721',
            0,
            array['structured-contact-alpha'],
            'structured-fingerprint-alpha',
            'hiring',
            'needs_review',
            '{"kind":"hiring","title":"Concurrent Structured Alpha","city":"Test City","salaryMin":6000,"salaryMax":8000,"roles":["Synthetic Cook"],"publishedAt":"2026-07-29T00:00:00Z","missingFields":[]}'::jsonb,
            0.99,
            '[]'::jsonb,
            5,
            'low',
            '[]'::jsonb,
            '[]'::jsonb,
            current_timestamp
        ),
        (
            '00000000-0000-0000-0000-000000000732',
            '00000000-0000-0000-0000-000000000722',
            0,
            array['structured-contact-beta'],
            'structured-fingerprint-beta',
            'hiring',
            'needs_review',
            '{"kind":"hiring","title":"Concurrent Structured Beta","city":"Test City","salaryMin":7000,"salaryMax":9000,"roles":["Synthetic Cook"],"publishedAt":"2026-07-29T00:00:00Z","missingFields":[]}'::jsonb,
            0.99,
            '[]'::jsonb,
            5,
            'low',
            '[]'::jsonb,
            '[]'::jsonb,
            current_timestamp
        );
    $setup$
);

create function pg_temp.wait_for_structured_publishers()
returns integer as $$
declare
    v_waiting integer;
    v_deadline timestamptz := clock_timestamp() + interval '5 seconds';
begin
    loop
        select count(*)::integer
        into v_waiting
        from pg_stat_activity
        where application_name in ('talent-structured-a', 'talent-structured-b')
          and wait_event = 'Advisory';

        exit when v_waiting = 2 or clock_timestamp() >= v_deadline;
        perform pg_sleep(0.02);
    end loop;
    return v_waiting;
end;
$$ language plpgsql;

select pg_advisory_lock(hashtextextended('talent:auto-publish:structured-v1', 0));

select dblink_send_query(
    'talent_structured_a',
    $session_a$
    do $publish$
    begin
        begin
            perform publish_talent_item(
                '00000000-0000-0000-0000-000000000731'::uuid,
                0,
                null,
                true,
                '{}'::jsonb,
                '[{"contact_type":"wechat","ciphertext":"7171","encryption_key_version":1,"normalized_hash":"structured-contact-alpha","masked_value":"alp***731"}]'::jsonb
            );
            insert into talent_structured_publish_outcomes values ('a', 'published');
        exception when sqlstate '22023' then
            if sqlerrm <> 'automatic talent publication gates failed' then
                raise;
            end if;
            insert into talent_structured_publish_outcomes values ('a', 'blocked');
        end;
    end;
    $publish$;
    $session_a$
);

select dblink_send_query(
    'talent_structured_b',
    $session_b$
    do $publish$
    begin
        begin
            perform publish_talent_item(
                '00000000-0000-0000-0000-000000000732'::uuid,
                0,
                null,
                true,
                '{}'::jsonb,
                '[{"contact_type":"wechat","ciphertext":"7272","encryption_key_version":1,"normalized_hash":"structured-contact-beta","masked_value":"bet***732"}]'::jsonb
            );
            insert into talent_structured_publish_outcomes values ('b', 'published');
        exception when sqlstate '22023' then
            if sqlerrm <> 'automatic talent publication gates failed' then
                raise;
            end if;
            insert into talent_structured_publish_outcomes values ('b', 'blocked');
        end;
    end;
    $publish$;
    $session_b$
);

select is(
    pg_temp.wait_for_structured_publishers(),
    2,
    'two independent sessions reach the final structured gate concurrently'
);

select pg_advisory_unlock(hashtextextended('talent:auto-publish:structured-v1', 0));

select * from dblink_get_result('talent_structured_a') as result(status text);
select * from dblink_get_result('talent_structured_b') as result(status text);

select is(
    (
        select count(*)
        from dblink(
            'talent_structured_a',
            $$select session_name, outcome from talent_structured_publish_outcomes$$
        ) as outcomes(session_name text, outcome text)
        where outcome = 'published'
    ),
    1::bigint,
    'only one structurally similar item auto-publishes'
);

select is(
    (
        select count(*)
        from dblink(
            'talent_structured_a',
            $$select session_name, outcome from talent_structured_publish_outcomes$$
        ) as outcomes(session_name text, outcome text)
        where outcome = 'blocked'
    ),
    1::bigint,
    'the other concurrent publisher is rejected by the authoritative gate'
);

select is(
    (
        select published_count
        from dblink(
            'talent_structured_a',
            $$
            select count(*)::bigint
            from talent_records
            where title in ('Concurrent Structured Alpha', 'Concurrent Structured Beta')
            $$
        ) as records(published_count bigint)
    ),
    1::bigint,
    'concurrent structured duplicates create at most one formal record'
);

select is(
    (
        select guarded_count
        from dblink(
            'talent_structured_a',
            $$
            select count(*)::bigint
            from talent_import_items
            where item_id in (
                '00000000-0000-0000-0000-000000000731'::uuid,
                '00000000-0000-0000-0000-000000000732'::uuid
            )
              and status in ('needs_review', 'auto_published')
            $$
        ) as items(guarded_count bigint)
    ),
    2::bigint,
    'the rejected item remains reviewable while the winner is auto-published'
);

select dblink_exec(
    'talent_structured_a',
    $cleanup$
    delete from talent_review_events
    where item_id in (
        '00000000-0000-0000-0000-000000000731'::uuid,
        '00000000-0000-0000-0000-000000000732'::uuid
    );
    delete from talent_contacts
    where record_id in (
        select record_id
        from talent_records
        where title in ('Concurrent Structured Alpha', 'Concurrent Structured Beta')
    );
    delete from talent_import_items
    where item_id in (
        '00000000-0000-0000-0000-000000000731'::uuid,
        '00000000-0000-0000-0000-000000000732'::uuid
    );
    delete from talent_records
    where title in ('Concurrent Structured Alpha', 'Concurrent Structured Beta');
    delete from talent_import_batches
    where batch_id in (
        '00000000-0000-0000-0000-000000000721'::uuid,
        '00000000-0000-0000-0000-000000000722'::uuid
    );
    delete from talent_sources
    where source_id = '00000000-0000-0000-0000-000000000711'::uuid;
    update talent_runtime_settings settings
    set
        auto_publish_enabled = saved.auto_publish_enabled,
        updated_by = saved.updated_by,
        updated_at = saved.updated_at
    from talent_structured_publish_settings saved
    where settings.singleton;
    delete from "user"
    where user_id = '00000000-0000-0000-0000-000000000701'::uuid;
    drop table talent_structured_publish_outcomes;
    drop table talent_structured_publish_settings;
    $cleanup$
);

select dblink_disconnect('talent_structured_a');
select dblink_disconnect('talent_structured_b');

select * from finish();
rollback;
