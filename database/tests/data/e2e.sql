INSERT INTO "user" (user_id, auth_hash, created_at, email, email_verified, name, username, password, moderator)
VALUES ('f39a95c8-9903-4537-8873-2d81bfb86b35', gen_random_bytes(32), '2025-08-25 08:43:11.605766+02', 'moderator@example.invalid', true, 'E2E Moderator', 'test', '$argon2id$v=19$m=19456,t=2,p=1$bw1sZnCys+4fBpkaFqAEsQ$OnYSK/WUXUMJTsnSmJw3oPnT9skGNMtIwlqPy0EvCoA', true);

INSERT INTO employer (employer_id, company, created_at, description, public)
VALUES ('18fff2d7-c794-4130-85e4-76b9d7c60b72', 'Test Inc.', '2025-08-25 09:20:05.88454+02', 'test', false);

INSERT INTO employer (employer_id, company, created_at, description, public)
VALUES ('8e2fe443-0f7c-4fe3-95dc-c4b39b5c4d10', 'Example Labs', '2025-08-25 09:25:05.88454+02', 'second test employer', false);

INSERT INTO member (member_id, foundation, name, level, logo_url)
VALUES ('ed9e4417-3fb5-4ded-8bb2-d5f4587cc977', 'cncf', 'Test Member', 'gold', 'https://example.com/member-logo.svg');

INSERT INTO employer_member (employer_id, member_id)
VALUES ('18fff2d7-c794-4130-85e4-76b9d7c60b72', 'ed9e4417-3fb5-4ded-8bb2-d5f4587cc977');

INSERT INTO employer_team (user_id, employer_id, approved)
VALUES ('f39a95c8-9903-4537-8873-2d81bfb86b35', '18fff2d7-c794-4130-85e4-76b9d7c60b72', true);

INSERT INTO employer_team (user_id, employer_id, approved)
VALUES ('f39a95c8-9903-4537-8873-2d81bfb86b35', '8e2fe443-0f7c-4fe3-95dc-c4b39b5c4d10', true);

