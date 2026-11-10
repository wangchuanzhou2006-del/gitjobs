begin;
select plan(5);

\set sourceCreator '00000000-0000-0000-0000-000000000501'

insert into "user" (auth_hash, email, name, user_id, username)
values (
    decode('01', 'hex'),
    'talent-source@example.com',
    'Talent Source',
    :'sourceCreator',
    'talent_source_creator'
);

select lives_ok(
    format(
        $$select upsert_talent_source('wechat_group', 'Synthetic Hiring Group', %L::uuid)$$,
        :'sourceCreator'
    ),
    'a talent source can be created'
);

select is(
    upsert_talent_source('wechat_group', 'synthetic hiring group', :'sourceCreator'),
    (
        select source_id
        from talent_sources
        where source_type = 'wechat_group'
          and lower(name) = 'synthetic hiring group'
    ),
    'source upsert is case-insensitive'
);

select is(
    (select count(*) from talent_sources where source_type = 'wechat_group'),
    1::bigint,
    'case-insensitive source upsert does not create a duplicate'
);

select is(
    (select trust_status from talent_sources where source_type = 'wechat_group'),
    'new',
    'a new source remains untrusted'
);

select is(
    (select auto_publish_enabled from talent_sources where source_type = 'wechat_group'),
    false,
    'a new source has automatic publishing disabled'
);

select * from finish();
rollback;
