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
    db::{DynDB, talent::PublicTalentRecord},
    handlers::{auth::AUTH_PROVIDER_KEY, error::HandlerError, prepare_headers},
    templates::{
        PageId,
        auth::User,
        platform::{
            opportunities::serialize_json_for_script,
            talent::{
                TalentHomePage, TalentMode, TalentRecord, prototype_hiring_records, prototype_seeking_records,
            },
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
    State(db): State<DynDB>,
) -> Result<impl IntoResponse, HandlerError> {
    let prototype_mode = cfg.talent.prototype_data;
    let (hiring_records, seeking_records) = if prototype_mode {
        (prototype_hiring_records(), prototype_seeking_records())
    } else {
        let published = db
            .list_published_talent_records(None, 200)
            .await
            .map_err(|_| HandlerError::Database("暂时无法读取已发布的人才信息。".to_owned()))?;
        let mut hiring = Vec::new();
        let mut seeking = Vec::new();
        for record in published {
            match record.kind {
                crate::talent::TalentKind::Hiring => hiring.push(public_record(record, TalentMode::Hiring)),
                crate::talent::TalentKind::Seeking => {
                    seeking.push(public_record(record, TalentMode::Seeking))
                }
                crate::talent::TalentKind::Irrelevant | crate::talent::TalentKind::Uncertain => {}
            }
        }
        (hiring, seeking)
    };
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
        prototype_mode,
        page_id: PageId::Talent,
        user: User::default(),
    };
    let headers = prepare_headers(Duration::minutes(10), &[])?;
    Ok((headers, Html(template.render()?)))
}

