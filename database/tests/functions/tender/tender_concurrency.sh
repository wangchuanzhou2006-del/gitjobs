#!/bin/sh
# Run against a migrated, disposable PostgreSQL test database. This exercises
# real concurrent sessions; the pgTAP .sql tests cannot control two sessions.
#
set -eu

psql_bin=${PSQL_BIN:-psql}

if [ "${TENDER_CONCURRENCY_TEST:-}" != 1 ]; then
    echo 'set TENDER_CONCURRENCY_TEST=1 only for a disposable test database' >&2
    exit 1
fi
if [ -z "${PGDATABASE:-}" ]; then
    echo 'PGDATABASE must name a dedicated test database' >&2
    exit 1
fi
case $PGDATABASE in
    tests|*_tests) ;;
    *) echo "refusing to run against non-test database: $PGDATABASE" >&2; exit 1 ;;
esac

q() {
    "$psql_bin" -X -v ON_ERROR_STOP=1 -qAt -c "$1"
}

actual_database=$(q 'select current_database()')
if [ "$actual_database" != "$PGDATABASE" ]; then
    echo "database mismatch: expected $PGDATABASE, connected to $actual_database" >&2
    exit 1
fi
has_pgtap=$(q "select exists (select 1 from pg_extension where extname = 'pgtap');")
if [ "$has_pgtap" != t ]; then
    echo 'refusing to run without the pgTAP test-database extension' >&2
    exit 1
fi

tmp_dir=$(mktemp -d)
owner_created=0
owner=
email=
holder_pid=
worker_pid=
session_a_pid=
session_b_pid=
watchdog_pid=

