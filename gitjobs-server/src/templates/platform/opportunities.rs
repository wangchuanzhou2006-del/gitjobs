use askama::Template;
use serde::{Deserialize, Serialize};

use crate::templates::{Config, PageId, auth::User};

#[derive(Debug, Clone, Template, Serialize, Deserialize)]
#[serde(bound(deserialize = "'de: 'static"))]
#[template(path = "platform/opportunity_home.html")]
pub(crate) struct OpportunityHomePage {
    pub auth_provider: Option<String>,
    pub cfg: Config,
    pub nodes: Vec<OpportunityNode>,
    pub nodes_json: String,
    pub page_id: PageId,
    pub user: User,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OpportunityNode {
    pub id: &'static str,
    pub title: &'static str,
    pub region: &'static str,
    pub city: &'static str,
    pub kind: OpportunityKind,
    pub headcount: u32,
    pub credibility: u8,
    pub days_left: u8,
    pub source_label: &'static str,
    pub prototype: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum OpportunityKind {
    University,
    Enterprise,
    Hospital,
    Park,
    Stall,
}

impl OpportunityKind {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::University => "高校",
            Self::Enterprise => "企业",
            Self::Hospital => "医院",
            Self::Park => "园区",
            Self::Stall => "档口",
        }
    }
}

pub(crate) fn prototype_nodes() -> Vec<OpportunityNode> {
    vec![
        OpportunityNode {
            id: "proto-gd-01",
            title: "广东高校食堂合作机会",
            region: "广东",
            city: "广州",
            kind: OpportunityKind::University,
            headcount: 12_000,
            credibility: 92,
            days_left: 5,
            source_label: "学校官网（原型字段）",
            prototype: true,
        },
        OpportunityNode {
            id: "proto-bj-01",
            title: "北京园区团餐托管机会",
            region: "北京",
            city: "北京",
            kind: OpportunityKind::Park,
            headcount: 6_500,
            credibility: 84,
            days_left: 12,
            source_label: "园区公告（原型字段）",
            prototype: true,
        },
        OpportunityNode {
            id: "proto-js-01",
            title: "江苏医院营养食堂合作机会",
            region: "江苏",
            city: "南京",
            kind: OpportunityKind::Hospital,
            headcount: 3_200,
            credibility: 88,
            days_left: 8,
            source_label: "医院官网（原型字段）",
            prototype: true,
        },
        OpportunityNode {
            id: "proto-zj-01",
            title: "浙江企业员工餐服务机会",
            region: "浙江",
            city: "杭州",
            kind: OpportunityKind::Enterprise,
            headcount: 8_000,
            credibility: 76,
            days_left: 20,
            source_label: "企业公告（原型字段）",
            prototype: true,
        },
        OpportunityNode {
            id: "proto-sc-01",
            title: "四川高校档口招商机会",
            region: "四川",
            city: "成都",
            kind: OpportunityKind::Stall,
            headcount: 18_000,
            credibility: 70,
            days_left: 3,
            source_label: "招商公告（原型字段）",
            prototype: true,
        },
        OpportunityNode {
            id: "proto-hb-01",
            title: "湖北高校食堂合作机会",
            region: "湖北",
            city: "武汉",
            kind: OpportunityKind::University,
            headcount: 15_000,
            credibility: 81,
            days_left: 16,
            source_label: "学校官网（原型字段）",
            prototype: true,
        },
    ]
}
