//! Moderator talent review queue and private detail pages.

use std::sync::Arc;

use anyhow::Result;
use askama::Template;
use axum::{
    Form,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header::LOCATION},
    response::{Html, IntoResponse, Response},
};
use axum_messages::Messages;
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use serde_json::Value;
use tower_sessions::Session;
use tracing::instrument;
use uuid::Uuid;

use crate::{
    auth::AuthSession,
    config::HttpServerConfig,
    db::talent::{
        AuditReason, MergeTalentImportItem, PublicTalentContact, SaveReviewOutcome, TalentReviewFilter,
        TalentReviewItemSummary,
    },
    handlers::{auth::AUTH_PROVIDER_KEY, error::HandlerError},
    talent::{
        ContactType, ImportItemStatus, PrivateReviewDetail, ReviewError, ReviewFields, RiskLevel, RiskReason,
        SourceTrust, TalentKind, TalentServices,
    },
    templates::{
        Config, PageId,
        dashboard::moderator::{
            Menu,
            talent::review::{DetailItem, DetailPage, QueueItem, QueuePage, RiskReasonItem},
        },
    },
};

#[derive(Debug, Default, Deserialize)]
pub(crate) struct QueueQuery {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub risk: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub time: String,
    #[serde(default)]
    pub batch: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LockForm {
    pub lock_version: u32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SaveForm {
    pub lock_version: u32,
    pub title: String,
    pub city: String,
    pub district: String,
    pub roles: String,
    pub skills: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MergeForm {
    pub lock_version: u32,
    pub target_record_id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RejectForm {
    pub lock_version: u32,
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WithdrawForm {
    pub lock_version: u32,
    pub record_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SourceStatusForm {
    pub status: String,
    pub expected_updated_at: DateTime<Utc>,
    pub review_item_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ResourceForm {
    pub expected_updated_at: DateTime<Utc>,
    pub review_item_id: Uuid,
}

#[instrument(skip_all, err)]
pub(crate) async fn queue(
    auth_session: AuthSession,
    messages: Messages,
    session: Session,
    State(cfg): State<HttpServerConfig>,
    State(talent): State<Arc<TalentServices>>,
    Query(query): Query<QueueQuery>,
) -> Result<Response, HandlerError> {
    if auth_session.user.is_none() {
        return Ok(StatusCode::FORBIDDEN.into_response());
    }
    let filter = match queue_filter(&query, Utc::now()) {
        Ok(filter) => filter,
        Err(message) => return Ok((StatusCode::UNPROCESSABLE_ENTITY, message).into_response()),
    };
    let items = talent
        .review
        .list(filter)
        .await
        .map_err(review_handler_error)?
        .into_iter()
        .map(queue_item)
        .collect();
    let page = QueuePage {
        cfg: Config::from(cfg),
        page_id: PageId::ModeratorDashboard,
        messages: messages.into_iter().collect(),
        user: auth_session.into(),
        auth_provider: session.get(AUTH_PROVIDER_KEY).await?,
        moderator_menu: Menu {
            talent_review: true,
            ..Menu::default()
        },
        items,
        selected_status: query.status,
        selected_kind: query.kind,
        selected_risk: query.risk,
        selected_source: query.source,
        selected_time: query.time,
        selected_batch: query.batch,
    };
    Ok(Html(page.render()?).into_response())
}

#[instrument(skip_all, err)]
pub(crate) async fn detail(
    auth_session: AuthSession,
    messages: Messages,
    session: Session,
    State(cfg): State<HttpServerConfig>,
    State(talent): State<Arc<TalentServices>>,
    Path(item_id): Path<Uuid>,
) -> Result<Response, HandlerError> {
    if auth_session.user.is_none() {
        return Ok(StatusCode::FORBIDDEN.into_response());
    }
    let detail = match talent.review.detail(item_id).await {
        Ok(detail) => detail,
        Err(ReviewError::NotFound) => return Ok(StatusCode::NOT_FOUND.into_response()),
        Err(error) => return Err(review_handler_error(error)),
    };
    let page = DetailPage {
        cfg: Config::from(cfg),
        page_id: PageId::ModeratorDashboard,
        messages: messages.into_iter().collect(),
        user: auth_session.into(),
        auth_provider: session.get(AUTH_PROVIDER_KEY).await?,
        moderator_menu: Menu {
            talent_review: true,
            ..Menu::default()
        },
        item: detail_item(detail),
    };
    Ok(Html(page.render()?).into_response())
}

#[instrument(skip_all, err)]
pub(crate) async fn publish(
    auth_session: AuthSession,
    State(talent): State<Arc<TalentServices>>,
    Path(item_id): Path<Uuid>,
    headers: HeaderMap,
    Form(input): Form<LockForm>,
) -> Result<Response, HandlerError> {
    let Some(user) = auth_session.user else {
        return Ok(StatusCode::FORBIDDEN.into_response());
    };
    match talent.review.publish(item_id, input.lock_version, user.user_id).await {
        Ok(_) => Ok(action_success(&headers, RefreshDestination::Queue)),
        Err(error) => Ok(stage_error(error, "发布", item_id)),
    }
}

#[instrument(skip_all, err)]
pub(crate) async fn save(
    auth_session: AuthSession,
    State(talent): State<Arc<TalentServices>>,
    Path(item_id): Path<Uuid>,
    headers: HeaderMap,
    Form(input): Form<SaveForm>,
) -> Result<Response, HandlerError> {
    let Some(user) = auth_session.user else {
        return Ok(StatusCode::FORBIDDEN.into_response());
    };
    let outcome = match talent
        .review
        .save_fields(
            item_id,
            input.lock_version,
            user.user_id,
            ReviewFields {
                title: input.title,
                city: input.city,
                district: input.district,
                roles: input.roles,
                skills: input.skills,
            },
        )
        .await
    {
        Ok(outcome) => outcome,
        Err(error) => return Ok(stage_error(error, "保存", item_id)),
    };
    Ok(action_outcome(
        outcome,
        &headers,
        item_id,
        RefreshDestination::Detail,
    ))
}

#[instrument(skip_all, err)]
#[allow(clippy::manual_let_else)]
pub(crate) async fn merge(
    auth_session: AuthSession,
    State(talent): State<Arc<TalentServices>>,
    Path(item_id): Path<Uuid>,
    headers: HeaderMap,
    Form(input): Form<MergeForm>,
) -> Result<Response, HandlerError> {
    let Some(user) = auth_session.user else {
        return Ok(StatusCode::FORBIDDEN.into_response());
    };
    let target_record_id = match Uuid::parse_str(input.target_record_id.trim()) {
        Ok(target) => target,
        Err(_) => return Ok(validation_error("合并", "请填写有效的目标记录编号。")),
    };
    let outcome = match talent
        .review
        .merge(MergeTalentImportItem {
            item_id,
            lock_version: input.lock_version,
            target_record_id,
            actor_user_id: user.user_id,
            reason: Some(AuditReason::Duplicate),
        })
        .await
    {
        Ok(outcome) => outcome,
        Err(error) => return Ok(merge_stage_error(error, item_id)),
    };
    Ok(action_outcome(
        outcome,
        &headers,
        item_id,
        RefreshDestination::Queue,
    ))
}

#[instrument(skip_all, err)]
pub(crate) async fn withdraw(
    auth_session: AuthSession,
    State(talent): State<Arc<TalentServices>>,
    Path(item_id): Path<Uuid>,
    headers: HeaderMap,
    Form(input): Form<WithdrawForm>,
) -> Result<Response, HandlerError> {
    let Some(user) = auth_session.user else {
        return Ok(StatusCode::FORBIDDEN.into_response());
    };
    let outcome = match talent
        .review
        .withdraw(item_id, input.record_id, input.lock_version, user.user_id, None)
        .await
    {
        Ok(outcome) => outcome,
        Err(error) => return Ok(stage_error(error, "撤回", item_id)),
    };
    Ok(action_outcome(
        outcome,
        &headers,
        item_id,
        RefreshDestination::Queue,
    ))
}

#[instrument(skip_all, err)]
/// Global settings use their own `updated_at` resource token rather than an item `lock_version`.
pub(crate) async fn pause(
    auth_session: AuthSession,
    State(talent): State<Arc<TalentServices>>,
    headers: HeaderMap,
    Form(input): Form<ResourceForm>,
) -> Result<Response, HandlerError> {
    let Some(user) = auth_session.user else {
        return Ok(StatusCode::FORBIDDEN.into_response());
    };
    match talent
        .review
        .pause_auto_publish(input.expected_updated_at, user.user_id)
        .await
    {
        Ok(_) => {}
        Err(ReviewError::Conflict) => {
            return Ok(resource_conflict_response(input.review_item_id, "暂停自动发布"));
        }
        Err(_) => return Ok(global_stage_error("暂停自动发布")),
    }
    Ok(action_success(&headers, RefreshDestination::Detail))
}

#[instrument(skip_all, err)]
/// Source trust uses the source's own `updated_at` token rather than an item `lock_version`.
pub(crate) async fn source_status(
    auth_session: AuthSession,
    State(talent): State<Arc<TalentServices>>,
    Path(source_id): Path<Uuid>,
    headers: HeaderMap,
    Form(input): Form<SourceStatusForm>,
) -> Result<Response, HandlerError> {
    let Some(user) = auth_session.user else {
        return Ok(StatusCode::FORBIDDEN.into_response());
    };
    let status = match input.status.as_str() {
        "trusted" => SourceTrust::Trusted,
        "watchlist" => SourceTrust::Watchlist,
        "blocked" => SourceTrust::Blocked,
        _ => return Ok(validation_error("来源设置", "请选择有效的来源状态。")),
    };
    let updated = match talent
        .review
        .set_source_status(source_id, status, input.expected_updated_at, user.user_id)
        .await
    {
        Ok(updated) => updated,
        Err(ReviewError::Conflict) => {
            return Ok(resource_conflict_response(input.review_item_id, "来源设置"));
        }
        Err(_) => return Ok(global_stage_error("来源设置")),
    };
    if updated {
        Ok(action_success(&headers, RefreshDestination::Detail))
    } else {
        Ok(StatusCode::NOT_FOUND.into_response())
    }
}

#[instrument(skip_all, err)]
pub(crate) async fn reject(
    auth_session: AuthSession,
    State(talent): State<Arc<TalentServices>>,
    Path(item_id): Path<Uuid>,
    headers: HeaderMap,
    Form(input): Form<RejectForm>,
) -> Result<Response, HandlerError> {
    let Some(user) = auth_session.user else {
        return Ok(StatusCode::FORBIDDEN.into_response());
    };
    let reason = match input.reason.as_str() {
        "rejected" => AuditReason::Rejected,
        "duplicate" => AuditReason::Duplicate,
        "policy_violation" => AuditReason::PolicyViolation,
        "other" => AuditReason::Other,
        _ => return Ok(validation_error("驳回", "请选择不包含个人信息的驳回原因。")),
    };
    let outcome = match talent
        .review
        .reject(item_id, input.lock_version, user.user_id, Some(reason))
        .await
    {
        Ok(outcome) => outcome,
        Err(error) => return Ok(stage_error(error, "驳回", item_id)),
    };
    Ok(action_outcome(
        outcome,
        &headers,
        item_id,
        RefreshDestination::Queue,
    ))
}

#[instrument(skip_all, err)]
pub(crate) async fn reprocess(
    auth_session: AuthSession,
    State(talent): State<Arc<TalentServices>>,
    Path(item_id): Path<Uuid>,
    headers: HeaderMap,
    Form(input): Form<LockForm>,
) -> Result<Response, HandlerError> {
    let Some(user) = auth_session.user else {
        return Ok(StatusCode::FORBIDDEN.into_response());
    };
    let outcome = match talent
        .review
        .reprocess(item_id, input.lock_version, user.user_id)
        .await
    {
        Ok(outcome) => outcome,
        Err(error) => return Ok(stage_error(error, "重新整理", item_id)),
    };
    Ok(action_outcome(
        outcome,
        &headers,
        item_id,
        RefreshDestination::Queue,
    ))
}

#[allow(clippy::needless_pass_by_value)]
fn action_outcome(
    outcome: SaveReviewOutcome,
    headers: &HeaderMap,
    item_id: Uuid,
    destination: RefreshDestination,
) -> Response {
    match outcome {
        SaveReviewOutcome::Saved { .. } => action_success(headers, destination),
        SaveReviewOutcome::Conflict => conflict_response(item_id),
    }
}

#[derive(Debug, Clone, Copy)]
enum RefreshDestination {
    Detail,
    Queue,
}

fn action_success(headers: &HeaderMap, destination: RefreshDestination) -> Response {
    if is_htmx(headers) {
        let trigger = match destination {
            RefreshDestination::Detail => r#"{"talent-review-refresh":{"destination":"detail"}}"#,
            RefreshDestination::Queue => r#"{"talent-review-refresh":{"destination":"queue"}}"#,
        };
        (StatusCode::NO_CONTENT, [("HX-Trigger", trigger)]).into_response()
    } else {
        (
            StatusCode::SEE_OTHER,
            [(LOCATION, "/dashboard/moderator/talent/review")],
        )
            .into_response()
    }
}

fn queue_filter(query: &QueueQuery, now: DateTime<Utc>) -> Result<TalentReviewFilter, &'static str> {
    let statuses = match query.status.as_str() {
        "failed" => vec![ImportItemStatus::Failed],
        "published" => vec![ImportItemStatus::Published, ImportItemStatus::AutoPublished],
        "rejected" => vec![ImportItemStatus::Rejected, ImportItemStatus::Merged],
        "all" => Vec::new(),
        "" | "needs_review" => vec![ImportItemStatus::NeedsReview],
        _ => return Err("状态筛选无效，请重新选择。"),
    };
    let kinds = match query.kind.as_str() {
        "hiring" => vec![TalentKind::Hiring],
        "seeking" => vec![TalentKind::Seeking],
        "irrelevant" => vec![TalentKind::Irrelevant],
        "uncertain" => vec![TalentKind::Uncertain],
        "" | "all" => Vec::new(),
        _ => return Err("类型筛选无效，请重新选择。"),
    };
    let risk_levels = match query.risk.as_str() {
        "low" => vec![RiskLevel::Low],
        "medium" => vec![RiskLevel::Medium],
        "high" => vec![RiskLevel::High],
        "" | "all" => Vec::new(),
        _ => return Err("风险筛选无效，请重新选择。"),
    };
    let source_id = optional_uuid(&query.source, "来源筛选无效，请重新选择。")?;
    let batch_id = optional_uuid(&query.batch, "批次筛选无效，请返回批次页重试。")?;
    let updated_after = match query.time.as_str() {
        "24h" => Some(now - Duration::hours(24)),
        "7d" => Some(now - Duration::days(7)),
        "30d" => Some(now - Duration::days(30)),
        "" | "all" => None,
        _ => return Err("时间筛选无效，请重新选择。"),
    };
    Ok(TalentReviewFilter {
        statuses,
        kinds,
        risk_levels,
        source_id,
        updated_after,
        batch_id,
        limit: 100,
    })
}

fn optional_uuid(value: &str, error: &'static str) -> Result<Option<Uuid>, &'static str> {
    if value.trim().is_empty() {
        Ok(None)
    } else {
        Uuid::parse_str(value.trim()).map(Some).map_err(|_| error)
    }
}

fn queue_item(item: TalentReviewItemSummary) -> QueueItem {
    let extraction = item.extraction.as_ref();
    let title =
        text(extraction, "title").unwrap_or_else(|| first(extraction, "roles").unwrap_or("信息待补充"));
    let region_label = [
        text(extraction, "region"),
        text(extraction, "city"),
        text(extraction, "district"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ");
    let role = first(extraction, "roles").unwrap_or("岗位待补充");
    let duplicates = array_len(item.duplicate_candidates.as_ref());
    let salary_min = number_alias(extraction, "salaryMin", "salary_min");
    let salary_max = number_alias(extraction, "salaryMax", "salary_max");
    let missing = array_len_alias(extraction, "missingFields", "missing_fields")
        + array_len(item.validation_errors.as_ref());
    let contacts = masked_contact_labels(item.contact_masked.as_ref());
    QueueItem {
        item_id: item.item_id,
        source_id: item.source_id,
        kind_label: kind_label(item.kind).to_owned(),
        title: title.to_owned(),
        region_label: if region_label.is_empty() {
            "地区待补充".to_owned()
        } else {
            region_label
        },
        role_label: role.to_owned(),
        salary_label: salary_label(salary_min, salary_max),
        completeness_label: if missing == 0 {
            "字段完整".to_owned()
        } else {
            format!("待补 {missing} 项")
        },
        confidence_label: confidence_label(item.extraction_confidence),
        risk_label: risk_label(item.risk_level).to_owned(),
        risk_high: item.risk_level == Some(RiskLevel::High),
        contact_label: if contacts.is_empty() {
            if item.has_contact {
                "已识别（掩码不可用）".to_owned()
            } else {
                "未识别".to_owned()
            }
        } else {
            contacts.join("、")
        },
        duplicate_label: if duplicates == 0 {
            "无相似信息".to_owned()
        } else {
            format!("{duplicates} 条相似")
        },
        source_name: item.source_name,
        updated_label: item.updated_at.format("%m-%d %H:%M").to_string(),
        status_label: status_label(item.status).to_owned(),
    }
}

fn detail_item(detail: PrivateReviewDetail) -> DetailItem {
    let item = detail.item;
    let extraction = item.extraction.as_ref();
    let salary_min = number_alias(extraction, "salaryMin", "salary_min");
    let salary_max = number_alias(extraction, "salaryMax", "salary_max");
    let contacts = masked_contact_labels(item.contact_masked.as_ref());
    DetailItem {
        item_id: item.item_id,
        source_id: item.source_id,
        runtime_updated_at: detail
            .runtime_settings
            .updated_at
            .to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true),
        source_updated_at: item
            .source_updated_at
            .to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true),
        published_record_id: item.published_record_id,
        lock_version: item.lock_version,
        raw_fragment: detail.raw_fragment,
        kind_label: kind_label(item.kind).to_owned(),
        title: text(extraction, "title").unwrap_or_default().to_owned(),
        city: text(extraction, "city").unwrap_or_default().to_owned(),
        district: text(extraction, "district").unwrap_or_default().to_owned(),
        region: text(extraction, "region").unwrap_or_default().to_owned(),
        roles: strings(extraction, "roles"),
        skills: strings(extraction, "skills"),
        salary: salary_label(salary_min, salary_max),
        confidence_label: confidence_label(item.extraction_confidence),
        risk_label: risk_label(item.risk_level).to_owned(),
        source_name: item.source_name,
        status_label: status_label(item.status).to_owned(),
        contact_label: if contacts.is_empty() {
            "未提供可显示的掩码联系方式".to_owned()
        } else {
            contacts.join("、")
        },
        duplicate_label: format!("{} 条", array_len(item.duplicate_candidates.as_ref())),
        risk_reasons: risk_reason_items(item.risk_reasons.as_ref()),
    }
}

fn text<'a>(value: Option<&'a Value>, key: &str) -> Option<&'a str> {
    value?.get(key)?.as_str().filter(|value| !value.trim().is_empty())
}

fn first<'a>(value: Option<&'a Value>, key: &str) -> Option<&'a str> {
    value?.get(key)?.as_array()?.first()?.as_str()
}

fn strings(value: Option<&Value>, key: &str) -> String {
    value
        .and_then(|value| value.get(key))
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("、"))
        .unwrap_or_default()
}

fn number(value: Option<&Value>, key: &str) -> Option<u64> {
    value?.get(key)?.as_u64()
}

fn number_alias(value: Option<&Value>, preferred: &str, fallback: &str) -> Option<u64> {
    number(value, preferred).or_else(|| number(value, fallback))
}

fn array_len(value: Option<&Value>) -> usize {
    value.and_then(Value::as_array).map_or(0, Vec::len)
}

fn array_len_alias(value: Option<&Value>, preferred: &str, fallback: &str) -> usize {
    value
        .and_then(|value| value.get(preferred).or_else(|| value.get(fallback)))
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

fn salary_label(minimum: Option<u64>, maximum: Option<u64>) -> String {
    match (minimum, maximum) {
        (Some(minimum), Some(maximum)) => format!("{minimum} - {maximum} 元"),
        (Some(minimum), None) => format!("{minimum} 元起"),
        (None, Some(maximum)) => format!("最高 {maximum} 元"),
        _ => "工资待补充".to_owned(),
    }
}

fn masked_contact_labels(value: Option<&Value>) -> Vec<String> {
    value
        .cloned()
        .and_then(|value| serde_json::from_value::<Vec<PublicTalentContact>>(value).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|contact| {
            let contact_type = match contact.contact_type {
                ContactType::Phone => "电话",
                ContactType::Wechat => "微信",
                ContactType::Qq => "QQ",
                ContactType::Other => "其他",
            };
            format!("{contact_type} {}", contact.masked_value.as_str())
        })
        .collect()
}

fn risk_reason_items(value: Option<&Value>) -> Vec<RiskReasonItem> {
    value
        .cloned()
        .and_then(|value| serde_json::from_value::<Vec<RiskReason>>(value).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|reason| RiskReasonItem {
            explanation: safe_risk_text(&reason.explanation, "风险说明已隐藏以保护隐私"),
            evidence: reason.evidence.map(|evidence| safe_evidence(&evidence)),
            hard_block: reason.hard_block,
        })
        .collect()
}

fn safe_evidence(evidence: &str) -> String {
    safe_risk_text(evidence, "证据已隐藏以保护隐私")
}

fn safe_risk_text(value: &str, private_replacement: &str) -> String {
    let trimmed = value.trim();
    let contains_contact_like_digits = trimmed
        .split(|character: char| !character.is_ascii_digit())
        .any(|part| part.len() >= 5);
    if trimmed.chars().count() > 160
        || trimmed.contains('@')
        || trimmed.contains('\n')
        || contains_contact_like_digits
    {
        private_replacement.to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn confidence_label(confidence: Option<f32>) -> String {
    match confidence {
        Some(value) if value >= 0.9 => "把握较高".to_owned(),
        Some(value) if value >= 0.7 => "建议确认".to_owned(),
        Some(_) => "需要核对".to_owned(),
        None => "尚未判断".to_owned(),
    }
}

const fn kind_label(kind: TalentKind) -> &'static str {
    match kind {
        TalentKind::Hiring => "招聘信息",
        TalentKind::Seeking => "求职信息",
        TalentKind::Irrelevant => "无关信息",
        TalentKind::Uncertain => "类型待确认",
    }
}

const fn risk_label(risk: Option<RiskLevel>) -> &'static str {
    match risk {
        Some(RiskLevel::Low) => "风险较低",
        Some(RiskLevel::Medium) => "建议留意",
        Some(RiskLevel::High) => "需要重点核对",
        None => "尚未判断",
    }
}

const fn status_label(status: ImportItemStatus) -> &'static str {
    match status {
        ImportItemStatus::Queued => "等待整理",
        ImportItemStatus::Extracting => "正在整理",
        ImportItemStatus::NeedsReview => "待查看",
        ImportItemStatus::AutoPublished | ImportItemStatus::Published => "已发布",
        ImportItemStatus::Rejected => "已驳回",
        ImportItemStatus::Merged => "已合并",
        ImportItemStatus::Failed => "整理失败",
    }
}

fn review_handler_error(_error: ReviewError) -> HandlerError {
    HandlerError::Database("暂时无法读取审核信息，请稍后再试。".to_owned())
}

fn is_htmx(headers: &HeaderMap) -> bool {
    headers
        .get("HX-Request")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("true"))
}

fn conflict_response(item_id: Uuid) -> Response {
    (
        StatusCode::CONFLICT,
        Html(format!(
            "<p>这条信息已被其他审核操作更新。</p><a href=\"/dashboard/moderator/talent/review/{item_id}\">重新加载后继续</a>"
        )),
    )
        .into_response()
}

fn resource_conflict_response(item_id: Uuid, stage: &'static str) -> Response {
    (
        StatusCode::CONFLICT,
        Html(format!(
            "<p>{stage}使用的设置已被其他审核操作更新。</p><a href=\"/dashboard/moderator/talent/review/{item_id}\">重新加载后继续</a>"
        )),
    )
        .into_response()
}

fn validation_error(stage: &'static str, next_action: &'static str) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        format!("{stage}阶段无法继续。{next_action}"),
    )
        .into_response()
}

