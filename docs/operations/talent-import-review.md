# Talent import and review operations

## Safety boundary

Process only content that GitJobs is authorized to use. Raw pasted text and complete contact details are protected data: they may appear only in the authenticated Moderator review detail and in encrypted database fields. Public HTML and the `#talent-data` browser JSON may contain reviewed structured fields and masked contacts only.

Formal deployments must use `prototype_data=false`. Prototype records are local demonstration fixtures and must never be mixed into a formal `/talent` response.

## Required configuration

Set every value below through the deployment secret/configuration system. Do not commit working values.

| Environment variable | Requirement |
| --- | --- |
| `GITJOBS_SERVER__TALENT__ENABLED` | `true` only after the full preflight succeeds. |
| `GITJOBS_SERVER__TALENT__PROTOTYPE_DATA` | `false` in formal environments. |
| `GITJOBS_SERVER__TALENT__AI__BASE_URL` | Approved OpenAI-compatible HTTPS base URL. |
| `GITJOBS_SERVER__TALENT__AI__MODEL` | Provider model known to return the strict extraction schema. |
| `GITJOBS_SERVER__TALENT__AI__API_KEY` | Working provider credential from the secret manager. |
| `GITJOBS_SERVER__TALENT__ENCRYPTION__ACTIVE_VERSION` | Canonical positive integer identifying the active key. |
| `GITJOBS_SERVER__TALENT__ENCRYPTION__KEYS__<VERSION>` | One 32-byte AES key encoded as 64 lowercase hex characters for every retained version; for version 1 the name is `...__KEYS__1`. |
| `GITJOBS_SERVER__TALENT__ENCRYPTION__CONTACT_PEPPER` | Independent high-entropy HMAC pepper; never reuse an encryption key or AI credential. |

The following labels are deliberately invalid and nonworking:

```text
AI base URL: https://api.example.invalid/v1
AI model: invalid-model-name
AI key: <INVALID-PROVIDER-CREDENTIAL>
encryption key: <INVALID-64-LOWERCASE-HEX-PLACEHOLDER>
contact pepper: <INVALID-INDEPENDENT-SECRET>
```

## First enablement

1. Take and verify an encrypted database backup.
2. Apply every migration and run all pgTAP tests.
3. Inject the AI and cryptographic configuration above, keeping auto-publication globally paused.
4. With authorized fictional input, verify import, splitting, review, publication, withdrawal, and public privacy.
5. Confirm `prototype_data=false` and inspect both the `/talent` HTML and `#talent-data` JSON.
6. Trust only an operator-verified source, then enable that source and the global auto-publication switch through the controlled deployment procedure.

## Daily review and source trust

New sources always start as `new` with source auto-publication disabled. A Moderator verifies authorization, identity, complaint history, and representative samples before selecting `trusted`; that action enables the source gate. `watchlist` and `blocked` both disable source auto-publication. A trusted source still cannot auto-publish unless the global switch is enabled and the item is complete, strictly valid, low risk, and not a high-similarity duplicate.

Review the original fragment against structured fields. Edit only structured fields. Publish safe items; reject with a fixed privacy-safe reason; merge only into the displayed durable record ID; and use reprocess after a provider or transient processing problem. Never paste raw text or complete contacts into an audit reason or incident chat.

## Retry and stale-worker recovery

The processor performs exactly one retry only when the first AI request times out:

1. First timeout: retry once with the same protected fragment.
2. Second timeout or any failure from that retry: stop retrying and persist the item for human review with a safe error code.
3. A non-timeout transport, HTTP, response-shape, JSON, or schema failure is not retried; the item enters review.

An item left in `extracting` is reclaimable after the two-minute lease expires. To recover a stale worker, first confirm the old worker is stopped or isolated, wait until its `updated_at` is at least two minutes old, start one healthy worker, and verify the item advances to `needs_review`, `auto_published`, or `failed`. Do not manually rewrite the item status or decrypt data in an operator shell.

