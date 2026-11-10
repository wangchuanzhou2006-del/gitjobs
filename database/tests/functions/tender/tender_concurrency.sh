#!/bin/sh
# Two-session concurrency regression for create_tender_document_job.
#
# Verifies three things that a single pgTAP session cannot:
#   1. ordinary concurrent creates of the same file+config yield one reusable
#      job and a real, identical job_id from both sessions (never empty);
#   2. the previous "find-existing -> INSERT ... ON CONFLICT DO NOTHING ->
#      loser-select" shape returns an empty job_id when the conflicting job is
#      failed between the insert and the re-select (documented repro of the bug
#      the FOR UPDATE lock fixes);
#   3. the FOR UPDATE wait is bounded by lock_timeout: a stuck holder makes the
#      create fail with SQLSTATE 55P03 instead of hanging.
#
# Run inside the gitjobs-db-tests image against the migrated test database,
# exactly like migrations/0018_talent_key_versions_u32.sh:
#
#   PGHOST=postgres PGPORT=5432 PGDATABASE=tests PGUSER=tests PGPASSWORD=tests \
#       sh functions/tender/tender_concurrency.sh
#
set -eu

psql_bin=${PSQL_BIN:-psql}

# Dedicated identity so a rerun never collides with the pgTAP suite (which uses
# 0601/0602 inside a rolled-back transaction). Cleanup deletes this user, which
# cascades through projects, documents, jobs, and reports.
owner='00000000-0000-0000-0000-000000000611'
sha256='00000000000000000000000000000000000000000000000000000000000000c1'
project_title='tender concurrency test'

q() {
    "$psql_bin" -X -v ON_ERROR_STOP=1 -qAt -c "$1"
}

cleanup() {
    "$psql_bin" -X -qAt -c "delete from \"user\" where user_id = '$owner';" >/dev/null 2>&1 || true
}
trap cleanup EXIT INT TERM
cleanup

q "insert into \"user\" (auth_hash, email, name, user_id, username) values (decode('02','hex'), 'tender-concurrency@example.com', 'Concurrency', '$owner', 'tender_concurrency');"
project_id=$(q "select create_tender_project(jsonb_build_object('user_id', '$owner', 'title', '$project_title', 'source_url', null))::text;")

# Seed the document with a throwaway config; the create function owns the
# document insert/upsert, so this also proves the document exists beforehand.
q "select create_tender_document_job(jsonb_build_object('user_id','$owner','project_id','$project_id','storage_key','docs/seed.pdf','sha256','$sha256','size_bytes',1,'filename','seed.pdf','analysis_config_hash','seed-config','model_provider','openai','model_name','m','prompt_version','p'));" >/dev/null
document_id=$(q "select document_id::text from tender_documents where project_id = '$project_id' and sha256 = '$sha256';")

job_json() {
    printf "jsonb_build_object('user_id','%s','project_id','%s','storage_key','docs/seed.pdf','sha256','%s','size_bytes',1,'filename','seed.pdf','analysis_config_hash','%s','model_provider','openai','model_name','m','prompt_version','p')" \
        "$owner" "$project_id" "$sha256" "$1"
}

# ---------------------------------------------------------------------------
# Test 1: concurrent creates of the same new config -> one reusable job.
# Session A opens a transaction, creates the job, then holds the document lock
# for 2s. Session B starts at the same time and must block on the FOR UPDATE
# until A commits, then reuse A's job. Both must return the same non-empty id.
# ---------------------------------------------------------------------------
cfg='config-new'
tmp_a=$(mktemp)
tmp_b=$(mktemp)
( "$psql_bin" -X -v ON_ERROR_STOP=1 -qAt -c "begin; select jsonb_extract_path_text(create_tender_document_job($(job_json "$cfg")), 'job_id'); select pg_sleep(2); commit;" >"$tmp_a" 2>&1 ) &
pid_a=$!
( "$psql_bin" -X -v ON_ERROR_STOP=1 -qAt -c "select jsonb_extract_path_text(create_tender_document_job($(job_json "$cfg")), 'job_id');" >"$tmp_b" 2>&1 ) &
pid_b=$!
wait "$pid_a"
wait "$pid_b"