fn public_record(record: PublicTalentRecord, mode: TalentMode) -> TalentRecord {
    let mut location_terms = Vec::new();
    for location in [&record.region, &record.city, &record.district].into_iter().flatten() {
        if !location_terms.contains(location) {
            location_terms.push(location.clone());
        }
    }
    let region = [&record.region, &record.city]
        .into_iter()
        .flatten()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(" ");
    let mut skills = record.roles;
    for skill in record.skills {
        if !skills.contains(&skill) {
            skills.push(skill);
        }
    }
    let mut risk_flags = Vec::new();
    for (missing, label) in [
        (
            record.salary_min.is_none() || record.salary_max.is_none(),
            "薪资范围待确认",
        ),
        (record.experience_years.is_none(), "经验年限待确认"),
        (record.hours_per_day.is_none(), "每日工时待确认"),
        (record.rest_days_per_month.is_none(), "每月休息天数待确认"),
        (record.lodging.is_none(), "住宿安排待确认"),
        (record.housing_allowance.is_none(), "住房补贴待确认"),
        (record.availability.is_none(), "到岗时间待确认"),
        (record.urgency.is_none(), "紧急度待确认"),
    ] {
        if missing {
            risk_flags.push(label.to_owned());
        }
    }
    TalentRecord {
        id: record.record_id.to_string(),
        mode,
        title: record.title,
        scene: record.scene,
        region,
        district: record.district.unwrap_or_default(),
        location_terms,
        salary_min: record.salary_min,
        salary_max: record.salary_max,
        skills,
        experience_years: record.experience_years,
        hours_per_day: record.hours_per_day,
        rest_days: record.rest_days_per_month,
        lodging: record
            .lodging
            .map(|value| if value { "提供" } else { "不提供" }.to_owned()),
        housing_allowance: record.housing_allowance,
        availability: record.availability,
        urgency: record.urgency,
        credibility: None,
        source_label: record.source_label,
        updated_label: record.published_at.format("%Y-%m-%d").to_string(),
        risk_flags,
        contact_masked: record
            .contact_masked
            .first()
            .map(|contact| contact.masked_value.as_str().to_owned()),
        prototype: false,
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use chrono::Utc;
    use tower::ServiceExt;
    use uuid::Uuid;

    use crate::{
        db::{
            mock::MockDB,
            talent::{MaskedContact, PublicTalentContact, PublicTalentRecord},
        },
        handlers::tests::{TestRouterBuilder, test_http_server_cfg},
        notifications::MockNotificationsManager,
        talent::{ContactType, TalentKind},
    };

    fn published_record() -> PublicTalentRecord {
        PublicTalentRecord {
            record_id: Uuid::new_v4(),
            kind: TalentKind::Hiring,
            title: "测试厨师岗位".to_owned(),
            scene: Some("餐饮门店".to_owned()),
            region: Some("河北".to_owned()),
            city: Some("石家庄".to_owned()),
            district: Some("长安区".to_owned()),
            salary_min: None,
            salary_max: None,
            roles: vec!["厨师".to_owned(), "炒锅".to_owned()],
            skills: vec!["炒锅".to_owned(), "刀工".to_owned()],
            experience_years: None,
            hours_per_day: None,
            rest_days_per_month: None,
            lodging: None,
            housing_allowance: Some(true),
            availability: None,
            urgency: None,
            source_label: None,
            published_at: Utc::now(),
            expires_at: None,
            contact_masked: vec![PublicTalentContact {
                contact_type: ContactType::Phone,
                masked_value: MaskedContact::try_from("138****8000").unwrap(),
            }],
        }
    }

    #[tokio::test]
    async fn talent_page_preserves_safe_published_meaning_without_inventing_facts() {
        let mut db = MockDB::new();
        db.expect_list_published_talent_records()
            .with(mockall::predicate::eq(None), mockall::predicate::eq(200))
            .times(1)
            .returning(|_, _| Ok(vec![published_record()]));
        let router = TestRouterBuilder::new(db, MockNotificationsManager::new())
            .build()
            .await;
        let response = router
            .oneshot(Request::builder().uri("/talent").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let html =
            String::from_utf8(to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();
        assert!(!html.contains("界面原型数据，不代表真实招聘或求职信息"));
        assert!(html.contains("测试厨师岗位"));
        assert!(html.contains("河北"));
        assert!(html.contains("石家庄"));
        assert!(html.contains("长安区"));
        assert!(html.contains("厨师"));
        assert_eq!(
            html.matches("炒锅").count(),
            2,
            "HTML fallback and browser JSON should each retain it once"
        );
        assert!(html.contains("刀工"));
        assert!(html.contains("138****8000"));
        assert!(html.contains(r#""salaryMin":null"#));
        assert!(html.contains(r#""experienceYears":null"#));
        assert!(html.contains(r#""hoursPerDay":null"#));
        assert!(html.contains(r#""restDays":null"#));
        assert!(html.contains(r#""lodging":null"#));
        assert!(html.contains(r#""housingAllowance":true"#));
        assert!(html.contains(r#""credibility":null"#));
        assert!(html.contains("薪资范围待确认"));
        assert!(!html.contains("0–0 元/月"));
        assert!(!html.contains("0 年"));
        assert!(!html.contains("0 小时/天"));
        assert!(!html.contains("0 天/月"));
        assert!(!html.contains("0 / 100"));
        assert!(!html.contains("13812348000"));
        for private_name in [
            "raw_text",
            "raw_fragment",
            "contact_candidates",
            "ciphertext",
            "nonce",
            "key_version",
            "hash",
            "fingerprint",
            "import_batch_id",
            "import_item_id",
            "source_item_id",
        ] {
            assert!(
                !html.contains(private_name),
                "private field name leaked: {private_name}"
            );
        }
        assert!(html.contains("data-talent-mode=\"hiring\""));
        assert!(html.contains("data-talent-mode=\"seeking\""));
        assert_eq!(html.matches("<article class=\"talent-card\"").count(), 1);
        assert!(html.contains("id=\"talent-detail-drawer\""));
        assert!(html.contains("aria-live=\"polite\""));
        assert!(html.contains("<html lang=\"zh-CN\""));
        assert!(html.contains("<title>AI 餐饮人才匹配工作台</title>"));
        assert_eq!(html.matches("href=\"/talent\"").count(), 2);
        assert_eq!(html.matches("aria-current=\"page\">人才</a>").count(), 2);
        assert_eq!(html.matches("aria-current=\"page\">机会</a>").count(), 0);
    }

    #[tokio::test]
    async fn talent_page_has_a_clear_empty_state_without_prototypes() {
        let mut db = MockDB::new();
        db.expect_list_published_talent_records().returning(|_, _| Ok(vec![]));
        let router = TestRouterBuilder::new(db, MockNotificationsManager::new())
            .build()
            .await;
        let response = router
            .oneshot(Request::builder().uri("/talent").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let html =
            String::from_utf8(to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();
        assert!(html.contains("暂时没有可展示的信息"));
        assert_eq!(html.matches("<article class=\"talent-card\"").count(), 0);
    }

    #[tokio::test]
    async fn talent_prototypes_require_explicit_fixture_mode() {
        let mut db = MockDB::new();
        db.expect_list_published_talent_records().times(0);
        let mut cfg = test_http_server_cfg();
        cfg.talent.prototype_data = true;
        let router = TestRouterBuilder::new(db, MockNotificationsManager::new())
            .with_cfg(cfg)
            .build()
            .await;
        let response = router
            .oneshot(Request::builder().uri("/talent").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let html =
            String::from_utf8(to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();
        assert!(html.contains("界面原型数据，不代表真实招聘或求职信息"));
        assert_eq!(html.matches("<article class=\"talent-card\"").count(), 16);
    }
}
