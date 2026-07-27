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
    pub prototype_mode: bool,
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
    pub id: String,
    pub mode: TalentMode,
    pub title: String,
    pub scene: Option<String>,
    pub region: String,
    pub district: String,
    pub location_terms: Vec<String>,
    pub salary_min: Option<u32>,
    pub salary_max: Option<u32>,
    pub skills: Vec<String>,
    pub experience_years: Option<u8>,
    pub hours_per_day: Option<u8>,
    pub rest_days: Option<u8>,
    pub lodging: Option<String>,
    pub housing_allowance: Option<bool>,
    pub availability: Option<String>,
    pub urgency: Option<String>,
    pub credibility: Option<u8>,
    pub source_label: Option<String>,
    pub updated_label: String,
    pub risk_flags: Vec<String>,
    pub contact_masked: Option<String>,
    pub prototype: bool,
}

impl TalentRecord {
    pub(crate) fn scene_label(&self) -> &str {
        self.scene.as_deref().unwrap_or("待确认")
    }

    pub(crate) fn salary_label(&self) -> String {
        match (self.salary_min, self.salary_max) {
            (Some(min), Some(max)) => format!("{min}–{max} 元/月"),
            (Some(min), None) => format!("{min} 元/月起"),
            (None, Some(max)) => format!("最高 {max} 元/月"),
            (None, None) => "待确认".to_owned(),
        }
    }

    pub(crate) fn lodging_label(&self) -> &str {
        self.lodging.as_deref().unwrap_or("待确认")
    }

    pub(crate) fn experience_label(&self) -> String {
        self.experience_years
            .map_or_else(|| "待确认".to_owned(), |years| format!("{years} 年"))
    }

    pub(crate) fn hours_label(&self) -> String {
        self.hours_per_day
            .map_or_else(|| "待确认".to_owned(), |hours| format!("{hours} 小时/天"))
    }

    pub(crate) fn rest_days_label(&self) -> String {
        self.rest_days
            .map_or_else(|| "待确认".to_owned(), |days| format!("{days} 天/月"))
    }

    pub(crate) fn housing_allowance_label(&self) -> &str {
        match self.housing_allowance {
            Some(true) => "提供",
            Some(false) => "不提供",
            None => "待确认",
        }
    }

    pub(crate) fn availability_label(&self) -> &str {
        self.availability.as_deref().unwrap_or("待确认")
    }

    pub(crate) fn urgency_label(&self) -> &str {
        self.urgency.as_deref().unwrap_or("待确认")
    }

    pub(crate) fn source_label(&self) -> &str {
        self.source_label.as_deref().unwrap_or("待补充")
    }

    pub(crate) fn credibility_label(&self) -> String {
        self.credibility.map_or_else(
            || "待确认".to_owned(),
            |credibility| {
                format!(
                    "{credibility} / 100{}",
                    if self.prototype { "（原型字段）" } else { "" }
                )
            },
        )
    }
}

#[allow(clippy::cast_possible_truncation, clippy::too_many_lines)]
pub(crate) fn prototype_hiring_records() -> Vec<TalentRecord> {
    let fixtures = [
        (
            "proto-hire-01",
            "切配岗位（原型）",
            "连锁餐饮门店（原型）",
            "北京",
            "朝阳区",
            "切配",
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
                id: id.to_owned(),
                mode: TalentMode::Hiring,
                title: title.to_owned(),
                scene: Some(scene.to_owned()),
                region: region.to_owned(),
                district: district.to_owned(),
                location_terms: vec![region.to_owned(), district.to_owned()],
                salary_min: Some(4_800 + index as u32 * 400),
                salary_max: Some(7_000 + index as u32 * 500),
                skills: vec![skill.to_owned(), "餐饮门店协作（原型）".to_owned()],
                experience_years: Some((index % 5 + 1) as u8),
                hours_per_day: Some(8),
                rest_days: Some(4),
                lodging: Some(
                    if index % 2 == 0 {
                        "提供（原型）"
                    } else {
                        "不提供（原型）"
                    }
                    .to_owned(),
                ),
                housing_allowance: Some(index % 2 != 0),
                availability: Some("一周内到岗（原型）".to_owned()),
                urgency: Some(
                    if index < 2 {
                        "较高（原型）"
                    } else {
                        "常规（原型）"
                    }
                    .to_owned(),
                ),
                credibility: Some(72 + index as u8 * 3),
                source_label: Some("平台演示数据".to_owned()),
                updated_label: "近期更新（原型）".to_owned(),
                risk_flags: vec![
                    if index % 2 == 0 {
                        "薪资构成未拆分（原型）"
                    } else {
                        "岗位细节待确认（原型）"
                    }
                    .to_owned(),
                ],
                contact_masked: None,
                prototype: true,
            },
        )
        .collect()
}

#[allow(clippy::cast_possible_truncation, clippy::too_many_lines)]
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
                id: id.to_owned(),
                mode: TalentMode::Seeking,
                title: title.to_owned(),
                scene: Some(scene.to_owned()),
                region: region.to_owned(),
                district: district.to_owned(),
                location_terms: vec![region.to_owned(), district.to_owned()],
                salary_min: Some(4_500 + index as u32 * 450),
                salary_max: Some(6_500 + index as u32 * 550),
                skills: vec![skill.to_owned(), "餐饮门店协作（原型）".to_owned()],
                experience_years: Some((index % 6 + 1) as u8),
                hours_per_day: Some(8),
                rest_days: Some(4),
                lodging: Some(
                    if index % 2 == 0 {
                        "需要（原型）"
                    } else {
                        "可协商（原型）"
                    }
                    .to_owned(),
                ),
                housing_allowance: Some(index % 2 == 0),
                availability: Some(
                    if index < 3 {
                        "随时到岗（原型）"
                    } else {
                        "两周内到岗（原型）"
                    }
                    .to_owned(),
                ),
                urgency: Some(
                    if index < 2 {
                        "较高（原型）"
                    } else {
                        "常规（原型）"
                    }
                    .to_owned(),
                ),
                credibility: Some(70 + index as u8 * 3),
                source_label: Some("平台演示数据".to_owned()),
                updated_label: "近期更新（原型）".to_owned(),
                risk_flags: vec![
                    if index % 2 == 0 {
                        "技能证明未核验（原型）"
                    } else {
                        "工作经历待确认（原型）"
                    }
                    .to_owned(),
                ],
                contact_masked: None,
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
            let ids = records.iter().map(|record| record.id.clone()).collect::<HashSet<_>>();
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

    #[test]
    fn hiring_examples_cover_beijing_cutting_match_command() {
        let record = prototype_hiring_records().into_iter().find(|record| {
            record.region == "北京"
                && record.title.contains("切配")
                && record.salary_max.is_some_and(|salary| salary >= 7_000)
                && record.rest_days.is_some_and(|days| days >= 4)
                && record
                    .lodging
                    .as_deref()
                    .is_some_and(|lodging| lodging.starts_with("提供"))
        });

        assert!(record.is_some(), "北京切配示例指令应至少有一条硬条件匹配记录");
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