INSERT INTO job (employer_id, title, description, kind, seniority, workplace, status, salary, salary_max_usd_year, salary_currency, salary_period, skills, published_at) VALUES
('18fff2d7-c794-4130-85e4-76b9d7c60b72', 'Frontend Developer', 'React expert', 'full-time', 'senior', 'remote', 'published', 120000, 120000, 'USD', 'year', '{"React", "TypeScript", "JavaScript"}', CURRENT_TIMESTAMP),
('18fff2d7-c794-4130-85e4-76b9d7c60b72', 'Backend Developer', 'Node.js expert', 'full-time', 'senior', 'hybrid', 'published', 130000, 130000, 'USD', 'year', '{"Node.js", "PostgreSQL", "REST"}', CURRENT_TIMESTAMP),
('18fff2d7-c794-4130-85e4-76b9d7c60b72', 'DevOps Engineer', 'Kubernetes expert', 'full-time', 'lead', 'on-site', 'published', 150000, 150000, 'USD', 'year', '{"Kubernetes", "Docker", "AWS"}', CURRENT_TIMESTAMP),
('18fff2d7-c794-4130-85e4-76b9d7c60b72', 'Data Scientist', 'Python expert', 'part-time', 'mid', 'remote', 'published', 80000, 80000, 'USD', 'year', '{"Python", "Pandas", "scikit-learn"}', CURRENT_TIMESTAMP),
('18fff2d7-c794-4130-85e4-76b9d7c60b72', 'UI/UX Designer', 'Figma expert', 'contractor', 'junior', 'remote', 'published', 60000, 60000, 'USD', 'year', '{"Figma", "UI", "UX"}', CURRENT_TIMESTAMP),
('18fff2d7-c794-4130-85e4-76b9d7c60b72', 'Software Engineer in Test', 'Playwright expert', 'full-time', 'mid', 'hybrid', 'published', 110000, 110000, 'USD', 'year', '{"Playwright", "TypeScript", "CI/CD"}', CURRENT_TIMESTAMP),
('18fff2d7-c794-4130-85e4-76b9d7c60b72', 'Product Manager', 'Agile expert', 'full-time', 'senior', 'on-site', 'published', 140000, 140000, 'USD', 'year', '{"Agile", "Scrum", "Jira"}', CURRENT_TIMESTAMP),
('18fff2d7-c794-4130-85e4-76b9d7c60b72', 'Mobile Developer', 'React Native expert', 'internship', 'entry', 'remote', 'published', 40000, 40000, 'USD', 'year', '{"React Native", "iOS", "Android"}', CURRENT_TIMESTAMP),
('18fff2d7-c794-4130-85e4-76b9d7c60b72', 'Full-stack Developer', 'Ruby on Rails expert', 'full-time', 'mid', 'hybrid', 'published', 115000, 115000, 'USD', 'year', '{"Ruby on Rails", "PostgreSQL", "React"}', CURRENT_TIMESTAMP),
('18fff2d7-c794-4130-85e4-76b9d7c60b72', 'Security Engineer', 'Cybersecurity expert', 'full-time', 'lead', 'on-site', 'published', 160000, 160000, 'USD', 'year', '{"Cybersecurity", "Penetration Testing", "CISSP"}', CURRENT_TIMESTAMP),
('18fff2d7-c794-4130-85e4-76b9d7c60b72', 'Job 11', 'Description for Job 11', 'full-time', 'junior', 'remote', 'published', 50000, 50000, 'USD', 'year', '{"React", "JavaScript"}', CURRENT_TIMESTAMP),
('18fff2d7-c794-4130-85e4-76b9d7c60b72', 'Job 12', 'Description for Job 12', 'part-time', 'mid', 'on-site', 'published', 60000, 60000, 'USD', 'year', '{"Node.js", "REST"}', CURRENT_TIMESTAMP),
('18fff2d7-c794-4130-85e4-76b9d7c60b72', 'Job 13', 'Description for Job 13', 'contractor', 'senior', 'hybrid', 'published', 70000, 70000, 'USD', 'year', '{"Kubernetes", "Docker"}', CURRENT_TIMESTAMP),
('18fff2d7-c794-4130-85e4-76b9d7c60b72', 'Job 14', 'Description for Job 14', 'internship', 'entry', 'remote', 'published', 30000, 30000, 'USD', 'year', '{"Python", "scikit-learn"}', CURRENT_TIMESTAMP),
('18fff2d7-c794-4130-85e4-76b9d7c60b72', 'Job 15', 'Description for Job 15', 'full-time', 'lead', 'on-site', 'published', 90000, 90000, 'USD', 'year', '{"Figma", "UX"}', CURRENT_TIMESTAMP),
('18fff2d7-c794-4130-85e4-76b9d7c60b72', 'Job 16', 'Description for Job 16', 'part-time', 'junior', 'hybrid', 'published', 45000, 45000, 'USD', 'year', '{"Playwright", "CI/CD"}', CURRENT_TIMESTAMP),
('18fff2d7-c794-4130-85e4-76b9d7c60b72', 'Job 17', 'Description for Job 17', 'full-time', 'mid', 'remote', 'published', 65000, 65000, 'USD', 'year', '{"Agile", "Jira"}', CURRENT_TIMESTAMP),
('18fff2d7-c794-4130-85e4-76b9d7c60b72', 'Job 18', 'Description for Job 18', 'contractor', 'senior', 'on-site', 'published', 75000, 75000, 'USD', 'year', '{"React Native", "Android"}', CURRENT_TIMESTAMP),
('18fff2d7-c794-4130-85e4-76b9d7c60b72', 'Job 19', 'Description for Job 19', 'internship', 'entry', 'hybrid', 'published', 35000, 35000, 'USD', 'year', '{"Ruby on Rails", "React"}', CURRENT_TIMESTAMP),
('18fff2d7-c794-4130-85e4-76b9d7c60b72', 'Job 20', 'Description for Job 20', 'full-time', 'lead', 'remote', 'published', 95000, 95000, 'USD', 'year', '{"Cybersecurity", "CISSP"}', CURRENT_TIMESTAMP),
('18fff2d7-c794-4130-85e4-76b9d7c60b72', 'Job 21', 'Description for Job 21', 'full-time', 'senior', 'on-site', 'published', 100000, 100000, 'USD', 'year', '{"TypeScript", "PostgreSQL"}', CURRENT_TIMESTAMP),
('8e2fe443-0f7c-4fe3-95dc-c4b39b5c4d10', 'Platform Engineer', 'Second employer seeded job', 'full-time', 'mid', 'remote', 'published', 105000, 105000, 'USD', 'year', '{"Go", "Kubernetes", "PostgreSQL"}', CURRENT_TIMESTAMP);

