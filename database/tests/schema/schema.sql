-- ============================================================================
-- SETUP
-- ============================================================================

begin;
select plan(244);

create function test_has_foreign_key(
    child_table text,
    child_column text,
    parent_table text,
    parent_column text,
    delete_action "char"
) returns boolean language sql stable as $$
    select exists (
        select 1
        from pg_constraint constraint_definition
        join pg_class child_relation on child_relation.oid = constraint_definition.conrelid
        join pg_class parent_relation on parent_relation.oid = constraint_definition.confrelid
        join pg_attribute child_attribute
          on child_attribute.attrelid = child_relation.oid
         and child_attribute.attnum = constraint_definition.conkey[1]
        join pg_attribute parent_attribute
          on parent_attribute.attrelid = parent_relation.oid
         and parent_attribute.attnum = constraint_definition.confkey[1]
        where constraint_definition.contype = 'f'
          and child_relation.relname = child_table
          and child_attribute.attname = child_column
          and parent_relation.relname = parent_table
          and parent_attribute.attname = parent_column
          and constraint_definition.confdeltype = delete_action
    );
$$;

-- ============================================================================
-- TESTS
-- ============================================================================

-- Test: check expected extensions exist
select has_extension('pgcrypto');
select has_extension('pg_trgm');
select has_extension('postgis');

-- Test: check expected tables exist
select has_table('application');
select has_table('certification');
select has_table('email_verification_code');
select has_table('employer');
select has_table('employer_member');
select has_table('employer_team');
select has_table('faq');
select has_table('foundation');
select has_table('image');
select has_table('image_version');
select has_table('job');
select has_table('job_certification');
select has_table('job_kind');
select has_table('job_project');
select has_table('job_seeker_profile');
select has_table('job_status');
select has_table('job_views');
select has_table('location');
select has_table('member');
select has_table('notification');
select has_table('notification_kind');
select has_table('notification_template_data');
select has_table('project');
select has_table('search_appearances');
select has_table('seniority');
select has_table('session');
select has_table('user');
select has_table('workplace');
select has_table('talent_sources');
select has_table('talent_import_batches');
select has_table('talent_import_items');
select has_table('talent_records');
select has_table('talent_contacts');
select has_table('talent_review_events');
select has_table('talent_runtime_settings');
select has_table('talent_source_subscriptions');
select has_table('talent_source_runs');
select has_table('talent_source_seen_items');

-- Test: application columns should match expected
select columns_are('application', array[
    'application_id',
    'job_seeker_profile_id',
    'job_id',
    'created_at',
    'cover_letter',
    'updated_at'
]);

-- Test: certification columns should match expected
select columns_are('certification', array[
    'certification_id',
    'name',
    'provider',
    'short_name',
    'description',
    'logo_url',
    'url'
]);

-- Test: email_verification_code columns should match expected
select columns_are('email_verification_code', array[
    'email_verification_code_id',
    'user_id',
    'created_at'
]);

-- Test: employer columns should match expected
select columns_are('employer', array[
    'employer_id',
    'location_id',
    'logo_id',
    'company',
    'created_at',
    'description',
    'public',
    'updated_at',
    'website_url'
]);

-- Test: employer_member columns should match expected
select columns_are('employer_member', array[
    'employer_id',
    'member_id'
]);

-- Test: employer_team columns should match expected
select columns_are('employer_team', array[
    'employer_id',
    'user_id',
    'approved',
    'created_at'
]);

-- Test: faq columns should match expected
select columns_are('faq', array[
    'faq_id',
    'answer',
    'question'
]);

-- Test: foundation columns should match expected
select columns_are('foundation', array[
    'foundation_id',
    'name',
    'landscape_url'
]);

-- Test: image columns should match expected
select columns_are('image', array[
    'image_id',
    'created_by'
]);

-- Test: image_version columns should match expected
select columns_are('image_version', array[
    'image_id',
    'version',
    'data'
]);