## Emergency pause

At the first sign of unexpected publication, provider behavior change, source complaint, or privacy exposure:

1. Open any Moderator review detail, expand **更多处理**, and click **暂停自动发布** once.
2. Confirm the request succeeds and reload the detail so the current global settings token is visible.
3. Submit one authorized fictional, complete, low-risk item from a trusted enabled source.
4. Verify it remains **待查看** and is absent from both `/talent` HTML and `#talent-data` JSON.

Pause does not remove pending items and does not prevent manual review. The current release intentionally has no Moderator resume button; resume only through the controlled deployment/database procedure after incident clearance and a two-person review.

## One-click withdrawal verification

For a published item, expand **更多处理** and click **撤回发布** once. Accept the single confirmation dialog; do not double-click or retry while the request is in flight.

Verify all of the following:

1. The Moderator item is no longer presented as a live published record.
2. A fresh `GET /talent` HTML response does not contain the withdrawn record ID or its unique reviewed title.
3. The `#talent-data` JSON in that same response does not contain the record ID or title.
4. A second withdrawal attempt cannot recreate or republish the record.
5. The review event records the fixed withdrawal action, actor, item/record IDs, and time without raw text or complete contacts.

If any public check still returns the record, keep auto-publication paused, invalidate affected caches, and begin incident response.

## AI provider switch

1. Validate the new base URL, model, strict JSON shape, timeouts, retention terms, and training policy in an isolated non-formal environment.
2. Pause global auto-publication.
3. Replace `...AI__BASE_URL`, `...AI__MODEL`, and `...AI__API_KEY` together; never carry a credential to a different provider.
4. Restart workers and test hiring, seeking, irrelevant, uncertain, medium/high risk, duplicate, invalid-schema, timeout, and unavailable-provider cases with fictional data.
5. Inspect public HTML, browser JSON, errors, and logs for protected values.
6. Resume only after the complete acceptance matrix and two-person approval.

## Encryption-key rotation

1. Keep every old key required to decrypt retained data.
2. Generate a new independent 32-byte key, add `...ENCRYPTION__KEYS__<NEW_VERSION>`, and deploy with both old and new keys.
3. Set `...ENCRYPTION__ACTIVE_VERSION` to the new canonical integer and restart.
4. Verify a new batch uses the new version and an old batch remains reviewable.
5. Re-encrypt retained historical protected fields with an audited job and sample both versions.
6. Remove the old key only after no retained row references it and backup/restore tests succeed.

Changing `...CONTACT_PEPPER` changes contact hashes and can break deduplication. Rotate it only with an approved full rehash plan; never silently replace it during encryption-key rotation.

## 90-day cleanup

The cleanup worker runs daily. Each week, query for due-but-not-cleaned rows and protected values that survived cleanup:

```sql
select batch_id
from talent_import_batches
where raw_delete_after <= current_timestamp
  and (
    raw_deleted_at is null
    or raw_text_ciphertext is not null
    or exists (
      select 1
      from talent_import_items item
      where item.batch_id = talent_import_batches.batch_id
        and (
          item.raw_fragment_ciphertext is not null
          or item.contact_candidates_ciphertext is not null
        )
    )
  );
```

Expected result: zero rows. Also sample cleaned batches to confirm durable structured fields and privacy-safe audit events remain while raw/ciphertext fields are null. Investigate clock, worker, or migration failures immediately; do not extend retention by copying ciphertext elsewhere.

## Backup and restore

Backups must be encrypted, access-controlled, logged, retention-limited, and stored separately from encryption keys, the contact pepper, and AI credentials. At least quarterly:

1. Restore the latest backup into an isolated environment with outbound AI and notifications disabled.
2. Apply pending migrations and compare schema versions and row counts.
3. Inject test copies of every required retained encryption key from a separate channel.
4. Verify one retained old record decrypts in Moderator detail, one new fictional import processes, one withdrawn record stays absent publicly, and one cleaned record has no protected source fields.
5. Destroy the isolated restore and its temporary secrets according to the recovery policy.

