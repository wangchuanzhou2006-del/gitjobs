use std::sync::OnceLock;

use regex::Regex;

use crate::talent::{ContactType, RiskReason, TalentKind, mask_contact};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EvidenceOffset {
    pub code: String,
    pub start_byte: usize,
    pub end_byte: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SalaryCandidate {
    pub minimum: u32,
    pub maximum: u32,
    pub evidence: EvidenceOffset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuleContactCandidate {
    pub contact_type: ContactType,
    pub masked_value: String,
    pub evidence: EvidenceOffset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuleRiskCandidate {
    pub reason: RiskReason,
    pub evidence: EvidenceOffset,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RuleHints {
    pub kind_hint: Option<TalentKind>,
    pub kind_evidence: Vec<EvidenceOffset>,
    pub salary_candidates: Vec<SalaryCandidate>,
    pub contact_candidates: Vec<RuleContactCandidate>,
    pub risk_candidates: Vec<RuleRiskCandidate>,
    has_hiring_signal: bool,
    has_seeking_signal: bool,
}

impl RuleHints {
    pub(crate) fn conflicts_with(&self, model_kind: TalentKind) -> bool {
        match model_kind {
            TalentKind::Hiring => self.has_seeking_signal,
            TalentKind::Seeking => self.has_hiring_signal,
            TalentKind::Irrelevant | TalentKind::Uncertain => {
                self.has_hiring_signal || self.has_seeking_signal
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct RuleEngine;

impl RuleEngine {
    #[allow(clippy::trivially_copy_pass_by_ref, clippy::unused_self)]
    pub(crate) fn analyze(&self, fragment: &str) -> RuleHints {
        let mut hints = RuleHints::default();
        add_kind_evidence(
            fragment,
            &["招聘", "招人", "急招", "诚聘"],
            "hiring_keyword",
            &mut hints.kind_evidence,
        );
        hints.has_hiring_signal = hints
            .kind_evidence
            .iter()
            .any(|evidence| evidence.code == "hiring_keyword");
        add_kind_evidence(
            fragment,
            &["求职", "找工作", "应聘"],
            "seeking_keyword",
            &mut hints.kind_evidence,
        );
        hints.has_seeking_signal = hints
            .kind_evidence
            .iter()
            .any(|evidence| evidence.code == "seeking_keyword");
        hints.kind_hint = match (hints.has_hiring_signal, hints.has_seeking_signal) {
            (true, false) => Some(TalentKind::Hiring),
            (false, true) => Some(TalentKind::Seeking),
            _ => None,
        };

        hints.salary_candidates = salary_candidates(fragment);
        hints.contact_candidates = contact_candidates(fragment);
        add_risk_candidates(
            fragment,
            &["报名费", "押金", "培训费", "中介费"],
            "fee_solicitation",
            "规则命中费用收取类别，需人工核验。",
            &mut hints.risk_candidates,
        );
        add_risk_candidates(
            fragment,
            &["刷单", "洗钱", "诈骗", "博彩"],
            "illegal_content",
            "规则命中违法违规内容类别，禁止自动发布。",
            &mut hints.risk_candidates,
        );
        add_risk_candidates(
            fragment,
            &["冒充", "身份不符", "代发招聘"],
            "identity_anomaly",
            "规则命中身份异常类别，需人工核验。",
            &mut hints.risk_candidates,
        );
        hints
    }
}

fn salary_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(
            r"(?i)(?:月薪|薪资|薪酬|工资)\s*[:：]?\s*(\d{1,6}(?:\.\d+)?)\s*([k千]?)\s*(?:-|－|—|~|～|至|到)\s*(\d{1,6}(?:\.\d+)?)\s*([k千]?)\s*(?:元|人民币)?",
        )
        .expect("salary regex is valid")
    })
}

fn phone_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)(?:电话|手机|联系电话|tel)\s*[:：]?\s*((?:\+?86[- ]?)?1[3-9]\d{9})(?:\b|$)")
            .expect("phone regex is valid")
    })
}

fn wechat_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)(?:微信|微信号|wx|wechat)\s*[:：]?\s*([A-Za-z][A-Za-z0-9_-]{5,19})(?:\b|$)")
            .expect("WeChat regex is valid")
    })
}

