use anyhow::Result;
use askama::Template;
use axum::{
    extract::State,
    response::{Html, IntoResponse},
};
use chrono::Duration;
use serde::Serialize;
use tower_sessions::Session;

use crate::{
    config::HttpServerConfig,
    handlers::{auth::AUTH_PROVIDER_KEY, error::HandlerError, prepare_headers},
    templates::{
        PageId,
        auth::User,
        platform::{
            opportunities::serialize_json_for_script,
            talent::{TalentHomePage, TalentRecord, prototype_hiring_records, prototype_seeking_records},
        },
    },
};

#[derive(Serialize)]
struct TalentRecords<'a> {
    hiring: &'a [TalentRecord],
    seeking: &'a [TalentRecord],
}

pub(crate) async fn page(
    session: Session,
    State(cfg): State<HttpServerConfig>,
) -> Result<impl IntoResponse, HandlerError> {
    let hiring_records = prototype_hiring_records();
    let seeking_records = prototype_seeking_records();
    let records_json = serialize_json_for_script(&TalentRecords {
        hiring: &hiring_records,
        seeking: &seeking_records,
    })?;
    let template = TalentHomePage {
        auth_provider: session.get(AUTH_PROVIDER_KEY).await?,
        cfg: cfg.into(),
        hiring_records,
        seeking_records,
        records_json,
        page_id: PageId::Talent,
        user: User::default(),
    };
    let headers = prepare_headers(Duration::minutes(10), &[])?;
    Ok((headers, Html(template.render()?)))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    use crate::{
        db::mock::MockDB, handlers::tests::TestRouterBuilder, notifications::MockNotificationsManager,
    };

    #[tokio::test]
    async fn talent_page_renders_public_prototype_and_accessible_fallback() {
        let router = TestRouterBuilder::new(MockDB::new(), MockNotificationsManager::new())
            .build()
            .await;
        let response = router
            .oneshot(Request::builder().uri("/talent").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let html =
            String::from_utf8(to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();
        assert!(html.contains("界面原型数据，不代表真实招聘或求职信息"));
        assert!(html.contains("data-talent-mode=\"hiring\""));
        assert!(html.contains("data-talent-mode=\"seeking\""));
        assert_eq!(html.matches("<article class=\"talent-card\"").count(), 16);
        assert!(html.contains("id=\"talent-detail-drawer\""));
        assert!(html.contains("aria-live=\"polite\""));
        assert!(html.contains("<html lang=\"zh-CN\""));
        assert!(html.contains("<title>AI 餐饮人才匹配工作台</title>"));
        assert_eq!(html.matches("href=\"/talent\"").count(), 2);
        assert_eq!(html.matches("aria-current=\"page\">人才</a>").count(), 2);
        assert_eq!(html.matches("aria-current=\"page\">机会</a>").count(), 0);
    }
}