-- Test: job columns should match expected
select columns_are('job', array[
    'job_id',
    'employer_id',
    'kind',
    'seniority',
    'status',
    'location_id',
    'workplace',
    'created_at',
    'description',
    'title',
    'tsdoc',
    'apply_instructions',
    'apply_url',
    'archived_at',
    'benefits',
    'open_source',
    'published_at',
    'qualifications',
    'responsibilities',
    'salary',
    'salary_currency',
    'salary_max',
    'salary_min',
    'salary_period',
    'skills',
    'updated_at',
    'upstream_commitment',
    'tz_start',
    'tz_end',
    'salary_usd_year',
    'salary_min_usd_year',
    'salary_max_usd_year',
    'review_notes',
    'reviewed_by',
    'reviewed_at',
    'first_published_at',
    'deleted_at'
]);

-- Test: job_certification columns should match expected
select columns_are('job_certification', array[
    'job_id',
    'certification_id'
]);

-- Test: job_kind columns should match expected
select columns_are('job_kind', array[
    'job_kind_id',
    'name'
]);

-- Test: job_project columns should match expected
select columns_are('job_project', array[
    'job_id',
    'project_id'
]);

-- Test: job_seeker_profile columns should match expected
select columns_are('job_seeker_profile', array[
    'job_seeker_profile_id',
    'user_id',
    'location_id',
    'photo_id',
    'email',
    'name',
    'public',
    'summary',
    'certifications',
    'education',
    'experience',
    'facebook_url',
    'github_url',
    'linkedin_url',
    'open_to_relocation',
    'open_to_remote',
    'phone',
    'projects',
    'resume_url',
    'skills',
    'twitter_url',
    'website_url',
    'bluesky_url'
]);

-- Test: job_status columns should match expected
select columns_are('job_status', array[
    'job_status_id',
    'name'
]);

-- Test: job_views columns should match expected
select columns_are('job_views', array[
    'job_id',
    'day',
    'total'
]);

-- Test: location columns should match expected
select columns_are('location', array[
    'location_id',
    'city',
    'country',
    'tsdoc',
    'coordinates',
    'state'
]);

-- Test: member columns should match expected
select columns_are('member', array[
    'member_id',
    'foundation',
    'name',
    'level',
    'logo_url'
]);

-- Test: notification columns should match expected
select columns_are('notification', array[
    'notification_id',
    'kind',
    'user_id',
    'processed',
    'created_at',
    'error',
    'processed_at',
    'notification_template_data_id'
]);

-- Test: notification_kind columns should match expected
select columns_are('notification_kind', array[
    'notification_kind_id',
    'name'
]);

-- Test: notification_template_data columns should match expected
select columns_are('notification_template_data', array[
    'notification_template_data_id',
    'created_at',
    'data',
    'hash'
]);

-- Test: project columns should match expected
select columns_are('project', array[
    'project_id',
    'foundation',
    'name',
    'maturity',
    'logo_url'
]);

-- Test: search_appearances columns should match expected
select columns_are('search_appearances', array[
    'job_id',
    'day',
    'total'
]);

-- Test: seniority columns should match expected
select columns_are('seniority', array[
    'seniority_id',
    'name'
]);

-- Test: session columns should match expected
select columns_are('session', array[
    'session_id',
    'data',
    'expires_at'
]);

-- Test: user columns should match expected
select columns_are('user', array[
    'user_id',
    'auth_hash',
    'created_at',
    'email',
    'email_verified',
    'name',
    'username',
    'password',
    'moderator'
]);

-- Test: workplace columns should match expected
select columns_are('workplace', array[
    'workplace_id',
    'name'
]);

-- Test: talent import and review columns should match expected
select columns_are('talent_sources', array[
    'source_id', 'source_type', 'name', 'trust_status',
    'auto_publish_enabled', 'published_count', 'reported_count',
    'withdrawn_count', 'corrected_count', 'created_by', 'created_at',
    'confirmed_by', 'confirmed_at', 'updated_at'
]);

select columns_are('talent_import_batches', array[
    'batch_id', 'source_id', 'imported_by', 'raw_text_ciphertext',
    'encryption_key_version', 'status', 'model_provider', 'model_name',
    'prompt_version', 'item_count', 'processed_count', 'failed_count',
    'created_at', 'completed_at', 'raw_delete_after', 'raw_deleted_at'
]);

