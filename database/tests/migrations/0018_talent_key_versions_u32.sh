#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
migration="$script_dir/../../migrations/schema/0018_talent_key_versions_u32.sql"
psql_bin=${PSQL_BIN:-psql}
test_schema=talent_migration_0018_test
temp_dir=$(mktemp -d)
up_sql="$temp_dir/up.sql"
down_sql="$temp_dir/down.sql"
down_error="$temp_dir/down.err"

cleanup() {
    "$psql_bin" -X -v ON_ERROR_STOP=1 -qAtc \
        "drop schema if exists $test_schema cascade" >/dev/null 2>&1 || true
    rm -f "$up_sql" "$down_sql" "$down_error"
    rmdir "$temp_dir"
}
trap cleanup EXIT INT TERM

awk -v up="$up_sql" -v down="$down_sql" '
    BEGIN { section = "up" }
    /^---- create above \/ drop below ----$/ { section = "down"; next }
    section == "up" { print > up }
    section == "down" { print > down }
' "$migration"

run_part() {
    {
        printf 'set search_path to %s;\n' "$test_schema"
        cat "$1"
    } | "$psql_bin" -X -v ON_ERROR_STOP=1 --single-transaction -q >/dev/null
}

assert_types() {
    expected=$1
    actual=$("$psql_bin" -X -qAtc "
        select string_agg(table_name || ':' || data_type, ',' order by table_name)
        from information_schema.columns
        where table_schema = '$test_schema'
          and column_name = 'encryption_key_version'
          and table_name in (
              'talent_import_batches',
              'talent_import_items',
              'talent_contacts'
          )
    ")
    test "$actual" = "$expected"
}

assert_u32_constraints() {
    expected=$1
    actual=$("$psql_bin" -X -qAtc "
        select count(*)
        from pg_constraint constraint_definition
        join pg_class relation on relation.oid = constraint_definition.conrelid
        join pg_namespace namespace on namespace.oid = relation.relnamespace
        where namespace.nspname = '$test_schema'
          and constraint_definition.conname in (
              'talent_import_batches_encryption_key_version_u32',
              'talent_import_items_encryption_key_version_u32',
              'talent_contacts_encryption_key_version_u32'
          )
    ")
    test "$actual" = "$expected"
}

"$psql_bin" -X -v ON_ERROR_STOP=1 -q <<SQL
drop schema if exists $test_schema cascade;
create schema $test_schema;
create table $test_schema.talent_import_batches (encryption_key_version integer);
create table $test_schema.talent_import_items (encryption_key_version integer);
create table $test_schema.talent_contacts (encryption_key_version integer);
SQL

integer_types='talent_contacts:integer,talent_import_batches:integer,talent_import_items:integer'
bigint_types='talent_contacts:bigint,talent_import_batches:bigint,talent_import_items:bigint'

run_part "$up_sql"
assert_types "$bigint_types"
assert_u32_constraints 3

run_part "$down_sql"
assert_types "$integer_types"
assert_u32_constraints 0

run_part "$up_sql"
assert_types "$bigint_types"
assert_u32_constraints 3

"$psql_bin" -X -v ON_ERROR_STOP=1 -qAtc "
    insert into $test_schema.talent_import_batches (encryption_key_version)
    values (2147483648)
"

if run_part "$down_sql" 2>"$down_error"; then
    echo 'expected 0018 down migration to refuse a lossy integer cast' >&2
    exit 1
fi

grep -F 'cannot safely roll back talent key versions: value is outside integer range' \
    "$down_error" >/dev/null
assert_types "$bigint_types"
assert_u32_constraints 3

echo '0018 migration up-down-up and safe-refusal checks passed'