#[allow(clippy::needless_pass_by_value)]
fn stage_error(error: ReviewError, stage: &'static str, item_id: Uuid) -> Response {
    match error {
        ReviewError::Conflict => conflict_response(item_id),
        ReviewError::NotFound => (
            StatusCode::NOT_FOUND,
            format!("{stage}阶段找不到对应记录，请返回待查看列表。"),
        )
            .into_response(),
        ReviewError::InvalidState | ReviewError::InvalidExtraction | ReviewError::ProtectedData => (
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("{stage}阶段无法继续，请检查字段并重新加载后再试。"),
        )
            .into_response(),
        ReviewError::Persistence => (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("{stage}阶段暂时无法保存，请稍后重新加载再试。"),
        )
            .into_response(),
    }
}

#[allow(clippy::needless_pass_by_value)]
fn merge_stage_error(error: ReviewError, item_id: Uuid) -> Response {
    match error {
        ReviewError::Conflict => conflict_response(item_id),
        ReviewError::NotFound => (
            StatusCode::NOT_FOUND,
            "合并阶段找不到待处理信息，请返回待查看列表。",
        )
            .into_response(),
        ReviewError::InvalidState
        | ReviewError::InvalidExtraction
        | ReviewError::ProtectedData
        | ReviewError::Persistence => {
            validation_error("合并", "请确认目标记录仍存在且未撤回，然后重新加载后再试。")
        }
    }
}