select columns_are('talent_import_items', array[
    'item_id', 'batch_id', 'position', 'raw_fragment_ciphertext',
    'encryption_key_version', 'contact_candidates_ciphertext',
    'contact_hashes', 'contact_masked', 'normalized_fingerprint',
    'kind', 'status', 'extraction', 'extraction_confidence',
    'validation_errors', 'risk_score', 'risk_level', 'risk_reasons',
    'duplicate_candidates', 'published_record_id', 'lock_version',
    'created_at', 'processed_at', 'reviewed_at', 'updated_at',
    'claim_token', 'claim_version', 'claim_expires_at'
]);

select columns_are('talent_records', array[
    'record_id', 'kind', 'title', 'scene', 'region', 'city', 'district',
    'salary_min', 'salary_max', 'roles', 'skills', 'experience_years',
    'hours_per_day', 'rest_days_per_month', 'lodging', 'housing_allowance',
    'availability', 'urgency', 'source_id', 'source_label', 'published_at',
    'expires_at', 'status', 'creation_source', 'created_at', 'updated_at',
    'withdrawn_at'
]);

select results_eq(
    $$
    select column_name::text collate "C", data_type::text collate "C"
    from information_schema.columns
    where table_schema = current_schema()
      and table_name = 'talent_records'
      and column_name in ('lodging', 'housing_allowance')
    order by column_name
    $$,
    $$
    values
        ('housing_allowance'::text collate "C", 'bigint'::text collate "C"),
        ('lodging'::text collate "C", 'text'::text collate "C")
    $$,
    'talent records preserve lodging text and housing allowance amounts'
);

select columns_are('talent_contacts', array[
    'contact_id', 'record_id', 'contact_type', 'ciphertext',
    'encryption_key_version', 'normalized_hash', 'masked_value',
    'created_at', 'deleted_at'
]);

select columns_are('talent_review_events', array[
    'event_id', 'item_id', 'record_id', 'actor_type', 'actor_user_id',
    'action', 'reason', 'before_snapshot', 'after_snapshot',
    'model_provider', 'model_name', 'prompt_version', 'rule_version',
    'policy_version', 'automatic_reason', 'created_at',
    'resource_scope', 'source_id', 'batch_id'
]);

select columns_are('talent_runtime_settings', array[
    'singleton', 'auto_publish_enabled', 'updated_by', 'updated_at'
]);

select columns_are('talent_source_subscriptions', array[
    'subscription_id', 'source_id', 'public_url', 'keywords',
    'interval_minutes', 'enabled', 'next_run_at', 'claimed_at',
    'incremental_cursor', 'etag', 'last_modified',
    'consecutive_failures', 'last_success_at', 'last_error_at',
    'last_error_code', 'created_by', 'created_at', 'updated_at'
]);

select columns_are('talent_source_runs', array[
    'run_id', 'subscription_id', 'discovered_count',
    'accepted_count', 'duplicate_count', 'failed_count',
    'error_code', 'started_at', 'completed_at'
]);

select columns_are('talent_source_seen_items', array[
    'subscription_id', 'source_item_id', 'content_fingerprint',
    'first_seen_at'
]);

