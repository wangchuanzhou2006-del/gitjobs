//! Moderator-facing talent text import.

use std::sync::Arc;

use anyhow::Result;
use askama::Template;
use axum::{
    Form,
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_messages::Messages;
use serde::Deserialize;
use tower_sessions::Session;
use tracing::instrument;

use crate::{
    auth::AuthSession,
    config::HttpServerConfig,
    db::{DynDB, talent::TalentImportBatchSummary},
    handlers::{auth::AUTH_PROVIDER_KEY, error::HandlerError},
    talent::{ImportError, NewImport, SourceType, TalentServices},
    templates::{
        Config, PageId,
        dashboard::moderator::{
            Menu,
            talent::import::{Batch, Page},
        },
    },
};

const MAX_RAW_BYTES: usize = 200_000;
const MAX_SOURCE_NAME_CHARS: usize = 120;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ImportForm {
    pub source_type: SourceType,
    pub source_name: String,
    pub raw_text: String,
}

#[instrument(skip_all, err)]
pub(crate) async fn page(
    auth_session: AuthSession,
    messages: Messages,
    session: Session,
    State(db): State<DynDB>,
    State(cfg): State<HttpServerConfig>,
) -> Result<Response, HandlerError> {
    if auth_session.user.is_none() {
        return Ok(StatusCode::FORBIDDEN.into_response());
    }
    render_page(
        auth_session,
        messages,
        session,
        db,
        cfg,
        Vec::new(),
        None,
        StatusCode::OK,
    )
    .await
}

#[instrument(skip_all, err)]
pub(crate) async fn create(
    auth_session: AuthSession,
    messages: Messages,
    session: Session,
    State(db): State<DynDB>,
    State(cfg): State<HttpServerConfig>,
    State(talent): State<Arc<TalentServices>>,
    Form(input): Form<ImportForm>,
) -> Result<Response, HandlerError> {
    let Some(user) = auth_session.user.as_ref() else {
        return Ok(StatusCode::FORBIDDEN.into_response());
    };
    let errors = validate(&input);
    if !errors.is_empty() {
        return render_page(
            auth_session,
            messages,
            session,
            db,
            cfg,
            errors,
            Some(&input),
            StatusCode::UNPROCESSABLE_ENTITY,
        )
        .await;
    }

    let receipt = match talent
        .import
        .create_batch(
            user.user_id,
            NewImport {
                source_type: input.source_type,
                source_name: input.source_name.trim().to_owned(),
                raw_text: input.raw_text,
            },
        )
        .await
    {
        Ok(receipt) => receipt,
        Err(ImportError::InvalidInput) => {
            return render_page(
                auth_session,
                messages,
                session,
                db,
                cfg,
                vec!["内容无法识别，请检查后再试。".to_owned()],
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await;
        }
        Err(ImportError::Disabled) => {
            return Ok((StatusCode::SERVICE_UNAVAILABLE, "信息整理暂未开启").into_response());
        }
        Err(ImportError::Encryption | ImportError::Persistence) => {
            return Ok((StatusCode::INTERNAL_SERVER_ERROR, "暂时无法开始整理，请稍后再试").into_response());
        }
    };

    messages.success(format!("已收到 {} 条信息，正在整理。", receipt.fragment_count));
    Ok(Redirect::to(&format!(
        "/dashboard/moderator/talent/import#batch-{}",
        receipt.batch_id
    ))
    .into_response())
}

fn validate(input: &ImportForm) -> Vec<String> {
    let mut errors = Vec::new();
    let source_name = input.source_name.trim();
    if source_name.is_empty() {
        errors.push("请填写信息来源。".to_owned());
    } else if source_name.chars().count() > MAX_SOURCE_NAME_CHARS {
        errors.push("信息来源不能超过 120 个字。".to_owned());
    }
    if input.raw_text.trim().is_empty() {
        errors.push("请粘贴需要整理的信息。".to_owned());
    } else if input.raw_text.len() > MAX_RAW_BYTES {
        errors.push("一次最多整理 20 万字节，请分批粘贴。".to_owned());
    }
    errors
}

#[allow(clippy::too_many_arguments)]
async fn render_page(
    auth_session: AuthSession,
    messages: Messages,
    session: Session,
    db: DynDB,
    cfg: HttpServerConfig,
    errors: Vec<String>,
    input: Option<&ImportForm>,
    status: StatusCode,
) -> Result<Response, HandlerError> {
    let batches = db
        .list_talent_import_batches(20)
        .await?
        .into_iter()
        .map(batch_view)
        .collect();
    let template = Page {
        cfg: Config::from(cfg),
        page_id: PageId::ModeratorDashboard,
        messages: messages.into_iter().collect(),
        user: auth_session.into(),
        auth_provider: session.get(AUTH_PROVIDER_KEY).await?,
        moderator_menu: Menu {
            talent_import: true,
            ..Menu::default()
        },
        batches,
        errors,
        source_type: input
            .map(|value| source_type_value(value.source_type))
            .unwrap_or("wechat_group")
            .to_owned(),
        source_name: input
            .map(|value| value.source_name.trim())
            .unwrap_or_default()
            .to_owned(),
    };
    Ok((status, Html(template.render()?)).into_response())
}

fn batch_view(batch: TalentImportBatchSummary) -> Batch {
    let failed = batch.status == "failed" || batch.status == "partially_failed";
    Batch {
        batch_id: batch.batch_id,
        source_name: batch.source_name,
        source_label: source_type_label(batch.source_type).to_owned(),
        status_label: status_label(&batch.status).to_owned(),
        progress_label: format!("{} / {}", batch.processed_count, batch.item_count),
        created_label: batch.created_at.format("%m-%d %H:%M").to_string(),
        failed,
    }
}

const fn source_type_value(source_type: SourceType) -> &'static str {
    match source_type {
        SourceType::WechatGroup => "wechat_group",
        SourceType::QqGroup => "qq_group",
        SourceType::Web => "web",
        SourceType::Other => "other",
    }
}

const fn source_type_label(source_type: SourceType) -> &'static str {
    match source_type {
        SourceType::WechatGroup => "微信群",
        SourceType::QqGroup => "QQ群",
        SourceType::Web => "公开网页",
        SourceType::Other => "其他",
    }
}

