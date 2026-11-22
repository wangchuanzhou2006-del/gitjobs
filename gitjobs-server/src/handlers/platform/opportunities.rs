use anyhow::Result;
use askama::Template;
use axum::{
    extract::State,
    response::{Html, IntoResponse},
};
use chrono::Duration;
use tower_sessions::Session;

use crate::{
    config::HttpServerConfig,
    handlers::{auth::AUTH_PROVIDER_KEY, error::HandlerError, prepare_headers},
    templates::{
        PageId,
        auth::User,
        platform::opportunities::{OpportunityHomePage, prototype_nodes, serialize_json_for_script},
    },
};

pub(crate) async fn page(
    session: Session,
    State(cfg): State<HttpServerConfig>,
) -> Result<impl IntoResponse, HandlerError> {
    let nodes = prototype_nodes();
    let template = OpportunityHomePage {
        auth_provider: session.get(AUTH_PROVIDER_KEY).await?,
        cfg: cfg.into(),
        nodes_json: serialize_json_for_script(&nodes)?,
        nodes,
        page_id: PageId::Opportunities,
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
    async fn opportunity_page_labels_prototype_data_and_renders_accessible_fallback() {
        let router = TestRouterBuilder::new(MockDB::new(), MockNotificationsManager::new())
            .build()
            .await;
        let response = router
            .oneshot(Request::builder().uri("/opportunities").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let html =
            String::from_utf8(to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();
        assert!(html.contains("界面原型数据，不代表真实项目"));
        assert!(html.contains("data-opportunity-id=\"proto-gd-01\""));
        assert!(html.contains("id=\"opportunity-fallback\""));
        assert!(html.contains("aria-live=\"polite\""));
        assert!(html.contains("<html lang=\"zh-CN\""));
        assert!(html.contains("<title>AI 餐饮机会星图</title>"));
        assert!(html.contains("content=\"AI 驱动的餐饮项目发现与可信情报原型。\""));
        assert!(html.contains("content=\"#071426\""));
        assert_eq!(html.matches("href=\"/talent\"").count(), 2);
        assert_eq!(html.matches("aria-current=\"page\">机会</a>").count(), 2);
        assert_eq!(html.matches("aria-current=\"page\">人才</a>").count(), 0);
    }
}