a_job=$(sed -n '1p' "$tmp_a" | tr -d '[:space:]')
b_job=$(sed -n '1p' "$tmp_b" | tr -d '[:space:]')
[ -n "$a_job" ] || { echo "session A returned an empty job_id" >&2; cat "$tmp_a" >&2; exit 1; }
[ -n "$b_job" ] || { echo "session B returned an empty job_id" >&2; cat "$tmp_b" >&2; exit 1; }
[ "$a_job" = "$b_job" ] || { echo "concurrent creates disagreed on job_id: $a_job vs $b_job" >&2; exit 1; }
count=$(q "select count(*) from tender_analysis_jobs where document_id = '$document_id' and analysis_config_hash = '$cfg' and status <> 'failed';")
[ "$count" = "1" ] || { echo "expected 1 reusable job for $cfg, found $count" >&2; exit 1; }
echo "PASS: concurrent creates of $cfg reused one job ($a_job)"

# ---------------------------------------------------------------------------
# Test 2: the old ON CONFLICT + loser-select shape returned an empty job_id
# when the conflicting job was failed between the insert and the re-select.
# This replays that exact shape with two sessions coordinated by an advisory
# lock: before the conflict there is no reusable job, after the failed job
# there is again none -- which is the empty-job_id the FOR UPDATE lock removes.
# ---------------------------------------------------------------------------
cfg='config-race'
q "delete from tender_analysis_jobs where document_id = '$document_id' and analysis_config_hash = '$cfg';"
tmp_a2=$(mktemp)

# Session B: on each turn, first create a non-failed job, then fail it.
( "$psql_bin" -X -v ON_ERROR_STOP=1 -qAt <<SQL
select pg_sleep(1);
select pg_advisory_lock(2000);
insert into tender_analysis_jobs (document_id, analysis_config_hash, status)
values ('$document_id', '$cfg', 'analyzing');
select pg_advisory_unlock(2000);
select pg_advisory_lock(2000);
update tender_analysis_jobs set status = 'failed'
where document_id = '$document_id' and analysis_config_hash = '$cfg' and status <> 'failed';
select pg_advisory_unlock(2000);
SQL
) >/dev/null 2>&1 &
pid_b=$!

# Session A: replay the old shape between hand-offs.
"$psql_bin" -X -v ON_ERROR_STOP=1 -qAt <<SQL >"$tmp_a2"
select pg_advisory_lock(2000);
select 'find-existing', count(*) from tender_analysis_jobs
 where document_id = '$document_id' and analysis_config_hash = '$cfg' and status <> 'failed';
select pg_advisory_unlock(2000);
select pg_advisory_lock(2000);
insert into tender_analysis_jobs (document_id, analysis_config_hash, status)
values ('$document_id', '$cfg', 'queued')
on conflict (document_id, analysis_config_hash) where status <> 'failed' do nothing;
select pg_advisory_unlock(2000);
select pg_advisory_lock(2000);
select 'loser-select', count(*) from tender_analysis_jobs
 where document_id = '$document_id' and analysis_config_hash = '$cfg' and status <> 'failed';
select pg_advisory_unlock(2000);
SQL

wait "$pid_b"
grep -F 'find-existing|0' "$tmp_a2" >/dev/null || { echo "expected find-existing to see no reusable job" >&2; cat "$tmp_a2" >&2; exit 1; }
grep -F 'loser-select|0' "$tmp_a2" >/dev/null || { echo "expected loser-select to see no reusable job" >&2; cat "$tmp_a2" >&2; exit 1; }
echo "PASS: replayed old shape ends with an empty job_id (the FOR UPDATE lock removes this window)"

# ---------------------------------------------------------------------------
# Test 3: the FOR UPDATE wait is bounded by lock_timeout (10s). A stuck holder
# must make the create fail with 55P03 rather than block forever.
# ---------------------------------------------------------------------------
cfg='config-timeout'
tmp_a3=$(mktemp)
tmp_b3=$(mktemp)
( "$psql_bin" -X -v ON_ERROR_STOP=1 -qAt -c "begin; select document_id from tender_documents where document_id = '$document_id' for update; select pg_sleep(30); commit;" >/dev/null 2>&1 ) &
pid_a=$!
sleep 1
if "$psql_bin" -X -v ON_ERROR_STOP=1 -qAt -c "select create_tender_document_job($(job_json "$cfg"))::text;" >"$tmp_b3" 2>&1; then
    echo "expected create to time out against a stuck document lock" >&2
    kill "$pid_a" 2>/dev/null || true
    wait "$pid_a" 2>/dev/null || true
    exit 1
fi
# B timed out as expected; stop the holder early so the test does not wait.
kill "$pid_a" 2>/dev/null || true
wait "$pid_a" 2>/dev/null || true
grep -E 'canceling statement due to lock timeout|55P03' "$tmp_b3" >/dev/null || {
    echo "expected lock_timeout (55P03), got:" >&2
    cat "$tmp_b3" >&2
    exit 1
}
echo "PASS: document lock wait fails fast with lock timeout (55P03)"

echo "all tender concurrency checks passed"