-- Test: check expected functions exist
select has_function('get_user_by_email');
select has_function('get_user_by_id_verified');
select has_function('get_user_by_username');
select has_function('get_user_password');
select has_function('is_image_public');
select has_function('sign_up_user');
select has_function('update_user_details');
select has_function('update_user_password');
select has_function('user_has_image_access');
select has_function('user_has_profile_access');
select has_function('user_owns_employer');
select has_function('user_owns_job');
select has_function('verify_email');
select has_function('accept_team_member_invitation');
select has_function('add_employer');
select has_function('add_job');
select has_function('add_team_member');
select has_function('archive_job');
select has_function('delete_job');
select has_function('delete_team_member');
select has_function('get_applications_filters_options');
select has_function('get_employer');
select has_function('get_job_dashboard');
select has_function('get_job_salary');
select has_function('get_job_seeker_user_id');
select has_function('get_job_stats');
select has_function('get_user_invitations_count');
select has_function('list_certifications');
select has_function('list_employer_jobs');
select has_function('list_employers');
select has_function('list_foundations');
select has_function('list_team_members');
select has_function('list_user_invitations');
select has_function('publish_job');
select has_function('search_applications');
select has_function('update_employer');
select has_function('update_job');
select has_function('cancel_application');
select has_function('get_job_seeker_profile');
select has_function('list_job_seeker_applications');
select has_function('upsert_job_seeker_profile');
select has_function('approve_job');
select has_function('list_jobs_for_moderation');
select has_function('reject_job');
select has_function('i_array_to_string');
select has_function('get_image_version');
select has_function('apply_to_job');
select has_function('get_job_jobboard');
select has_function('get_jobs_filters_options');
select has_function('get_stats');
select has_function('search_jobs');
select has_function('update_jobs_views');
select has_function('update_search_appearances');
select has_function('search_locations');
select has_function('search_locations_json');
select has_function('search_members');
select has_function('search_projects');
select has_function('enqueue_notification');
select has_function('get_pending_notification');
select has_function('update_notification');
select has_function('add_member');
select has_function('add_project');
select has_function('syncer_list_foundations');
select has_function('list_members');
select has_function('list_projects');
select has_function('remove_member');
select has_function('remove_project');
select has_function('update_member');
select has_function('update_project');
select has_function('archive_expired_jobs');

-- Test: check expected primary keys
select has_pk('application');
select has_pk('certification');
select has_pk('email_verification_code');
select has_pk('employer');
select has_pk('employer_member');
select has_pk('employer_team');
select has_pk('faq');
select has_pk('foundation');
select has_pk('image');
select has_pk('image_version');
select has_pk('job');
select has_pk('job_certification');
select has_pk('job_kind');
select has_pk('job_project');
select has_pk('job_seeker_profile');
select has_pk('job_status');
select hasnt_pk('job_views');
select has_pk('location');
select has_pk('member');
select has_pk('notification');
select has_pk('notification_kind');
select has_pk('notification_template_data');
select has_pk('project');
select hasnt_pk('search_appearances');
select has_pk('seniority');
select has_pk('session');
select has_pk('user');
select has_pk('workplace');
select has_pk('talent_sources');
select has_pk('talent_import_batches');
select has_pk('talent_import_items');
select has_pk('talent_records');
select has_pk('talent_contacts');
select has_pk('talent_review_events');
select has_pk('talent_runtime_settings');

-- Test: talent import and review foreign keys should exist
select ok(test_has_foreign_key('talent_sources', 'created_by', 'user', 'user_id', 'n'), 'talent_sources.created_by should delete set null');
select ok(test_has_foreign_key('talent_sources', 'confirmed_by', 'user', 'user_id', 'n'), 'talent_sources.confirmed_by should delete set null');
select ok(test_has_foreign_key('talent_import_batches', 'source_id', 'talent_sources', 'source_id', 'r'), 'talent_import_batches.source_id should delete restrict');
select ok(test_has_foreign_key('talent_import_batches', 'imported_by', 'user', 'user_id', 'n'), 'talent_import_batches.imported_by should delete set null');
select ok(test_has_foreign_key('talent_import_items', 'batch_id', 'talent_import_batches', 'batch_id', 'r'), 'talent_import_items.batch_id should delete restrict');
select ok(test_has_foreign_key('talent_import_items', 'published_record_id', 'talent_records', 'record_id', 'n'), 'talent_import_items.published_record_id should delete set null');
select ok(test_has_foreign_key('talent_records', 'source_id', 'talent_sources', 'source_id', 'n'), 'talent_records.source_id should delete set null');
select ok(test_has_foreign_key('talent_contacts', 'record_id', 'talent_records', 'record_id', 'r'), 'talent_contacts.record_id should delete restrict');
select ok(test_has_foreign_key('talent_review_events', 'item_id', 'talent_import_items', 'item_id', 'r'), 'talent_review_events.item_id should delete restrict');
select ok(test_has_foreign_key('talent_review_events', 'record_id', 'talent_records', 'record_id', 'n'), 'talent_review_events.record_id should delete set null');
select ok(test_has_foreign_key('talent_review_events', 'actor_user_id', 'user', 'user_id', 'n'), 'talent_review_events.actor_user_id should delete set null');
select ok(test_has_foreign_key('talent_review_events', 'source_id', 'talent_sources', 'source_id', 'r'), 'talent_review_events.source_id should delete restrict');
select ok(test_has_foreign_key('talent_review_events', 'batch_id', 'talent_import_batches', 'batch_id', 'r'), 'talent_review_events.batch_id should delete restrict');
select ok(test_has_foreign_key('talent_runtime_settings', 'updated_by', 'user', 'user_id', 'n'), 'talent_runtime_settings.updated_by should delete set null');
select ok(test_has_foreign_key('talent_source_subscriptions', 'source_id', 'talent_sources', 'source_id', 'c'), 'talent_source_subscriptions.source_id should delete cascade');
select ok(test_has_foreign_key('talent_source_subscriptions', 'created_by', 'user', 'user_id', 'n'), 'talent_source_subscriptions.created_by should delete set null');
select ok(test_has_foreign_key('talent_source_runs', 'subscription_id', 'talent_source_subscriptions', 'subscription_id', 'c'), 'talent_source_runs.subscription_id should delete cascade');
select ok(test_has_foreign_key('talent_source_seen_items', 'subscription_id', 'talent_source_subscriptions', 'subscription_id', 'c'), 'talent_source_seen_items.subscription_id should delete cascade');