fn salary_candidates(fragment: &str) -> Vec<SalaryCandidate> {
    salary_pattern()
        .captures_iter(fragment)
        .filter_map(|captures| {
            let whole = captures.get(0)?;
            let first_value = captures.get(1)?.as_str();
            let first_unit = captures.get(2)?.as_str();
            let second_value = captures.get(3)?.as_str();
            let second_unit = captures.get(4)?.as_str();
            let first = parse_salary_endpoint(first_value, first_unit, second_unit)?;
            let second = parse_salary_endpoint(second_value, second_unit, first_unit)?;
            let (minimum, maximum) = if first <= second {
                (first, second)
            } else {
                (second, first)
            };
            (minimum >= 500 && maximum <= 1_000_000).then(|| SalaryCandidate {
                minimum,
                maximum,
                evidence: EvidenceOffset {
                    code: "salary_range".to_owned(),
                    start_byte: whole.start(),
                    end_byte: whole.end(),
                },
            })
        })
        .collect()
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn parse_salary(value: &str, unit: &str) -> Option<u32> {
    let value = value.parse::<f64>().ok()?;
    let multiplier = if unit.is_empty() { 1.0 } else { 1_000.0 };
    let salary = value * multiplier;
    (salary.is_finite() && salary >= 0.0 && salary <= f64::from(u32::MAX)).then(|| salary.round() as u32)
}

fn parse_salary_endpoint(value: &str, unit: &str, other_unit: &str) -> Option<u32> {
    let numeric = value.parse::<f64>().ok()?;
    let inferred_unit = if unit.is_empty() && !other_unit.is_empty() && numeric < 100.0 {
        other_unit
    } else {
        unit
    };
    parse_salary(value, inferred_unit)
}

fn contact_candidates(fragment: &str) -> Vec<RuleContactCandidate> {
    let mut candidates = Vec::new();
    for captures in phone_pattern().captures_iter(fragment) {
        if let Some(value) = captures.get(1) {
            candidates.push(RuleContactCandidate {
                contact_type: ContactType::Phone,
                masked_value: mask_contact(ContactType::Phone, value.as_str()),
                evidence: EvidenceOffset {
                    code: "phone_candidate".to_owned(),
                    start_byte: value.start(),
                    end_byte: value.end(),
                },
            });
        }
    }
    for captures in wechat_pattern().captures_iter(fragment) {
        if let Some(value) = captures.get(1) {
            candidates.push(RuleContactCandidate {
                contact_type: ContactType::Wechat,
                masked_value: mask_contact(ContactType::Wechat, value.as_str()),
                evidence: EvidenceOffset {
                    code: "wechat_candidate".to_owned(),
                    start_byte: value.start(),
                    end_byte: value.end(),
                },
            });
        }
    }
    candidates
}

fn add_kind_evidence(fragment: &str, keywords: &[&str], code: &str, evidence: &mut Vec<EvidenceOffset>) {
    for keyword in keywords {
        for (start_byte, _) in fragment.match_indices(keyword) {
            evidence.push(EvidenceOffset {
                code: code.to_owned(),
                start_byte,
                end_byte: start_byte + keyword.len(),
            });
        }
    }
}

fn add_risk_candidates(
    fragment: &str,
    keywords: &[&str],
    code: &str,
    explanation: &str,
    candidates: &mut Vec<RuleRiskCandidate>,
) {
    for keyword in keywords {
        for (start_byte, _) in fragment.match_indices(keyword) {
            candidates.push(RuleRiskCandidate {
                reason: RiskReason {
                    code: code.to_owned(),
                    explanation: explanation.to_owned(),
                    evidence: None,
                    hard_block: true,
                },
                evidence: EvidenceOffset {
                    code: code.to_owned(),
                    start_byte,
                    end_byte: start_byte + keyword.len(),
                },
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::talent::{ContactType, TalentKind};

    use super::RuleEngine;

    #[test]
    fn explicit_keywords_classify_hiring_and_seeking_fragments() {
        let hiring = RuleEngine.analyze("招聘后厨切配，月薪6000-8000元");
        let seeking = RuleEngine.analyze("求职服务员，希望在测试市工作");

        assert_eq!(hiring.kind_hint, Some(TalentKind::Hiring));
        assert_eq!(seeking.kind_hint, Some(TalentKind::Seeking));
        assert!(!hiring.kind_evidence.is_empty());
        assert!(!seeking.kind_evidence.is_empty());
    }

    #[test]
    fn ambiguous_or_implicit_language_is_not_classified() {
        let ambiguous = RuleEngine.analyze("招聘求职信息交流");
        let implicit = RuleEngine.analyze("后厨切配，随时到岗");

        assert_eq!(ambiguous.kind_hint, None);
        assert_eq!(implicit.kind_hint, None);
        assert!(ambiguous.conflicts_with(TalentKind::Hiring));
        assert!(!implicit.conflicts_with(TalentKind::Hiring));
    }

    #[test]
    fn salary_ranges_and_anchored_contacts_have_offsets_but_no_raw_evidence() {
        let fragment = "招聘厨师，月薪 6k-8k；电话：13800138000；微信：cook_2026";
        let hints = RuleEngine.analyze(fragment);

        assert_eq!(hints.salary_candidates.len(), 1);
        assert_eq!(hints.salary_candidates[0].minimum, 6_000);
        assert_eq!(hints.salary_candidates[0].maximum, 8_000);
        assert_eq!(hints.contact_candidates.len(), 2);
        assert_eq!(hints.contact_candidates[0].contact_type, ContactType::Phone);
        assert_eq!(hints.contact_candidates[0].masked_value, "138****8000");
        assert_eq!(hints.contact_candidates[1].contact_type, ContactType::Wechat);
        assert_eq!(hints.contact_candidates[1].masked_value, "coo***026");

        for evidence in hints
            .kind_evidence
            .iter()
            .chain(hints.salary_candidates.iter().map(|item| &item.evidence))
            .chain(hints.contact_candidates.iter().map(|item| &item.evidence))
        {
            assert!(evidence.start_byte < evidence.end_byte);
            assert!(evidence.end_byte <= fragment.len());
            assert!(!evidence.code.contains("13800138000"));
            assert!(!evidence.code.contains("cook_2026"));
        }
    }

    #[test]
    fn timestamps_and_unlabelled_identifiers_are_not_salary_or_wechat_candidates() {
        let hints = RuleEngine.analyze("发布于 10:30:45，编号 13800138000，账号 candidate_2026，招聘店员");

        assert!(hints.salary_candidates.is_empty());
        assert!(hints.contact_candidates.is_empty());
    }

    #[test]
    fn mixed_salary_units_preserve_each_endpoint_scale() {
        for (fragment, expected) in [
            ("招聘厨师，月薪6000-8k", (6_000, 8_000)),
            ("招聘厨师，月薪6k-8000", (6_000, 8_000)),
            ("招聘厨师，月薪6-8k", (6_000, 8_000)),
        ] {
            let hints = RuleEngine.analyze(fragment);

            assert_eq!(hints.salary_candidates.len(), 1, "{fragment}");
            assert_eq!(
                (
                    hints.salary_candidates[0].minimum,
                    hints.salary_candidates[0].maximum,
                ),
                expected,
                "{fragment}"
            );
        }
    }

    #[test]
    fn short_wechat_identifiers_use_canonical_non_revealing_masking() {
        let hints = RuleEngine.analyze("招聘店员，微信：abc123");
        let contact = &hints.contact_candidates[0];

        assert_eq!(contact.masked_value, "***");
        assert!(!contact.masked_value.contains("abc123"));
        assert_eq!(contact.evidence.code, "wechat_candidate");
        assert!(!contact.evidence.code.contains("abc123"));
    }

    #[test]
    fn hard_risk_keywords_are_emitted_before_model_analysis() {
        let hints = RuleEngine.analyze("招聘兼职，先缴纳报名费；工作内容是刷单；发布者自称可冒充门店招聘");
        let codes = hints
            .risk_candidates
            .iter()
            .map(|candidate| candidate.reason.code.as_str())
            .collect::<Vec<_>>();

        assert!(codes.contains(&"fee_solicitation"));
        assert!(codes.contains(&"illegal_content"));
        assert!(codes.contains(&"identity_anomaly"));
        assert!(
            hints
                .risk_candidates
                .iter()
                .all(|candidate| candidate.reason.hard_block)
        );
        assert!(
            hints
                .risk_candidates
                .iter()
                .all(|candidate| candidate.reason.evidence.is_none())
        );
    }
}
