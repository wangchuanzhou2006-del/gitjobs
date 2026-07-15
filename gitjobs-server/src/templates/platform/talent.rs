use askama::Template;
use serde::{Deserialize, Serialize};

use crate::templates::{Config, PageId, auth::User};

#[derive(Debug, Clone, Template)]
#[template(path = "platform/talent_home.html")]
pub(crate) struct TalentHomePage {
    // Required for platform page-context parity; consumed by later authenticated actions.
    #[allow(dead_code)]
    pub auth_provider: Option<String>,
    pub cfg: Config,
    pub hiring_records: Vec<TalentRecord>,
    pub seeking_records: Vec<TalentRecord>,
    pub records_json: String,
    pub page_id: PageId,
    // Required for platform page-context parity; consumed by later authenticated actions.
    #[allow(dead_code)]
    pub user: User,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum TalentMode {
    Hiring,
    Seeking,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TalentRecord {
    pub id: &'static str,
    pub mode: TalentMode,
    pub title: &'static str,
    pub scene: &'static str,
    pub region: &'static str,
    pub district: &'static str,
    pub salary_min: u32,
    pub salary_max: u32,
    pub skills: Vec<&'static str>,
    pub experience_years: u8,
    pub hours_per_day: u8,
    pub rest_days: u8,
    pub lodging: &'static str,
    pub housing_allowance: u32,
    pub availability: &'static str,
    pub urgency: &'static str,
    pub credibility: u8,
    pub source_label: &'static str,
    pub updated_label: &'static str,
    pub risk_flags: Vec<&'static str>,
    pub prototype: bool,
}

pub(crate) fn prototype_hiring_records() -> Vec<TalentRecord> {
    let fixtures = [
        (
            "proto-hire-01",
            "炒锅岗位（原型）",
            "连锁餐饮门店（原型）",
            "北京",
            "朝阳区",
            "炒锅",
        ),
        (
            "proto-hire-02",
            "切配岗位（原型）",
            "社区餐饮门店（原型）",
            "天津",
            "和平区",
            "切配",
        ),
        (
            "proto-hire-03",
            "打荷岗位（原型）",
            "园区餐厅（原型）",
            "河北",
            "石家庄长安区",
            "打荷",
        ),
        (
            "proto-hire-04",
            "洗碗岗位（原型）",
            "连锁餐饮门店（原型）",
            "广东",
            "广州天河区",
            "洗碗",
        ),
        (
            "proto-hire-05",
            "面点岗位（原型）",
            "社区餐饮门店（原型）",
            "江苏",
            "南京鼓楼区",
            "面点",
        ),
        (
            "proto-hire-06",
            "烧烤岗位（原型）",
            "园区餐厅（原型）",
            "浙江",
            "杭州拱墅区",
            "烧烤",
        ),
        (
            "proto-hire-07",
            "服务员岗位（原型）",
            "连锁餐饮门店（原型）",
            "四川",
            "成都武侯区",
            "服务员",
        ),
        (
            "proto-hire-08",
            "店长岗位（原型）",
            "社区餐饮门店（原型）",
            "安徽",
            "合肥蜀山区",
            "店长",
        ),
    ];

    fixtures
        .into_iter()
        .enumerate()
        .map(
            |(index, (id, title, scene, region, district, skill))| TalentRecord {
                id,
                mode: TalentMode::Hiring,
                title,
                scene,
                region,
                district,
                salary_min: 4_800 + index as u32 * 400,
                salary_max: 6_800 + index as u32 * 500,
                skills: vec![skill, "餐饮门店协作（原型）"],
                experience_years: (index % 5 + 1) as u8,
                hours_per_day: 8,
                rest_days: 4,
                lodging: if index % 2 == 0 {
                    "提供（原型）"
                } else {
                    "不提供（原型）"
                },
                housing_allowance: if index % 2 == 0 { 0 } else { 500 },
                availability: "一周内到岗（原型）",
                urgency: if index < 2 {
                    "较高（原型）"
                } else {
                    "常规（原型）"
                },
                credibility: 72 + index as u8 * 3,
                source_label: "平台演示数据",
                updated_label: "近期更新（原型）",
                risk_flags: vec![if index % 2 == 0 {
                    "薪资构成未拆分（原型）"
                } else {
                    "岗位细节待确认（原型）"
                }],
                prototype: true,
            },
        )
        .collect()
}

pub(crate) fn prototype_seeking_records() -> Vec<TalentRecord> {
    let fixtures = [
        (
            "proto-seek-01",
            "候选人 A（原型）",
            "餐饮从业者档案（原型）",
            "北京",
            "朝阳区",
            "炒锅",
        ),
        (
            "proto-seek-02",
            "候选人 B（原型）",
            "餐饮从业者档案（原型）",
            "天津",
            "和平区",
            "切配",
        ),
        (
            "proto-seek-03",
            "候选人 C（原型）",
            "餐饮从业者档案（原型）",
            "河北",
            "石家庄长安区",
            "打荷",
        ),
        (
            "proto-seek-04",
            "候选人 D（原型）",
            "餐饮从业者档案（原型）",
            "广东",
            "广州天河区",
            "洗碗",
        ),
        (
            "proto-seek-05",
            "候选人 E（原型）",
            "餐饮从业者档案（原型）",
            "江苏",
            "南京鼓楼区",
            "面点",
        ),
        (
            "proto-seek-06",
            "候选人 F（原型）",
            "餐饮从业者档案（原型）",
            "浙江",
            "杭州拱墅区",
            "烧烤",
        ),
        (
            "proto-seek-07",
            "候选人 G（原型）",
            "餐饮从业者档案（原型）",
            "四川",
            "成都武侯区",
            "服务员",
        ),
        (
            "proto-seek-08",
            "候选人 H（原型）",
            "餐饮从业者档案（原型）",
            "安徽",
            "合肥蜀山区",
            "店长",
        ),
    ];

    fixtures
        .into_iter()
        .enumerate()
        .map(
            |(index, (id, title, scene, region, district, skill))| TalentRecord {
                id,
                mode: TalentMode::Seeking,
                title,
                scene,
                region,
                district,
                salary_min: 4_500 + index as u32 * 450,
                salary_max: 6_500 + index as u32 * 550,
                skills: vec![skill, "餐饮门店协作（原型）"],
                experience_years: (index % 6 + 1) as u8,
                hours_per_day: 8,
                rest_days: 4,
                lodging: if index % 2 == 0 {
                    "需要（原型）"
                } else {
                    "可协商（原型）"
                },
                housing_allowance: if index % 2 == 0 { 600 } else { 0 },
                availability: if index < 3 {
                    "随时到岗（原型）"
                } else {
                    "两周内到岗（原型）"
                },
                urgency: if index < 2 {
                    "较高（原型）"
                } else {
                    "常规（原型）"
                },
                credibility: 70 + index as u8 * 3,
                source_label: "平台演示数据",
                updated_label: "近期更新（原型）",
                risk_flags: vec![if index % 2 == 0 {
                    "技能证明未核验（原型）"
                } else {
                    "工作经历待确认（原型）"
                }],
                prototype: true,
            },
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use regex::Regex;
    use serde::Serialize;

    use super::{prototype_hiring_records, prototype_seeking_records};
    use crate::templates::platform::opportunities::serialize_json_for_script;

    #[test]
    fn prototype_fixture_lists_have_eight_unique_ids_each() {
        for records in [prototype_hiring_records(), prototype_seeking_records()] {
            assert_eq!(records.len(), 8);
            let ids = records.iter().map(|record| record.id).collect::<HashSet<_>>();
            assert_eq!(ids.len(), 8);
        }
    }

    #[test]
    fn prototype_fixture_records_are_labeled_and_contain_no_contact_numbers() {
        let long_digit_run = Regex::new(r"\d{7,}").unwrap();

        for record in prototype_hiring_records()
            .into_iter()
            .chain(prototype_seeking_records())
        {
            assert!(record.prototype);
            let json = serde_json::to_string(&record).unwrap();
            assert!(!long_digit_run.is_match(&json), "contact-like digits in {json}");
        }
    }

    #[derive(Serialize)]
    struct Payload<'a> {
        title: &'a str,
    }

    #[test]
    fn talent_script_json_escapes_closing_script_tag() {
        let json = serialize_json_for_script(&Payload { title: "</script>" }).unwrap();

        assert!(json.contains(r"\u003c/script>"));
        assert!(!json.contains("</script>"));
    }
}