-- Test: singleton and uniqueness invariants should exist
select has_check('talent_runtime_settings', 'talent_runtime_settings_singleton_check');
select is(
    (select regexp_replace(pg_get_constraintdef(constraint_definition.oid, true), '\s+', ' ', 'g')
     from pg_constraint constraint_definition
     where constraint_definition.conname = 'talent_runtime_settings_singleton_check'),
    'CHECK (singleton)',
    'talent_runtime_settings_singleton_check should require singleton to be true'
);
select ok(exists (
    select 1
    from pg_index index_definition
    join pg_class index_relation on index_relation.oid = index_definition.indexrelid
    where index_relation.relname = 'talent_source_type_name_idx'
      and index_definition.indisunique
      and pg_get_indexdef(index_definition.indexrelid) like '%(source_type, lower(name))%'
), 'talent_source_type_name_idx should uniquely index source_type and lower(name)');
select ok(exists (
    select 1
    from pg_index index_definition
    join pg_class index_relation on index_relation.oid = index_definition.indexrelid
    where index_relation.relname = 'talent_import_item_batch_position_idx'
      and index_definition.indisunique
      and pg_get_indexdef(index_definition.indexrelid) like '%(batch_id, "position")%'
), 'talent_import_item_batch_position_idx should uniquely index batch_id and position');
select ok(exists (
    select 1
    from pg_index index_definition
    join pg_class index_relation on index_relation.oid = index_definition.indexrelid
    where index_relation.relname = 'talent_import_item_published_record_idx'
      and index_definition.indisunique
      and pg_get_indexdef(index_definition.indexrelid) like '%(published_record_id)%'
      and pg_get_expr(index_definition.indpred, index_definition.indrelid) = '(published_record_id IS NOT NULL)'
), 'talent_import_item_published_record_idx should uniquely index non-null published_record_id');

select has_pk('talent_source_subscriptions');
select has_pk('talent_source_runs');
select has_pk('talent_source_seen_items');

select ok(exists (
    select 1
    from pg_index index_definition
    join pg_class index_relation on index_relation.oid = index_definition.indexrelid
    where index_relation.relname = 'talent_source_subscriptions_source_id_public_url_key'
      and index_definition.indisunique
), 'talent_source_subscriptions(source_id, public_url) should be unique');

select ok(exists (
    select 1
    from pg_index index_definition
    join pg_class index_relation on index_relation.oid = index_definition.indexrelid
    where index_relation.relname = 'talent_source_subscriptions_due_idx'
      and pg_get_expr(index_definition.indpred, index_definition.indrelid) like '%enabled%'
), 'talent_source_subscriptions_due_idx should be a partial index on enabled subscriptions');