## Incident response

1. Pause global auto-publication and disable the affected source.
2. Withdraw affected records and record IDs, time range, actor, and source ID without copying protected text.
3. Preserve access/audit logs with restricted access. Search public HTML, browser JSON, caches, errors, logs, and browser console output for the exposure.
4. If credentials may be exposed, rotate the AI key and encryption keys; assess pepper rehash separately.
5. Notify the security/privacy owner, follow legal notification requirements, and document scope and containment.
6. Fix with a regression test, rerun the complete matrix with fictional data, obtain two-person approval, and restore service gradually.

## Release verification and 14-criterion evidence

Run:

```text
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- --deny warnings
just db-tests
node --test tests/unit/talent-e2e-ai-provider.test.mjs
npx playwright test --config tests/e2e/playwright.config.ts tests/e2e/dashboard/moderator/talent-import-review.spec.ts tests/e2e/public/talent-workbench.spec.ts tests/e2e/public/talent-workbench-visual.spec.ts
```

For the deterministic acceptance harness, point the formal test server at
`http://127.0.0.1:18081/v1`, set
`GITJOBS_E2E_TALENT_AI_PORT=18081` and
`GITJOBS_E2E_TALENT_ACCEPTANCE=1` in the Playwright process, and use only the
well-known disposable E2E cryptographic values from the CI workflow. Those
test-only values and the local HTTP mock are forbidden in any formal
deployment.

The Playwright command must pass in Chromium, Firefox, and WebKit with zero `console.error` and zero `pageerror` events.

| # | Acceptance criterion | Passing evidence |
| --- | --- | --- |
| 1 | Moderator pastes one complete chat block | Moderator Playwright first workflow submits `#talent-import-form` through the authenticated route. |
| 2 | The block becomes multiple classified items | The same test requires five batch-filtered rows containing hiring/seeking/risk/duplicate fixtures. |
| 3 | AI structured output is strict and invalid output is rejected | `talent::ai`, validation, and processor Rust tests plus the deterministic provider boundary unit test. |
| 4 | Risk, missing fields, and duplicates are visible | Moderator Playwright requires medium/high labels and a nonzero similarity label. |
| 5 | Only complete, trusted, low-risk items auto-publish | Moderator Playwright requires the two safe trusted rows to publish through the worker pipeline. |
| 6 | New, medium/high risk, and duplicates require review | Moderator Playwright checks each gate and the seeded high-similarity record. |
| 7 | Edit, publish, reject, merge, reprocess, withdraw | Moderator Playwright exercises every action through existing forms/routes. |
| 8 | `/talent` contains reviewed formal data only | Formal seed plus both public functional/visual specs require `prototype=false`. |
| 9 | Public output protects contacts and raw text | Moderator privacy test inspects HTML, browser JSON, safe error bodies, console, and page errors. |
| 10 | Automatic and manual decisions are audited | pgTAP review/publication tests verify review events; workflow actions exercise their database paths. |
| 11 | AI unavailable still enters review | Processor retry/unavailable Rust tests verify safe review persistence after the second timeout/failure. |
| 12 | One-click global pause | WebKit performs the irreversible pause last and proves a later trusted low-risk item stays private. |
| 13 | Protected source data is removed after 90 days | Cleanup pgTAP tests plus the due-row operator query above. |
| 14 | Duplicate clicks/concurrency do not create duplicate records | Publication pgTAP idempotency/concurrency tests plus the single-request withdrawal assertion. |

Finally run a privacy/secret scan over templates, static assets, E2E tests, seed data, configuration examples, workflow, and this runbook. Allowed hits are field names, explicitly labeled invalid placeholders, well-known disposable E2E-only cryptographic values, and reserved synthetic contacts. Any usable credential, user-provided text, complete real contact, or ciphertext value blocks release.