fn status_label(status: &str) -> &'static str {
    match status {
        "queued" => "等待整理",
        "processing" => "正在整理",
        "completed" => "整理完成",
        "partially_failed" => "部分需处理",
        "failed" => "整理失败",
        _ => "状态更新中",
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode, header::COOKIE},
    };
    use axum_login::tower_sessions::session;
    use chrono::{TimeZone, Utc};
    use tower::ServiceExt;
    use uuid::Uuid;

    use crate::{
        db::{mock::MockDB, talent::TalentImportBatchSummary},
        handlers::tests::{TestRouterBuilder, message_matches, sample_auth_user, sample_session_record},
        notifications::MockNotificationsManager,
        talent::{SourceTrust, SourceType},
    };

    use super::MAX_RAW_BYTES;

    const RAW_TEXT_SENTINEL: &str = "FICTIONAL_RAW_TEXT_SENTINEL_9D2F";
    const SAFE_SUCCESS_MESSAGE: &str = "已收到 1 条信息，正在整理。";

    fn user_session(db: &mut MockDB, is_moderator: bool) -> (session::Id, Uuid) {
        let auth_hash = "hash";
        let session_id = session::Id::default();
        let user_id = Uuid::new_v4();
        let session_record = sample_session_record(session_id, user_id, auth_hash, None);
        let mut user = sample_auth_user(user_id, auth_hash);
        user.moderator = is_moderator;
        db.expect_get_session()
            .times(1)
            .withf(move |id| *id == session_id)
            .returning(move |_| Ok(Some(session_record.clone())));
        db.expect_get_user_by_id()
            .times(1)
            .withf(move |id| *id == user_id)
            .returning(move |_| Ok(Some(user.clone())));
        (session_id, user_id)
    }

    fn moderator_session(db: &mut MockDB) -> (session::Id, Uuid) {
        user_session(db, true)
    }

    #[tokio::test]
    async fn anonymous_user_cannot_view_the_import_page() {
        let router = TestRouterBuilder::new(MockDB::new(), MockNotificationsManager::new())
            .build()
            .await;

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/dashboard/moderator/talent/import")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn non_moderator_user_cannot_view_the_import_page() {
        let mut db = MockDB::new();
        let (session_id, _) = user_session(&mut db, false);
        let router = TestRouterBuilder::new(db, MockNotificationsManager::new())
            .build()
            .await;

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/dashboard/moderator/talent/import")
                    .header(COOKIE, format!("id={session_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn anonymous_user_cannot_post_an_import() {
        let router = TestRouterBuilder::new(MockDB::new(), MockNotificationsManager::new())
            .build()
            .await;

        let response = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/dashboard/moderator/talent/import")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(format!(
                        "source_type=web&source_name=test&raw_text={RAW_TEXT_SENTINEL}"
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn non_moderator_user_cannot_post_an_import() {
        let mut db = MockDB::new();
        let (session_id, _) = user_session(&mut db, false);
        let router = TestRouterBuilder::new(db, MockNotificationsManager::new())
            .build()
            .await;

        let response = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/dashboard/moderator/talent/import")
                    .header(COOKIE, format!("id={session_id}"))
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(format!(
                        "source_type=web&source_name=test&raw_text={RAW_TEXT_SENTINEL}"
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn moderator_page_shows_accessible_controls_live_counter_and_size_limit() {
        let mut db = MockDB::new();
        let (session_id, _) = moderator_session(&mut db);
        db.expect_list_talent_import_batches().times(1).returning(|limit| {
            assert_eq!(limit, 20);
            Ok(Vec::new())
        });
        let router = TestRouterBuilder::new(db, MockNotificationsManager::new())
            .build()
            .await;

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/dashboard/moderator/talent/import")
                    .header(COOKIE, format!("id={session_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let html =
            String::from_utf8(to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();
        assert!(html.contains("整理人才信息"));
        assert!(html.contains(r#"label for="source-type""#));
        assert!(html.contains(r#"label for="source-name""#));
        assert!(html.contains(r#"label for="talent-raw-text""#));
        assert!(html.contains(r#"id="talent-character-count""#));
        assert!(html.contains(r#"aria-live="polite""#));
        assert!(html.contains(r#"aria-describedby="talent-raw-limit""#));
        assert!(html.contains("最多 200,000 字节"));
        assert!(html.contains(">开始 AI 整理</button>"));
        assert!(html.contains("/static/js/dashboard/moderator/talent-import.js"));
        assert!(html.contains("sticky bottom-0"));
    }

    #[tokio::test]
    async fn moderator_page_shows_populated_recent_batch_fields_and_review_link_without_raw_data() {
        let batch_id = Uuid::new_v4();
        let mut db = MockDB::new();
        let (session_id, _) = moderator_session(&mut db);
        db.expect_list_talent_import_batches()
            .times(1)
            .return_once(move |limit| {
                assert_eq!(limit, 20);
                Ok(vec![TalentImportBatchSummary {
                    batch_id,
                    source_id: Uuid::new_v4(),
                    source_type: SourceType::Web,
                    source_name: "测试公开招聘页".to_owned(),
                    source_trust_status: SourceTrust::New,
                    status: "processing".to_owned(),
                    item_count: 7,
                    processed_count: 3,
                    failed_count: 0,
                    model_provider: None,
                    model_name: None,
                    prompt_version: None,
                    created_at: Utc.with_ymd_and_hms(2026, 7, 26, 8, 45, 0).unwrap(),
                    completed_at: None,
                }])
            });
        let router = TestRouterBuilder::new(db, MockNotificationsManager::new())
            .build()
            .await;

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/dashboard/moderator/talent/import")
                    .header(COOKIE, format!("id={session_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let html =
            String::from_utf8(to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();
        assert!(html.contains("测试公开招聘页"));
        assert!(html.contains("公开网页"));
        assert!(html.contains("正在整理"));
        assert!(html.contains("3 / 7"));
        assert!(html.contains("07-26 08:45"));
        assert!(html.contains(&format!(
            r#"href="/dashboard/moderator/talent/review?batch={batch_id}""#
        )));
        assert!(!html.contains("raw_fragment"));
    }

    #[tokio::test]
    async fn empty_import_returns_a_clear_error_without_creating_a_batch() {
        let mut db = MockDB::new();
        let (session_id, _) = moderator_session(&mut db);
        db.expect_create_talent_import().never();
        db.expect_list_talent_import_batches()
            .times(1)
            .returning(|_| Ok(Vec::new()));
        let router = TestRouterBuilder::new(db, MockNotificationsManager::new())
            .build()
            .await;

        let response = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/dashboard/moderator/talent/import")
                    .header(COOKIE, format!("id={session_id}"))
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("source_type=web&source_name=&raw_text="))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let html =
            String::from_utf8(to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();
        assert!(html.contains("请填写信息来源"));
        assert!(html.contains("请粘贴需要整理的信息"));
    }

    #[tokio::test]
    async fn validation_error_does_not_render_or_persist_submitted_raw_text() {
        let mut db = MockDB::new();
        let (session_id, _) = moderator_session(&mut db);
        db.expect_create_talent_import().never();
        db.expect_update_session().never();
        db.expect_list_talent_import_batches()
            .times(1)
            .returning(|_| Ok(Vec::new()));
        let router = TestRouterBuilder::new(db, MockNotificationsManager::new())
            .build()
            .await;

        let response = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/dashboard/moderator/talent/import")
                    .header(COOKIE, format!("id={session_id}"))
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(format!(
                        "source_type=web&source_name=&raw_text={RAW_TEXT_SENTINEL}"
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let html =
            String::from_utf8(to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();
        assert!(html.contains("请填写信息来源"));
        assert!(!html.contains(RAW_TEXT_SENTINEL));
    }

    #[tokio::test]
    async fn oversized_import_returns_422_without_creating_a_batch() {
        let mut db = MockDB::new();
        let (session_id, _) = moderator_session(&mut db);
        db.expect_create_talent_import().never();
        db.expect_list_talent_import_batches()
            .times(1)
            .returning(|_| Ok(Vec::new()));
        let router = TestRouterBuilder::new(db, MockNotificationsManager::new())
            .build()
            .await;
        let body = format!(
            "source_type=web&source_name=test&raw_text={}",
            "a".repeat(MAX_RAW_BYTES + 1)
        );

        let response = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/dashboard/moderator/talent/import")
                    .header(COOKIE, format!("id={session_id}"))
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let html =
            String::from_utf8(to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();
        assert!(html.contains("一次最多整理 20 万字节"));
    }

    #[tokio::test]
    async fn valid_import_creates_a_normal_batch_and_redirects_to_its_status() {
        let batch_id = Uuid::new_v4();
        let mut db = MockDB::new();
        let (session_id, user_id) = moderator_session(&mut db);
        db.expect_create_talent_import()
            .times(1)
            .withf(move |input| {
                input.source_type == SourceType::Web
                    && input.source_name == "公开招聘页"
                    && input.imported_by == Some(user_id)
                    && input.items.len() == 1
            })
            .return_once(move |_| Ok(batch_id));
        db.expect_update_session()
            .times(1)
            .withf(|record| {
                let pending_message_count = record
                    .data
                    .get("axum-messages.data")
                    .and_then(|value| value.get("pending_messages"))
                    .and_then(|messages| messages.as_array())
                    .map_or(0, Vec::len);
                let persisted_session = serde_json::to_string(&record.data).unwrap();
                pending_message_count == 1
                    && message_matches(record, SAFE_SUCCESS_MESSAGE)
                    && !persisted_session.contains(RAW_TEXT_SENTINEL)
            })
            .returning(|_| Ok(()));
        let router = TestRouterBuilder::new(db, MockNotificationsManager::new())
            .build()
            .await;

        let response = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/dashboard/moderator/talent/import")
                    .header(COOKIE, format!("id={session_id}"))
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(format!(
                        "source_type=web&source_name=%E5%85%AC%E5%BC%80%E6%8B%9B%E8%81%98%E9%A1%B5&raw_text={RAW_TEXT_SENTINEL}"
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        let expected = format!("/dashboard/moderator/talent/import#batch-{batch_id}");
        assert_eq!(
            response.headers().get("location").unwrap().to_str().unwrap(),
            expected
        );
    }
}