-- Task 12 talent acceptance fixtures. IDs and contacts are reserved synthetic
-- values. Protected raw text and complete contacts are created only through the
-- application encryption pipeline during the browser test.
INSERT INTO talent_sources (
    source_id,
    source_type,
    name,
    trust_status,
    auto_publish_enabled,
    created_by,
    confirmed_by,
    confirmed_at,
    created_at,
    updated_at
) VALUES
(
    '00000000-0000-4000-8000-000000001201',
    'wechat_group',
    'E2E Trusted Talent Source',
    'trusted',
    true,
    'f39a95c8-9903-4537-8873-2d81bfb86b35',
    'f39a95c8-9903-4537-8873-2d81bfb86b35',
    '2026-07-28 00:00:00+00',
    '2026-07-28 00:00:00+00',
    '2026-07-28 00:00:00+00'
),
(
    '00000000-0000-4000-8000-000000001202',
    'wechat_group',
    'E2E New Talent Source',
    'new',
    false,
    'f39a95c8-9903-4537-8873-2d81bfb86b35',
    null,
    null,
    '2026-07-28 00:00:00+00',
    '2026-07-28 00:00:00+00'
);

UPDATE talent_runtime_settings
SET
    auto_publish_enabled = true,
    updated_by = 'f39a95c8-9903-4537-8873-2d81bfb86b35',
    updated_at = '2026-07-28 00:00:00+00'
WHERE singleton;

INSERT INTO talent_records (
    record_id,
    kind,
    title,
    scene,
    region,
    city,
    district,
    salary_min,
    salary_max,
    roles,
    skills,
    experience_years,
    hours_per_day,
    rest_days_per_month,
    lodging,
    housing_allowance,
    availability,
    urgency,
    source_id,
    source_label,
    published_at,
    status,
    creation_source,
    created_at,
    updated_at
) VALUES
(
    '00000000-0000-4000-8000-000000001211',
    'hiring',
    'E2E 重复切配招聘',
    'E2E 餐饮门店',
    '北京',
    '北京',
    '朝阳区',
    7000,
    9000,
    ARRAY['切配'],
    ARRAY['切配', '食品安全'],
    3,
    8,
    4,
    true,
    false,
    '随时到岗',
    '常规',
    '00000000-0000-4000-8000-000000001201',
    'E2E 已验证来源',
    '2026-07-28 00:00:00+00',
    'published',
    'manual_review',
    '2026-07-28 00:00:00+00',
    '2026-07-28 00:00:00+00'
),
(
    '00000000-0000-4000-8000-000000001212',
    'seeking',
    'E2E 切配求职',
    'E2E 餐饮门店',
    '北京',
    '北京',
    '朝阳区',
    7000,
    9000,
    ARRAY['切配'],
    ARRAY['切配', '食品安全'],
    3,
    8,
    4,
    true,
    false,
    '随时到岗',
    '急',
    '00000000-0000-4000-8000-000000001201',
    'E2E 已验证来源',
    '2026-07-28 00:00:00+00',
    'published',
    'manual_review',
    '2026-07-28 00:00:00+00',
    '2026-07-28 00:00:00+00'
);

INSERT INTO talent_contacts (
    contact_id,
    record_id,
    contact_type,
    masked_value,
    created_at
) VALUES
(
    '00000000-0000-4000-8000-000000001221',
    '00000000-0000-4000-8000-000000001211',
    'phone',
    '138****0001',
    '2026-07-28 00:00:00+00'
),
(
    '00000000-0000-4000-8000-000000001222',
    '00000000-0000-4000-8000-000000001212',
    'wechat',
    'tes***_02',
    '2026-07-28 00:00:00+00'
);