select ok(exists (
    select 1
    from pg_index index_definition
    join pg_class index_relation on index_relation.oid = index_definition.indexrelid
    where index_definition.indrelid = 'talent_source_seen_items'::regclass
      and index_definition.indisunique
      and pg_get_indexdef(index_definition.indexrelid) like '%(subscription_id, content_fingerprint)%'
), 'talent_source_seen_items(subscription_id, content_fingerprint) should be unique');

select has_check('talent_source_subscriptions', 'talent_source_subscriptions_interval_minutes_check');
select has_check('talent_source_subscriptions', 'talent_source_subscriptions_keywords_check');
select has_check('talent_source_subscriptions', 'talent_source_subscriptions_public_url_check');
select has_check('talent_source_seen_items', 'talent_source_seen_items_content_fingerprint_check');

-- Test: tender analysis tables exist
select has_table('tender_projects');
select has_table('tender_documents');
select has_table('tender_analysis_jobs');
select has_table('tender_analysis_reports');

select columns_are('tender_projects', array[
    'project_id', 'user_id', 'title', 'source_url', 'created_at', 'updated_at'
]);

select columns_are('tender_documents', array[
    'document_id', 'project_id', 'storage_key', 'sha256', 'size_bytes',
    'filename', 'uploaded_at'
]);

select columns_are('tender_analysis_jobs', array[
    'job_id', 'document_id', 'analysis_config_hash', 'status', 'claim_token',
    'claim_version', 'claim_expires_at', 'attempt_count', 'error_code',
    'model_provider', 'model_name', 'prompt_version',
    'created_at', 'updated_at', 'completed_at'
]);

select columns_are('tender_analysis_reports', array[
    'report_id', 'job_id', 'document_id', 'analysis_config_hash', 'result',
    'contract_version', 'created_at'
]);

select has_pk('tender_projects');
select has_pk('tender_documents');
select has_pk('tender_analysis_jobs');
select has_pk('tender_analysis_reports');

select ok(test_has_foreign_key('tender_projects', 'user_id', 'user', 'user_id', 'c'), 'tender_projects.user_id should delete cascade');
select ok(test_has_foreign_key('tender_documents', 'project_id', 'tender_projects', 'project_id', 'c'), 'tender_documents.project_id should delete cascade');
select ok(test_has_foreign_key('tender_analysis_jobs', 'document_id', 'tender_documents', 'document_id', 'c'), 'tender_analysis_jobs.document_id should delete cascade');
select ok(test_has_foreign_key('tender_analysis_reports', 'job_id', 'tender_analysis_jobs', 'job_id', 'c'), 'tender_analysis_reports.job_id should delete cascade');
select ok(test_has_foreign_key('tender_analysis_reports', 'document_id', 'tender_documents', 'document_id', 'c'), 'tender_analysis_reports.document_id should delete cascade');

select has_check('tender_documents', 'tender_documents_sha256_check');
select has_check('tender_documents', 'tender_documents_size_bytes_check');
select has_check('tender_documents', 'tender_documents_storage_key_check');
select has_check('tender_analysis_jobs', 'tender_analysis_jobs_status_check');
select has_check('tender_analysis_reports', 'tender_analysis_reports_contract_version_check');
select has_check('tender_analysis_reports', 'tender_analysis_reports_result_check');

select ok(exists (
    select 1
    from pg_index index_definition
    join pg_class index_relation on index_relation.oid = index_definition.indexrelid
    where index_relation.relname = 'tender_analysis_jobs_reusable_config_idx'
      and index_definition.indisunique
      and index_definition.indpred is not null
), 'tender_analysis_jobs_reusable_config_idx should be a unique partial index on non-failed jobs');

select ok(exists (
    select 1
    from pg_index index_definition
    join pg_class index_relation on index_relation.oid = index_definition.indexrelid
    where index_relation.relname = 'tender_analysis_reports_document_config_idx'
      and index_definition.indisunique
), 'tender_analysis_reports_document_config_idx should be unique');

select ok(exists (
    select 1
    from pg_index index_definition
    join pg_class index_relation on index_relation.oid = index_definition.indexrelid
    where index_relation.relname = 'tender_documents_project_sha256_idx'
      and index_definition.indisunique
), 'tender_documents_project_sha256_idx should be unique');

-- ============================================================================
-- CLEANUP
-- ============================================================================

select * from finish();
rollback;
