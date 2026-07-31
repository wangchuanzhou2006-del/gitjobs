use anyhow::Result;
use askama::Template;
use axum::{
    Form,
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_messages::Messages;
use serde::Deserialize;
use tower_sessions::Session;
use uuid::Uuid;

use crate::{
    auth::AuthSession,
    config::HttpServerConfig,
    db::{
        DynDB,
        talent::{TalentSourceSubscription, UpsertTalentSourceSubscription},
    },
    handlers::{auth::AUTH_PROVIDER_KEY, error::HandlerError},
    talent::validate_source_subscription,
    templates::{
        Config, PageId,
        dashboard::moderator::{
            Menu,
            talent::radar::{RadarEditPage, RadarPage, RadarSource},
        },
    },
};

#[derive(Debug, Deserialize)]
pub(crate) struct RadarForm {
    #[serde(default)]
    pub subscription_id: Option<Uuid>,
    pub source_name: String,
    pub public_url: String,
    pub keywords: String,
    pub frequency: String,
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ToggleForm {
    pub enabled: bool,
}

pub(crate) async fn page(
    auth_session: AuthSession,
    messages: Messages,
    session: Session,
    State(cfg): State<HttpServerConfig>,
    State(db): State<DynDB>,
) -> Result<Response, HandlerError> {
    if auth_session.user.is_none() {
        return Ok(StatusCode::FORBIDDEN.into_response());
    }
    render(auth_session, messages, session, cfg, db, Vec::new()).await
}

pub(crate) async fn create(
    auth_session: AuthSession,
    messages: Messages,
    session: Session,
    State(cfg): State<HttpServerConfig>,
    State(db): State<DynDB>,
    Form(input): Form<RadarForm>,
) -> Result<Response, HandlerError> {
    let Some(user) = auth_session.user.as_ref() else {
        return Ok(StatusCode::FORBIDDEN.into_response());
    };
    let actor_user_id = user.user_id;
    let interval_minutes = match input.frequency.as_str() {
        "twice_daily" => 720,
        "weekly" => 10_080,
        _ => 1_440,
    };
    if input.source_name.trim().is_empty() {
        return render(
            auth_session,
            messages,
            session,
            cfg,
            db,
            vec!["请填写来源名称。".to_owned()],
        )
        .await;
    }
    let keywords = input
        .keywords
        .split(['，', ',', '、'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let validated = match validate_source_subscription(
        &input.public_url,
        &keywords,
        interval_minutes,
        &cfg.talent.allowed_public_hosts,
    ) {
        Ok(value) => value,
        Err(_) => {
            return render(
                auth_session,
                messages,
                session,
                cfg,
                db,
                vec!["这个公开来源暂不支持，请确认地址已获允许。".to_owned()],
            )
            .await;
        }
    };
    db.upsert_talent_source_subscription(UpsertTalentSourceSubscription {
        subscription_id: input.subscription_id,
        source_name: input.source_name.trim().to_owned(),
        public_url: validated.public_url.to_string(),
        keywords: validated.keywords,
        interval_minutes: validated.interval_minutes,
        enabled: input.enabled,
        actor_user_id,
    })
    .await
    .map_err(|_| HandlerError::Database("暂时无法保存公开来源。".to_owned()))?;
    Ok(Redirect::to("/dashboard/moderator/talent/radar").into_response())
}

pub(crate) async fn edit_page(
    auth_session: AuthSession,
    messages: Messages,
    session: Session,
    State(cfg): State<HttpServerConfig>,
    State(db): State<DynDB>,
    Path(subscription_id): Path<Uuid>,
) -> Result<Response, HandlerError> {
    if auth_session.user.is_none() {
        return Ok(StatusCode::FORBIDDEN.into_response());
    }
    let source = db
        .list_talent_source_subscriptions()
        .await
        .map_err(|_| HandlerError::Database("暂时无法读取公开来源。".to_owned()))?
        .into_iter()
        .find(|source| source.subscription_id == subscription_id);
    let Some(source) = source else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let page = RadarEditPage {
        cfg: Config::from(cfg),
        page_id: PageId::ModeratorDashboard,
        messages: messages.into_iter().collect(),
        user: auth_session.into(),
        auth_provider: session.get(AUTH_PROVIDER_KEY).await?,
        moderator_menu: Menu {
            talent_radar: true,
            ..Menu::default()
        },
        subscription_id,
        source_name: source.source_name,
        public_url: source.public_url,
        keywords: source.keywords.join("、"),
        frequency: match source.interval_minutes {
            720 => "twice_daily",
            10_080 => "weekly",
            _ => "daily",
        }
        .to_owned(),
        enabled: source.enabled,
    };
    Ok(Html(page.render()?).into_response())
}

pub(crate) async fn toggle(
    auth_session: AuthSession,
    State(db): State<DynDB>,
    Path(subscription_id): Path<Uuid>,
    Form(input): Form<ToggleForm>,
) -> Result<Response, HandlerError> {
    let Some(user) = auth_session.user else {
        return Ok(StatusCode::FORBIDDEN.into_response());
    };
    if db
        .set_talent_source_subscription_enabled(subscription_id, input.enabled, user.user_id)
        .await
        .map_err(|_| HandlerError::Database("暂时无法更新这个来源。".to_owned()))?
    {
        Ok(Redirect::to("/dashboard/moderator/talent/radar").into_response())
    } else {
        Ok(StatusCode::NOT_FOUND.into_response())
    }
}

async fn render(
    auth_session: AuthSession,
    messages: Messages,
    session: Session,
    cfg: HttpServerConfig,
    db: DynDB,
    errors: Vec<String>,
) -> Result<Response, HandlerError> {
    let subscriptions = db
        .list_talent_source_subscriptions()
        .await
        .map_err(|_| HandlerError::Database("暂时无法读取公开来源。".to_owned()))?;
    let attention_count = subscriptions
        .iter()
        .filter(|source| source.consecutive_failures > 0)
        .count();
    let page = RadarPage {
        cfg: Config::from(cfg.clone()),
        page_id: PageId::ModeratorDashboard,
        messages: messages.into_iter().collect(),
        user: auth_session.into(),
        auth_provider: session.get(AUTH_PROVIDER_KEY).await?,
        moderator_menu: Menu {
            talent_radar: true,
            ..Menu::default()
        },
        enabled: cfg.talent.source_updates_enabled,
        watched_count: subscriptions.iter().filter(|source| source.enabled).count(),
        attention_count,
        sources: subscriptions.into_iter().map(source_view).collect(),
        errors,
    };
    Ok(Html(page.render()?).into_response())
}

fn source_view(source: TalentSourceSubscription) -> RadarSource {
    let needs_attention = source.consecutive_failures > 0;
    RadarSource {
        subscription_id: source.subscription_id,
        name: source.source_name,
        topics: source.keywords.join("、"),
        frequency: match source.interval_minutes {
            720 => "每天两次",
            10_080 => "每周一次",
            _ => "每天一次",
        }
        .to_owned(),
        updated: source.last_success_at.map_or_else(
            || "尚未更新".to_owned(),
            |time| time.format("%m-%d %H:%M").to_string(),
        ),
        status: if !source.enabled {
            "已暂停"
        } else if needs_attention {
            "需要留意"
        } else {
            "正常关注"
        }
        .to_owned(),
        needs_attention,
        enabled: source.enabled,
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode, header::COOKIE},
    };
    use axum_login::tower_sessions::session;
    use chrono::Utc;
    use tower::ServiceExt;
    use uuid::Uuid;

    use crate::{
        db::{mock::MockDB, talent::TalentSourceSubscription},
        handlers::tests::{TestRouterBuilder, sample_auth_user, sample_session_record, test_http_server_cfg},
        notifications::MockNotificationsManager,
    };

    fn moderator_session(db: &mut MockDB) -> session::Id {
        let auth_hash = "hash";
        let session_id = session::Id::default();
        let user_id = Uuid::new_v4();
        let record = sample_session_record(session_id, user_id, auth_hash, None);
        let mut moderator = sample_auth_user(user_id, auth_hash);
        moderator.moderator = true;
        db.expect_get_session()
            .times(1)
            .returning(move |_| Ok(Some(record.clone())));
        db.expect_get_user_by_id()
            .times(1)
            .returning(move |_| Ok(Some(moderator.clone())));
        session_id
    }

    fn subscription() -> TalentSourceSubscription {
        TalentSourceSubscription {
            subscription_id: Uuid::new_v4(),
            source_id: Uuid::new_v4(),
            source_name: "虚构公开来源".to_owned(),
            public_url: "https://feeds.example.test/jobs.xml".to_owned(),
            keywords: vec!["厨师".to_owned()],
            interval_minutes: 1_440,
            enabled: true,
            next_run_at: Utc::now(),
            incremental_cursor: None,
            etag: None,
            last_modified: None,
            consecutive_failures: 0,
            last_success_at: None,
            last_error_at: None,
            last_error_code: None,
            created_by: Some(Uuid::new_v4()),
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn radar_page_uses_plain_mobile_language_without_internal_state() {
        let mut db = MockDB::new();
        let session_id = moderator_session(&mut db);
        db.expect_list_talent_source_subscriptions()
            .times(1)
            .returning(|| Ok(vec![subscription()]));
        let router = TestRouterBuilder::new(db, MockNotificationsManager::new())
            .build()
            .await;
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/dashboard/moderator/talent/radar")
                    .header(COOKIE, format!("id={session_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let html =
            String::from_utf8(to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();
        for text in ["信息雷达", "从哪里找", "找什么", "多久找一次", "编辑", "暂停关注"]
        {
            assert!(html.contains(text), "{text}");
        }
        for hidden in [
            "incremental_cursor",
            "etag",
            "last_modified",
            "consecutive_failures",
            "ciphertext",
        ] {
            assert!(!html.contains(hidden), "{hidden}");
        }
    }

    #[tokio::test]
    async fn unsafe_source_is_rejected_before_persistence() {
        let mut db = MockDB::new();
        let session_id = moderator_session(&mut db);
        db.expect_upsert_talent_source_subscription().never();
        db.expect_list_talent_source_subscriptions()
            .times(1)
            .returning(|| Ok(Vec::new()));
        let mut cfg = test_http_server_cfg();
        cfg.talent.allowed_public_hosts = vec!["feeds.example.test".to_owned()];
        let router = TestRouterBuilder::new(db, MockNotificationsManager::new())
            .with_cfg(cfg)
            .build()
            .await;
        let response = router.oneshot(Request::builder().method("POST").uri("/dashboard/moderator/talent/radar").header(COOKIE, format!("id={session_id}")).header("content-type", "application/x-www-form-urlencoded").body(Body::from("source_name=test&public_url=http%3A%2F%2Ffeeds.example.test%2Ffeed&keywords=cook&frequency=daily&enabled=true")).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let html =
            String::from_utf8(to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();
        assert!(html.contains("这个公开来源暂不支持"));
    }
}