INSERT INTO talent_import_batches (
    batch_id,
    source_id,
    imported_by,
    status,
    item_count,
    processed_count,
    failed_count,
    created_at,
    completed_at,
    raw_delete_after,
    raw_deleted_at
) VALUES (
    '00000000-0000-4000-8000-000000001231',
    '00000000-0000-4000-8000-000000001201',
    'f39a95c8-9903-4537-8873-2d81bfb86b35',
    'completed',
    2,
    2,
    0,
    '2026-07-28 00:00:00+00',
    '2026-07-28 00:01:00+00',
    '2026-07-28 00:00:00+00',
    '2026-07-28 00:00:00+00'
);

INSERT INTO talent_import_items (
    item_id,
    batch_id,
    position,
    contact_masked,
    kind,
    status,
    extraction,
    extraction_confidence,
    validation_errors,
    risk_score,
    risk_level,
    risk_reasons,
    duplicate_candidates,
    lock_version,
    created_at,
    processed_at,
    updated_at
) VALUES
(
    '00000000-0000-4000-8000-000000001232',
    '00000000-0000-4000-8000-000000001231',
    0,
    '[{"contact_type":"phone","masked_value":"138****0031"}]'::jsonb,
    'hiring',
    'needs_review',
    '{
      "kind":"hiring",
      "confidence":0.85,
      "title":"E2E 种子费用待核招聘",
      "scene":"E2E 餐饮门店",
      "region":"北京",
      "city":"北京",
      "district":"朝阳区",
      "salaryMin":7000,
      "salaryMax":9000,
      "roles":["服务员"],
      "skills":["服务员","食品安全"],
      "experienceYears":2,
      "hoursPerDay":8,
      "restDaysPerMonth":4,
      "lodging":"提供",
      "housingAllowance":0,
      "availability":"随时到岗",
      "urgency":"常规",
      "publishedAt":"2026-07-28T00:00:00Z",
      "missingFields":["fee_policy_review"],
      "riskCandidates":[]
    }'::jsonb,
    0.85,
    '[]'::jsonb,
    30,
    'medium',
    '[
      {
        "code":"confidence_below_threshold",
        "explanation":"Synthetic fee details require Moderator review.",
        "evidence":null,
        "hard_block":false
      }
    ]'::jsonb,
    '[]'::jsonb,
    1,
    '2026-07-28 00:00:00+00',
    '2026-07-28 00:01:00+00',
    '2026-07-28 00:01:00+00'
),
(
    '00000000-0000-4000-8000-000000001233',
    '00000000-0000-4000-8000-000000001231',
    1,
    '[{"contact_type":"phone","masked_value":"138****0032"}]'::jsonb,
    'hiring',
    'needs_review',
    '{
      "kind":"hiring",
      "confidence":0.98,
      "title":"E2E 种子高相似招聘",
      "scene":"E2E 餐饮门店",
      "region":"北京",
      "city":"北京",
      "district":"朝阳区",
      "salaryMin":7000,
      "salaryMax":9000,
      "roles":["切配"],
      "skills":["切配","食品安全"],
      "experienceYears":3,
      "hoursPerDay":8,
      "restDaysPerMonth":4,
      "lodging":"提供",
      "housingAllowance":0,
      "availability":"随时到岗",
      "urgency":"常规",
      "publishedAt":"2026-07-28T00:00:00Z",
      "missingFields":[],
      "riskCandidates":[]
    }'::jsonb,
    0.98,
    '[]'::jsonb,
    100,
    'high',
    '[
      {
        "code":"duplicate_high",
        "explanation":"Synthetic high-similarity candidate requires Moderator review.",
        "evidence":null,
        "hard_block":true
      }
    ]'::jsonb,
    '[
      {
        "candidate_item_id":null,
        "candidate_record_id":"00000000-0000-4000-8000-000000001211",
        "score":0.90,
        "reasons":["structured_similarity"],
        "high_similarity":true
      }
    ]'::jsonb,
    1,
    '2026-07-28 00:00:00+00',
    '2026-07-28 00:01:00+00',
    '2026-07-28 00:01:00+00'
);