fn global_stage_error(stage: &'static str) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        format!("{stage}阶段暂时无法保存，请稍后返回待查看列表重试。"),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use axum::{
        body::{Body, to_bytes},
        http::{HeaderMap, Method, Request, StatusCode, header::COOKIE, header::LOCATION},
    };
    use axum_login::tower_sessions::session;
    use chrono::{Duration, TimeZone, Utc};
    use serde_json::json;
    use tower::ServiceExt;
    use uuid::Uuid;

    use crate::{
        db::{
            mock::MockDB,
            talent::{
                MaskedContact, PublicTalentContact, ResourceUpdateOutcome, SaveReviewOutcome,
                TalentReviewItem, TalentReviewItemSummary, TalentRuntimeSettings, TalentSourceStatus,
            },
        },
        handlers::tests::{TestRouterBuilder, sample_auth_user, sample_session_record},
        notifications::MockNotificationsManager,
        talent::{ContactType, ImportItemStatus, RiskLevel, SourceTrust, SourceType, TalentKind},
    };

    use super::{
        QueueQuery, RefreshDestination, action_success, queue_filter, queue_item, safe_evidence,
        safe_risk_text,
    };

    fn assert_hashed_js_asset(html: &str, stem: &str) {
        let asset = html
            .split('"')
            .find(|value| value.starts_with(stem))
            .expect("hashed JavaScript asset should be rendered");
        let hash = asset
            .strip_prefix(stem)
            .and_then(|value| value.strip_suffix(".js"))
            .expect("asset should retain its JavaScript suffix");
        assert!(!hash.is_empty());
        assert!(hash.chars().all(|character| character.is_ascii_alphanumeric()));
    }

    fn user_session(db: &mut MockDB, moderator: bool) -> (session::Id, Uuid) {
        user_session_times(db, moderator, 1)
    }

    fn user_session_times(db: &mut MockDB, moderator: bool, calls: usize) -> (session::Id, Uuid) {
        let auth_hash = "review-test-hash";
        let session_id = session::Id::default();
        let user_id = Uuid::new_v4();
        let record = sample_session_record(session_id, user_id, auth_hash, None);
        let mut user = sample_auth_user(user_id, auth_hash);
        user.moderator = moderator;
        db.expect_get_session()
            .times(calls)
            .returning(move |_| Ok(Some(record.clone())));
        db.expect_get_user_by_id()
            .times(calls)
            .returning(move |_| Ok(Some(user.clone())));
        (session_id, user_id)
    }

    fn summary_fixture() -> TalentReviewItemSummary {
        TalentReviewItemSummary {
            item_id: Uuid::new_v4(),
            batch_id: Uuid::new_v4(),
            position: 0,
            kind: TalentKind::Hiring,
            status: ImportItemStatus::NeedsReview,
            extraction: Some(json!({
                "title": "Cook",
                "region": "East",
                "city": "Test City",
                "roles": ["Cook"],
                "salaryMin": 6000,
                "salaryMax": 8000,
                "missingFields": [],
                "contacts": [{"value": "fictional_complete_contact_26"}]
            })),
            extraction_confidence: Some(0.95),
            contact_masked: Some(
                serde_json::to_value(vec![PublicTalentContact {
                    contact_type: ContactType::Wechat,
                    masked_value: MaskedContact::try_from("fic***026").unwrap(),
                }])
                .unwrap(),
            ),
            has_contact: true,
            validation_errors: Some(json!([])),
            risk_score: Some(10.0),
            risk_level: Some(RiskLevel::Low),
            risk_reasons: Some(json!([])),
            duplicate_candidates: Some(json!([])),
            lock_version: 2,
            processed_at: Some(Utc::now()),
            reviewed_at: None,
            updated_at: Utc::now(),
            source_id: Uuid::new_v4(),
            source_type: SourceType::Web,
            source_name: "fictional source".to_owned(),
            source_trust_status: SourceTrust::Trusted,
        }
    }

    fn review_item_fixture(item_id: Uuid) -> TalentReviewItem {
        TalentReviewItem {
            item_id,
            batch_id: Uuid::new_v4(),
            position: 0,
            raw_fragment_ciphertext: None,
            encryption_key_version: Some(1),
            contact_candidates_ciphertext: None,
            contact_hashes: Vec::new(),
            contact_masked: Some(json!([])),
            normalized_fingerprint: None,
            kind: TalentKind::Hiring,
            status: ImportItemStatus::NeedsReview,
            extraction: Some(json!({
                "kind": "hiring",
                "confidence": 0.8,
                "title": "Cook",
                "city": "Test City",
                "roles": ["Cook"],
                "skills": [],
                "contacts": [],
                "missingFields": [],
                "riskCandidates": []
            })),
            extraction_confidence: Some(0.8),
            validation_errors: Some(json!([])),
            risk_score: Some(10.0),
            risk_level: Some(RiskLevel::Low),
            risk_reasons: Some(json!([])),
            duplicate_candidates: Some(json!([])),
            published_record_id: None,
            lock_version: 4,
            processed_at: Some(Utc::now()),
            reviewed_at: None,
            updated_at: Utc::now(),
            model_provider: Some("fictional".to_owned()),
            model_name: Some("fictional-model".to_owned()),
            prompt_version: Some("v1".to_owned()),
            source_id: Uuid::new_v4(),
            source_type: SourceType::Web,
            source_name: "fictional source".to_owned(),
            source_trust_status: SourceTrust::New,
            source_auto_publish_enabled: false,
            source_confirmed_at: None,
            source_updated_at: Utc::now(),
        }
    }

    #[test]
    fn queue_filter_threads_kind_risk_source_time_status_and_batch() {
        let source_id = Uuid::new_v4();
        let batch_id = Uuid::new_v4();
        let now = Utc.with_ymd_and_hms(2026, 7, 26, 12, 0, 0).unwrap();
        let filter = queue_filter(
            &QueueQuery {
                kind: "hiring".to_owned(),
                risk: "high".to_owned(),
                source: source_id.to_string(),
                time: "7d".to_owned(),
                status: "published".to_owned(),
                batch: batch_id.to_string(),
            },
            now,
        )
        .unwrap();

        assert_eq!(filter.kinds, vec![TalentKind::Hiring]);
        assert_eq!(filter.risk_levels, vec![RiskLevel::High]);
        assert_eq!(filter.source_id, Some(source_id));
        assert_eq!(filter.updated_after, Some(now - Duration::days(7)));
        assert_eq!(
            filter.statuses,
            vec![ImportItemStatus::Published, ImportItemStatus::AutoPublished]
        );
        assert_eq!(filter.batch_id, Some(batch_id));
        assert_eq!(filter.limit, 100);
    }

    #[test]
    fn queue_item_displays_only_validated_masked_contacts_and_safe_summary_fields() {
        let item = queue_item(summary_fixture());

        assert_eq!(item.region_label, "East · Test City");
        assert_eq!(item.role_label, "Cook");
        assert_eq!(item.salary_label, "6000 - 8000 元");
        assert_eq!(item.completeness_label, "字段完整");
        assert_eq!(item.contact_label, "微信 fic***026");
        assert!(!format!("{item:?}").contains("fictional_complete_contact_26"));
    }

    #[test]
    fn htmx_success_is_204_with_trigger_while_complete_request_is_303() {
        let mut htmx = HeaderMap::new();
        htmx.insert("HX-Request", "true".parse().unwrap());
        let response = action_success(&htmx, RefreshDestination::Detail);
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            response.headers()["HX-Trigger"],
            r#"{"talent-review-refresh":{"destination":"detail"}}"#
        );
        assert!(
            !response.headers()["HX-Trigger"]
                .to_str()
                .unwrap()
                .contains("fictional_complete_contact")
        );
        assert!(response.headers().get(LOCATION).is_none());

        let response = action_success(&HeaderMap::new(), RefreshDestination::Queue);
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response.headers()[LOCATION], "/dashboard/moderator/talent/review");
        assert!(response.headers().get("HX-Trigger").is_none());
    }

    #[test]
    fn risk_evidence_hides_contact_like_values_before_template_rendering() {
        assert_eq!(
            safe_evidence("category: fee solicitation"),
            "category: fee solicitation"
        );
        assert_eq!(safe_evidence("contact 13800138000"), "证据已隐藏以保护隐私");
        assert_eq!(safe_evidence("person@example.test"), "证据已隐藏以保护隐私");
        assert_eq!(
            safe_risk_text("call 13800138000", "风险说明已隐藏以保护隐私"),
            "风险说明已隐藏以保护隐私"
        );
    }

    #[tokio::test]
    async fn review_get_and_put_routes_reject_anonymous_users_without_private_data() {
        let raw = "FICTIONAL_PRIVATE_RAW_26";
        let complete_contact = "fictional_complete_contact_26";
        let router = TestRouterBuilder::new(MockDB::new(), MockNotificationsManager::new())
            .build()
            .await;

        for request in [
            Request::builder()
                .uri("/dashboard/moderator/talent/review")
                .body(Body::empty())
                .unwrap(),
            Request::builder()
                .method(Method::PUT)
                .uri(format!(
                    "/dashboard/moderator/talent/review/{}/save",
                    Uuid::new_v4()
                ))
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "lock_version=1&title=x&city=x&district=x&roles=x&skills=x",
                ))
                .unwrap(),
        ] {
            let expected_location = format!("/log-in?next_url={}", request.uri().path().replace('/', "%2F"));
            let response = router.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
            assert_eq!(response.headers()["location"], expected_location);
            let body = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec())
                .unwrap();
            assert!(!body.contains(raw));
            assert!(!body.contains(complete_contact));
        }
    }

    #[tokio::test]
    async fn authenticated_non_moderator_cannot_open_private_review_detail() {
        let item_id = Uuid::new_v4();
        let mut db = MockDB::new();
        let (session_id, _) = user_session(&mut db, false);
        db.expect_get_talent_review_item().never();
        let router = TestRouterBuilder::new(db, MockNotificationsManager::new())
            .build()
            .await;

        let response = router
            .oneshot(
                Request::builder()
                    .uri(format!("/dashboard/moderator/talent/review/{item_id}"))
                    .header(COOKIE, format!("id={session_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body =
            String::from_utf8(to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();
        assert!(!body.contains("FICTIONAL_PRIVATE_RAW_26"));
        assert!(!body.contains("fictional_complete_contact_26"));
    }

    #[tokio::test]
    async fn moderator_queue_threads_all_filters_and_renders_only_masked_summary_data() {
        let source_id = Uuid::new_v4();
        let batch_id = Uuid::new_v4();
        let mut summary = summary_fixture();
        summary.source_id = source_id;
        summary.batch_id = batch_id;
        let mut db = MockDB::new();
        let (session_id, _) = user_session(&mut db, true);
        db.expect_list_talent_review_items()
            .times(1)
            .withf(move |filter| {
                filter.statuses == [ImportItemStatus::NeedsReview]
                    && filter.kinds == [TalentKind::Hiring]
                    && filter.risk_levels == [RiskLevel::Low]
                    && filter.source_id == Some(source_id)
                    && filter.updated_after.is_some()
                    && filter.batch_id == Some(batch_id)
            })
            .return_once(move |_| Ok(vec![summary]));
        let router = TestRouterBuilder::new(db, MockNotificationsManager::new())
            .build()
            .await;
        let uri = format!(
            "/dashboard/moderator/talent/review?status=needs_review&kind=hiring&risk=low&source={source_id}&time=24h&batch={batch_id}"
        );

        let response = router
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .header(COOKIE, format!("id={session_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let html =
            String::from_utf8(to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();
        for expected in ["East · Test City", "6000 - 8000 元", "字段完整", "fic***026"] {
            assert!(html.contains(expected), "{expected}");
        }
        assert!(html.contains("批次编号"));
        let batch_input = html
            .split("<input")
            .skip(1)
            .filter_map(|fragment| fragment.split_once('>').map(|(tag, _)| tag))
            .find(|tag| tag.contains("name=\"batch\""))
            .expect("batch filter input should be rendered");
        assert!(batch_input.contains(&format!("value=\"{batch_id}\"")));
        assert!(!batch_input.contains("type=\"hidden\""));
        assert!(!html.contains("fictional_complete_contact_26"));
    }

    #[tokio::test]
    async fn moderator_detail_decrypts_raw_fragment_and_askama_escapes_model_html() {
        let item_id = Uuid::new_v4();
        let batch_id = Uuid::new_v4();
        let raw = "<script>FICTIONAL_PRIVATE_RAW_26</script>";
        let crypto = crate::talent::TalentCrypto::new(
            "1".to_owned(),
            HashMap::from([(
                "1".to_owned(),
                "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f".to_owned(),
            )]),
            "synthetic-contact-pepper-material-v1".to_owned(),
        )
        .unwrap();
        let protected = crypto
            .encrypt_with_version(
                raw.as_bytes(),
                &crate::talent::fragment_aad(batch_id, item_id, 0),
                "1",
            )
            .unwrap();
        let mut item = review_item_fixture(item_id);
        item.batch_id = batch_id;
        let source_token = Utc.with_ymd_and_hms(2026, 7, 27, 9, 1, 0).unwrap();
        item.source_updated_at = source_token;
        item.raw_fragment_ciphertext = Some(protected.bytes);
        item.extraction.as_mut().unwrap()["title"] = json!("<img src=x onerror=alert(1)>");
        item.risk_reasons = Some(json!([{
            "code": "fictional",
            "explanation": "<b>unsafe model html</b>",
            "evidence": "safe category",
            "hard_block": true
        }]));
        let mut db = MockDB::new();
        let (session_id, _) = user_session(&mut db, true);
        db.expect_get_talent_review_item()
            .times(1)
            .return_once(move |_| Ok(Some(item)));
        let runtime_token = Utc.with_ymd_and_hms(2026, 7, 27, 9, 0, 0).unwrap();
        db.expect_get_talent_runtime_settings().times(1).return_once(move || {
            Ok(TalentRuntimeSettings {
                auto_publish_enabled: true,
                updated_by: None,
                updated_at: runtime_token,
            })
        });
        let router = TestRouterBuilder::new(db, MockNotificationsManager::new())
            .build()
            .await;

        let response = router
            .oneshot(
                Request::builder()
                    .uri(format!("/dashboard/moderator/talent/review/{item_id}"))
                    .header(COOKIE, format!("id={session_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let html =
            String::from_utf8(to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();
        assert!(html.contains("FICTIONAL_PRIVATE_RAW_26"));
        assert!(!html.contains("<script>FICTIONAL_PRIVATE_RAW_26</script>"));
        assert!(!html.contains("<img src=x onerror=alert(1)>"));
        assert!(!html.contains("<b>unsafe model html</b>"));
        assert!(html.contains("unsafe model html"));
        assert!(html.contains("name=\"expected_updated_at\""));
        assert!(html.contains("2026-07-27T09:00:00Z"));
        assert!(html.contains("2026-07-27T09:01:00Z"));
        assert_hashed_js_asset(&html, "/static/js/dashboard/moderator/talent-review.");
    }

    #[tokio::test]
    async fn stale_save_returns_409_with_item_specific_reload_link() {
        let item_id = Uuid::new_v4();
        let mut item = review_item_fixture(item_id);
        item.lock_version = 5;
        let mut db = MockDB::new();
        let (session_id, _) = user_session(&mut db, true);
        db.expect_get_talent_review_item()
            .times(1)
            .return_once(move |_| Ok(Some(item)));
        db.expect_save_talent_review().never();
        let router = TestRouterBuilder::new(db, MockNotificationsManager::new())
            .build()
            .await;

        let response = router
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri(format!("/dashboard/moderator/talent/review/{item_id}/save"))
                    .header(COOKIE, format!("id={session_id}"))
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(
                        "lock_version=4&title=Cook&city=Test&district=&roles=Cook&skills=",
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CONFLICT);
        let html =
            String::from_utf8(to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();
        assert!(html.contains(&format!("/dashboard/moderator/talent/review/{item_id}")));
        assert!(html.contains("重新加载"));
    }

    #[tokio::test]
    async fn successful_save_uses_submitted_version_and_redirects_complete_request() {
        let item_id = Uuid::new_v4();
        let item = review_item_fixture(item_id);
        let mut db = MockDB::new();
        let (session_id, _) = user_session(&mut db, true);
        db.expect_get_talent_review_item()
            .times(1)
            .return_once(move |_| Ok(Some(item)));
        db.expect_save_talent_review()
            .times(1)
            .withf(move |review| review.item_id == item_id && review.lock_version == 4)
            .returning(|_| Ok(SaveReviewOutcome::Saved { lock_version: 5 }));
        let router = TestRouterBuilder::new(db, MockNotificationsManager::new())
            .build()
            .await;

        let response = router
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri(format!("/dashboard/moderator/talent/review/{item_id}/save"))
                    .header(COOKIE, format!("id={session_id}"))
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(
                        "lock_version=4&title=Cook&city=Test&district=&roles=Cook&skills=Rust",
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response.headers()[LOCATION], "/dashboard/moderator/talent/review");
    }

    #[tokio::test]
    async fn successful_htmx_save_triggers_detail_reload_for_the_new_version() {
        let item_id = Uuid::new_v4();
        let item = review_item_fixture(item_id);
        let mut db = MockDB::new();
        let (session_id, _) = user_session(&mut db, true);
        db.expect_get_talent_review_item()
            .times(1)
            .return_once(move |_| Ok(Some(item)));
        db.expect_save_talent_review()
            .times(1)
            .returning(|_| Ok(SaveReviewOutcome::Saved { lock_version: 5 }));
        let router = TestRouterBuilder::new(db, MockNotificationsManager::new())
            .build()
            .await;

        let response = router
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri(format!("/dashboard/moderator/talent/review/{item_id}/save"))
                    .header(COOKIE, format!("id={session_id}"))
                    .header("HX-Request", "true")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(
                        "lock_version=4&title=Cook&city=Test&district=&roles=Cook&skills=Rust",
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            response.headers()["HX-Trigger"],
            r#"{"talent-review-refresh":{"destination":"detail"}}"#
        );
    }

    #[tokio::test]
    async fn valid_reject_reason_is_a_fixed_privacy_safe_audit_code() {
        let item_id = Uuid::new_v4();
        let mut db = MockDB::new();
        let (session_id, _) = user_session(&mut db, true);
        db.expect_transition_talent_import_item()
            .times(1)
            .withf(move |transition| {
                transition.item_id == item_id
                    && transition.lock_version == 4
                    && transition.next_status == ImportItemStatus::Rejected
                    && transition.reason == Some(crate::db::talent::AuditReason::PolicyViolation)
            })
            .returning(|_| Ok(SaveReviewOutcome::Saved { lock_version: 5 }));
        let router = TestRouterBuilder::new(db, MockNotificationsManager::new())
            .build()
            .await;

        let response = router
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri(format!("/dashboard/moderator/talent/review/{item_id}/reject"))
                    .header(COOKIE, format!("id={session_id}"))
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("lock_version=4&reason=policy_violation"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
    }

    #[tokio::test]
    async fn reject_and_merge_validate_privacy_safe_inputs_before_database_writes() {
        let item_id = Uuid::new_v4();
        for (path, body, stage) in [
            ("reject", "lock_version=4&reason=private+free+text", "驳回"),
            ("merge", "lock_version=4&target_record_id=not-a-uuid", "合并"),
        ] {
            let mut db = MockDB::new();
            let (session_id, _) = user_session(&mut db, true);
            db.expect_transition_talent_import_item().never();
            db.expect_merge_talent_import_item().never();
            let router = TestRouterBuilder::new(db, MockNotificationsManager::new())
                .build()
                .await;
            let response = router
                .oneshot(
                    Request::builder()
                        .method(Method::PUT)
                        .uri(format!("/dashboard/moderator/talent/review/{item_id}/{path}"))
                        .header(COOKIE, format!("id={session_id}"))
                        .header("content-type", "application/x-www-form-urlencoded")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
            let safe = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec())
                .unwrap();
            assert!(safe.contains(stage));
            assert!(!safe.contains("private free text"));
        }
    }

    #[tokio::test]
    async fn missing_merge_target_returns_safe_target_guidance_without_database_text() {
        let item_id = Uuid::new_v4();
        let target_id = Uuid::new_v4();
        let database_text = "synthetic raw database merge target failure";
        let mut db = MockDB::new();
        let (session_id, _) = user_session(&mut db, true);
        db.expect_merge_talent_import_item()
            .times(1)
            .returning(move |_| Err(anyhow::anyhow!(database_text)));
        let router = TestRouterBuilder::new(db, MockNotificationsManager::new())
            .build()
            .await;

        let response = router
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri(format!("/dashboard/moderator/talent/review/{item_id}/merge"))
                    .header(COOKIE, format!("id={session_id}"))
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(format!("lock_version=4&target_record_id={target_id}")))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let safe =
            String::from_utf8(to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();
        assert!(safe.contains("目标记录"));
        assert!(!safe.contains(database_text));
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn reprocess_withdraw_pause_and_source_status_use_the_expected_safe_database_contracts() {
        let item_id = Uuid::new_v4();
        let record_id = Uuid::new_v4();
        let source_id = Uuid::new_v4();
        let runtime_token = Utc.with_ymd_and_hms(2026, 7, 27, 9, 0, 0).unwrap();
        let source_token = Utc.with_ymd_and_hms(2026, 7, 27, 9, 1, 0).unwrap();
        let mut db = MockDB::new();
        let (session_id, user_id) = user_session_times(&mut db, true, 4);
        db.expect_transition_talent_import_item()
            .times(1)
            .withf(move |transition| {
                transition.item_id == item_id
                    && transition.lock_version == 4
                    && transition.next_status == ImportItemStatus::Queued
            })
            .returning(|_| Ok(SaveReviewOutcome::Saved { lock_version: 5 }));
        db.expect_withdraw_talent_record()
            .times(1)
            .withf(move |item, record, version, actor, _| {
                *item == item_id && *record == record_id && *version == 4 && *actor == user_id
            })
            .returning(|_, _, _, _, _| Ok(SaveReviewOutcome::Saved { lock_version: 5 }));
        db.expect_set_talent_auto_publish()
            .times(1)
            .returning(move |enabled, actor, expected| {
                assert!(!enabled);
                assert_eq!(actor, user_id);
                assert_eq!(expected, runtime_token);
                Ok(ResourceUpdateOutcome::Updated(TalentRuntimeSettings {
                    auto_publish_enabled: false,
                    updated_by: Some(actor),
                    updated_at: Utc::now(),
                }))
            });
        db.expect_set_talent_source_status()
            .times(1)
            .withf(move |source, trust, actor, enabled, expected| {
                *source == source_id
                    && *trust == SourceTrust::Watchlist
                    && *actor == user_id
                    && !*enabled
                    && *expected == source_token
            })
            .returning(move |_, _, actor, _, _| {
                Ok(ResourceUpdateOutcome::Updated(TalentSourceStatus {
                    source_id,
                    trust_status: SourceTrust::Watchlist,
                    auto_publish_enabled: false,
                    confirmed_by: Some(actor),
                    confirmed_at: Some(Utc::now()),
                    updated_at: Utc::now(),
                }))
            });
        let router = TestRouterBuilder::new(db, MockNotificationsManager::new())
            .build()
            .await;
        let requests = [
            (
                format!("/dashboard/moderator/talent/review/{item_id}/reprocess"),
                "lock_version=4".to_owned(),
                false,
            ),
            (
                format!("/dashboard/moderator/talent/review/{item_id}/withdraw"),
                format!("lock_version=4&record_id={record_id}"),
                true,
            ),
            (
                "/dashboard/moderator/talent/auto-publish/pause".to_owned(),
                format!(
                    "expected_updated_at={}&review_item_id={item_id}",
                    runtime_token.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)
                ),
                false,
            ),
            (
                format!("/dashboard/moderator/talent/sources/{source_id}/status"),
                format!(
                    "status=watchlist&expected_updated_at={}&review_item_id={item_id}",
                    source_token.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)
                ),
                false,
            ),
        ];

        for (uri, body, htmx) in requests {
            let mut request = Request::builder()
                .method(Method::PUT)
                .uri(uri)
                .header(COOKIE, format!("id={session_id}"))
                .header("content-type", "application/x-www-form-urlencoded");
            if htmx {
                request = request.header("HX-Request", "true");
            }
            let response = router
                .clone()
                .oneshot(request.body(Body::from(body)).unwrap())
                .await
                .unwrap();
            if htmx {
                assert_eq!(response.status(), StatusCode::NO_CONTENT);
                assert_eq!(
                    response.headers()["HX-Trigger"],
                    r#"{"talent-review-refresh":{"destination":"queue"}}"#
                );
            } else {
                assert_eq!(response.status(), StatusCode::SEE_OTHER);
                assert_eq!(response.headers()[LOCATION], "/dashboard/moderator/talent/review");
            }
        }
    }

    #[tokio::test]
    async fn stale_global_and_source_tokens_return_409_with_safe_detail_reload_guidance() {
        let item_id = Uuid::new_v4();
        let source_id = Uuid::new_v4();
        let stale_token = Utc.with_ymd_and_hms(2026, 7, 27, 8, 0, 0).unwrap();
        let mut db = MockDB::new();
        let (session_id, _) = user_session_times(&mut db, true, 2);
        db.expect_set_talent_auto_publish()
            .times(1)
            .return_once(|_, _, _| Ok(ResourceUpdateOutcome::Conflict));
        db.expect_set_talent_source_status()
            .times(1)
            .return_once(|_, _, _, _, _| Ok(ResourceUpdateOutcome::Conflict));
        let router = TestRouterBuilder::new(db, MockNotificationsManager::new())
            .build()
            .await;

        for (uri, body) in [
            (
                "/dashboard/moderator/talent/auto-publish/pause".to_owned(),
                format!(
                    "expected_updated_at={}&review_item_id={item_id}",
                    stale_token.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)
                ),
            ),
            (
                format!("/dashboard/moderator/talent/sources/{source_id}/status"),
                format!(
                    "status=blocked&expected_updated_at={}&review_item_id={item_id}",
                    stale_token.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)
                ),
            ),
        ] {
            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .method(Method::PUT)
                        .uri(uri)
                        .header(COOKIE, format!("id={session_id}"))
                        .header("content-type", "application/x-www-form-urlencoded")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::CONFLICT);
            let safe = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec())
                .unwrap();
            assert!(safe.contains(&format!("/dashboard/moderator/talent/review/{item_id}")));
            assert!(safe.contains("重新加载"));
            assert!(!safe.contains("fictional_complete_contact"));
        }
    }
}
