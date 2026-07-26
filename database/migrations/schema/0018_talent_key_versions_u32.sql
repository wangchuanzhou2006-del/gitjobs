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
