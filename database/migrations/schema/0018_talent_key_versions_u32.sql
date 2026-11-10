-- Store the full positive u32 key-version contract used by the service.
alter table talent_import_batches
    alter column encryption_key_version type bigint
    using encryption_key_version::bigint;

alter table talent_import_batches
    add constraint talent_import_batches_encryption_key_version_u32
    check (encryption_key_version between 1 and 4294967295);

alter table talent_import_items
    alter column encryption_key_version type bigint
    using encryption_key_version::bigint;

alter table talent_import_items
    add constraint talent_import_items_encryption_key_version_u32
    check (encryption_key_version between 1 and 4294967295);

alter table talent_contacts
    alter column encryption_key_version type bigint
    using encryption_key_version::bigint;

alter table talent_contacts
    add constraint talent_contacts_encryption_key_version_u32
    check (encryption_key_version between 1 and 4294967295);

---- create above / drop below ----

do $$
begin
    if exists (
        select 1
        from talent_import_batches
        where encryption_key_version is not null
          and encryption_key_version not between -2147483648 and 2147483647
        union all
        select 1
        from talent_import_items
        where encryption_key_version is not null
          and encryption_key_version not between -2147483648 and 2147483647
        union all
        select 1
        from talent_contacts
        where encryption_key_version is not null
          and encryption_key_version not between -2147483648 and 2147483647
    ) then
        raise exception
            'cannot safely roll back talent key versions: value is outside integer range'
            using errcode = '22003';
    end if;
end;
$$ language plpgsql;

alter table talent_import_batches
    drop constraint talent_import_batches_encryption_key_version_u32,
    alter column encryption_key_version type integer
    using encryption_key_version::integer;

alter table talent_import_items
    drop constraint talent_import_items_encryption_key_version_u32,
    alter column encryption_key_version type integer
    using encryption_key_version::integer;

alter table talent_contacts
    drop constraint talent_contacts_encryption_key_version_u32,
    alter column encryption_key_version type integer
    using encryption_key_version::integer;