cleanup() {
    trap - 0
    for child in "$watchdog_pid" "$worker_pid" "$holder_pid" "$session_a_pid" "$session_b_pid"; do
        if [ -n "$child" ]; then
            kill "$child" 2>/dev/null || true
            wait "$child" 2>/dev/null || true
        fi
    done
    exec 3>&- 4>&- 5>&-
    if [ "$owner_created" -eq 1 ]; then
        "$psql_bin" -X -v ON_ERROR_STOP=1 -qAt -c \
            "delete from \"user\" where user_id = '$owner' and email = '$email';" \
            >/dev/null 2>&1 || true
    fi
    if [ -n "$tmp_dir" ] && [ "$tmp_dir" != / ] && [ -d "$tmp_dir" ]; then
        rm -f "$tmp_dir"/*
        rmdir "$tmp_dir" 2>/dev/null || true
    fi
}
trap cleanup 0
trap 'exit 130' INT
trap 'exit 143' TERM

fail() {
    echo "$1" >&2
    exit 1
}

wait_for_file() {
    wf_file=$1
    wf_pid=$2
    wf_label=$3
    wf_tries=0
    while [ ! -f "$wf_file" ]; do
        kill -0 "$wf_pid" 2>/dev/null || fail "$wf_label exited before signalling"
        [ "$wf_tries" -lt 100 ] || fail "timed out waiting for $wf_label"
        wf_tries=$((wf_tries + 1))
        sleep 0.1
    done
}

wait_for_db_lock() {
    watch_app=$1
    watch_pid=$2
    watch_tries=0
    while :; do
        waiting=$(q "select count(*) from pg_stat_activity where application_name = '$watch_app' and wait_event_type = 'Lock';")
        [ "$waiting" = 1 ] && return
        kill -0 "$watch_pid" 2>/dev/null || fail "$watch_app exited before waiting for the document lock"
        [ "$watch_tries" -lt 80 ] || fail "timed out waiting for $watch_app to block"
        watch_tries=$((watch_tries + 1))
        sleep 0.1
    done
}

start_holder() {
    holder_name=$1
    holder_sql=$2
    holder_fifo="$tmp_dir/$holder_name.fifo"
    holder_out="$tmp_dir/$holder_name.out"
    holder_err="$tmp_dir/$holder_name.err"
    holder_marker="$tmp_dir/$holder_name.ready"
    mkfifo "$holder_fifo"
    "$psql_bin" -X -v ON_ERROR_STOP=1 -qAt -f "$holder_fifo" >"$holder_out" 2>"$holder_err" &
    holder_pid=$!
    exec 3>"$holder_fifo"
    printf 'begin;\n%s\n\\! touch %s\n' "$holder_sql" "$holder_marker" >&3
    wait_for_file "$holder_marker" "$holder_pid" "$holder_name"
}

release_holder() {
    printf '%s;\n' "$1" >&3
    exec 3>&-
    if ! wait "$holder_pid"; then
        cat "$holder_err" >&2
        fail 'holder session failed'
    fi
    holder_pid=
}

owner=$(q 'select gen_random_uuid()')
email="tender-concurrency-$owner@example.invalid"
sha256='00000000000000000000000000000000000000000000000000000000000000c1'
q "insert into \"user\" (auth_hash, email, name, user_id, username) values (decode('02','hex'), '$email', 'Concurrency', '$owner', 'tender_concurrency_$owner');"
owner_created=1
project_id=$(q "select create_tender_project(jsonb_build_object('user_id', '$owner', 'title', 'tender concurrency test', 'source_url', null))::text;")

# Seed the document with a throwaway config; the create function owns the
# document insert/upsert, so this also proves the document exists beforehand.
q "select create_tender_document_job(jsonb_build_object('user_id','$owner','project_id','$project_id','storage_key','docs/seed.pdf','sha256','$sha256','size_bytes',1,'filename','seed.pdf','analysis_config_hash','seed-config','model_provider','openai','model_name','m','prompt_version','p'));" >/dev/null
document_id=$(q "select document_id::text from tender_documents where project_id = '$project_id' and sha256 = '$sha256';")

job_json() {
    printf "jsonb_build_object('user_id','%s','project_id','%s','storage_key','docs/seed.pdf','sha256','%s','size_bytes',1,'filename','seed.pdf','analysis_config_hash','%s','model_provider','openai','model_name','m','prompt_version','p')" \
        "$owner" "$project_id" "$sha256" "$1"
}

# Test 1: A holds the document row after creating a new-config job. Start B
# only after A signals readiness, observe B waiting on a PostgreSQL lock, then
# commit A. Both function calls must return the same real job_id.
cfg=config-new
start_holder create_a "select jsonb_extract_path_text(create_tender_document_job($(job_json "$cfg")), 'job_id');"
a_out=$holder_out
worker_app="tender-concurrency-$owner"
worker_out="$tmp_dir/create_b.out"
worker_err="$tmp_dir/create_b.err"
PGAPPNAME="$worker_app" "$psql_bin" -X -v ON_ERROR_STOP=1 -qAt -c \
    "select jsonb_extract_path_text(create_tender_document_job($(job_json "$cfg")), 'job_id');" \
    >"$worker_out" 2>"$worker_err" &
worker_pid=$!
wait_for_db_lock "$worker_app" "$worker_pid"
release_holder commit
if ! wait "$worker_pid"; then
    cat "$worker_err" >&2
    fail 'concurrent create failed'
fi
worker_pid=
a_job=$(sed -n '1p' "$a_out" | tr -d '[:space:]')
b_job=$(sed -n '1p' "$worker_out" | tr -d '[:space:]')
[ -n "$a_job" ] && [ "$a_job" = "$b_job" ] || fail "concurrent creates returned different or empty job IDs: $a_job / $b_job"
count=$(q "select count(*) from tender_analysis_jobs where document_id = '$document_id' and analysis_config_hash = '$cfg' and status <> 'failed';")
[ "$count" = 1 ] || fail "expected one reusable job, found $count"
echo 'PASS: two concurrent creates reuse one non-null job_id'

# Test 2: replay the former three-statement algorithm in two persistent
# sessions. The shell hands control between A and B after each SQL statement:
# A finds none -> B inserts -> A conflicts -> B fails -> A finds none.
cfg=config-race
a_fifo="$tmp_dir/legacy_a.fifo"
b_fifo="$tmp_dir/legacy_b.fifo"
a_out="$tmp_dir/legacy_a.out"
b_out="$tmp_dir/legacy_b.out"
a_err="$tmp_dir/legacy_a.err"
b_err="$tmp_dir/legacy_b.err"
mkfifo "$a_fifo" "$b_fifo"
"$psql_bin" -X -v ON_ERROR_STOP=1 -qAt -f "$a_fifo" >"$a_out" 2>"$a_err" &
session_a_pid=$!
"$psql_bin" -X -v ON_ERROR_STOP=1 -qAt -f "$b_fifo" >"$b_out" 2>"$b_err" &
session_b_pid=$!
exec 4>"$a_fifo"
exec 5>"$b_fifo"

printf "select 'find-existing', count(*) from tender_analysis_jobs where document_id = '%s' and analysis_config_hash = '%s' and status <> 'failed';\n\\! touch %s\n" \
    "$document_id" "$cfg" "$tmp_dir/legacy_a_found" >&4
wait_for_file "$tmp_dir/legacy_a_found" "$session_a_pid" 'legacy A lookup'

printf "insert into tender_analysis_jobs (document_id, analysis_config_hash, status) values ('%s', '%s', 'analyzing');\n\\! touch %s\n" \
    "$document_id" "$cfg" "$tmp_dir/legacy_b_inserted" >&5
wait_for_file "$tmp_dir/legacy_b_inserted" "$session_b_pid" 'legacy B insert'

printf "with inserted as (insert into tender_analysis_jobs (document_id, analysis_config_hash, status) values ('%s', '%s', 'queued') on conflict (document_id, analysis_config_hash) where status <> 'failed' do nothing returning job_id) select 'inserted', count(*) from inserted;\n\\! touch %s\n" \
    "$document_id" "$cfg" "$tmp_dir/legacy_a_conflicted" >&4
wait_for_file "$tmp_dir/legacy_a_conflicted" "$session_a_pid" 'legacy A conflict'

printf "update tender_analysis_jobs set status = 'failed' where document_id = '%s' and analysis_config_hash = '%s' and status <> 'failed';\n\\! touch %s\n" \
    "$document_id" "$cfg" "$tmp_dir/legacy_b_failed" >&5
wait_for_file "$tmp_dir/legacy_b_failed" "$session_b_pid" 'legacy B failure'

printf "select 'loser-select', count(*) from tender_analysis_jobs where document_id = '%s' and analysis_config_hash = '%s' and status <> 'failed';\n\\! touch %s\n" \
    "$document_id" "$cfg" "$tmp_dir/legacy_a_empty" >&4
wait_for_file "$tmp_dir/legacy_a_empty" "$session_a_pid" 'legacy A reselect'
exec 4>&- 5>&-
if ! wait "$session_a_pid"; then cat "$a_err" >&2; fail 'legacy A failed'; fi
session_a_pid=
if ! wait "$session_b_pid"; then cat "$b_err" >&2; fail 'legacy B failed'; fi
session_b_pid=
grep -Fx 'find-existing|0' "$a_out" >/dev/null || fail 'legacy lookup did not see zero jobs'
grep -Fx 'inserted|0' "$a_out" >/dev/null || fail 'legacy insert did not conflict'
grep -Fx 'loser-select|0' "$a_out" >/dev/null || fail 'legacy reselect did not expose the empty-ID window'

# The current function must create a real replacement after that failed job.
replacement=$(q "select jsonb_extract_path_text(create_tender_document_job($(job_json "$cfg")), 'job_id');")
[ -n "$replacement" ] || fail 'current create function returned an empty job_id'
count=$(q "select count(*) from tender_analysis_jobs where document_id = '$document_id' and analysis_config_hash = '$cfg' and status <> 'failed';")
[ "$count" = 1 ] || fail "expected one replacement job, found $count"
echo 'PASS: controlled legacy interleaving exposes old empty-ID window; current function returns a real job'

# Test 3: A holds the document row; B must fail with SQLSTATE 55P03 after the
# function's 10-second lock_timeout. A watchdog makes a missing timeout fail
# the test rather than hanging it indefinitely.
cfg=config-timeout
start_holder timeout_a "select document_id from tender_documents where document_id = '$document_id' for update;"
worker_err="$tmp_dir/timeout_b.err"
worker_out="$tmp_dir/timeout_b.out"
PGAPPNAME="tender-timeout-$owner" "$psql_bin" -X -v ON_ERROR_STOP=1 -v VERBOSITY=verbose -qAt -c \
    "select create_tender_document_job($(job_json "$cfg"))::text;" \
    >"$worker_out" 2>"$worker_err" &
worker_pid=$!
( sleep 20; touch "$tmp_dir/watchdog_fired"; kill "$worker_pid" 2>/dev/null || true ) &
watchdog_pid=$!
if wait "$worker_pid"; then
    fail 'create unexpectedly succeeded while document lock was held'
fi
worker_pid=
kill "$watchdog_pid" 2>/dev/null || true
wait "$watchdog_pid" 2>/dev/null || true
watchdog_pid=
release_holder rollback
[ ! -f "$tmp_dir/watchdog_fired" ] || fail 'create hung past the watchdog deadline'
grep -F '55P03' "$worker_err" >/dev/null || {
    cat "$worker_err" >&2
    fail 'expected SQLSTATE 55P03 from lock_timeout'
}
count=$(q "select count(*) from tender_analysis_jobs where document_id = '$document_id' and analysis_config_hash = '$cfg';")
[ "$count" = 0 ] || fail 'timed-out create left a job behind'
echo 'PASS: document lock wait ends with SQLSTATE 55P03'

echo "all tender concurrency checks passed"
