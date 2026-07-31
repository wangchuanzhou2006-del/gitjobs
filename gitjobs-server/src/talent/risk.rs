use crate::talent::{
    DUPLICATE_HIGH_THRESHOLD, DUPLICATE_MEDIUM_THRESHOLD, DuplicateCandidate, RiskLevel, RiskReason,
    RuleHints, SourceTrust, TalentKind, ValidatedExtraction, canonical_model_risk,
};

pub(crate) const RISK_VERSION: &str = "talent-risk-v1";
const CONFIDENCE_THRESHOLD: f32 = 0.90;

#[derive(Clone, PartialEq)]
pub(crate) struct RiskInput {
    pub extraction: ValidatedExtraction,
    pub source_trust: SourceTrust,
    pub rule_hints: RuleHints,
    pub duplicates: Vec<DuplicateCandidate>,
    pub identity_anomaly: bool,
    pub model_rule_conflict: bool,
}

impl std::fmt::Debug for RiskInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RiskInput")
            .field("extraction", &"[REDACTED SAFE EXTRACTION]")
            .field("source_trust", &self.source_trust)
            .field("rule_hints", &"[REDACTED RULE HINTS]")
            .field("duplicate_count", &self.duplicates.len())
            .field("identity_anomaly", &self.identity_anomaly)
            .field("model_rule_conflict", &self.model_rule_conflict)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RiskAssessment {
    pub version: &'static str,
    pub score: u8,
    pub level: RiskLevel,
    pub reasons: Vec<RiskReason>,
    pub hard_block: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct RiskEngine;

impl RiskEngine {
    pub(crate) fn evaluate(&self, input: RiskInput) -> RiskAssessment {
        let mut score = 0_u16;
        let mut reasons = Vec::new();
        let rule_conflict = input.rule_hints.conflicts_with(input.extraction.safe.kind);

        for candidate in input.rule_hints.risk_candidates {
            push_reason(&mut reasons, candidate.reason);
        }
        if !input.extraction.safe.risk_candidates.is_empty() {
            score += 30;
        }
        for candidate in &input.extraction.safe.risk_candidates {
            if let Some(reason) = canonical_model_risk(&candidate.code) {
                push_reason(&mut reasons, reason);
            }
        }
        if input.identity_anomaly {
            push_reason(
                &mut reasons,
                hard_reason("identity_anomaly", "身份信息存在异常类别，禁止自动发布。"),
            );
        }
        match input.source_trust {
            SourceTrust::New => {
                score += 20;
                push_reason(
                    &mut reasons,
                    soft_reason("source_new", "来源尚未完成可信确认，需人工审核。"),
                );
            }
            SourceTrust::Trusted => {}
            SourceTrust::Watchlist => {
                score += 40;
                push_reason(
                    &mut reasons,
                    soft_reason("source_watchlist", "来源处于观察类别，需人工审核。"),
                );
            }
            SourceTrust::Blocked => {
                push_reason(
                    &mut reasons,
                    hard_reason("source_blocked", "来源处于禁止类别，禁止自动发布。"),
                );
            }
        }

        if !input.extraction.safe.missing_fields.is_empty() || !input.extraction.issues.is_empty() {
            let count = input.extraction.safe.missing_fields.len() + input.extraction.issues.len();
            score += u16::try_from(count.saturating_mul(10).min(30)).unwrap_or(30);
            push_reason(
                &mut reasons,
                soft_reason("missing_fields", "结构化结果存在缺失字段类别，需人工补充。"),
            );
        }
        if input.extraction.safe.confidence < CONFIDENCE_THRESHOLD {
            score += 20;
            push_reason(
                &mut reasons,
                soft_reason(
                    "confidence_below_threshold",
                    "模型置信度低于自动发布门槛，需人工审核。",
                ),
            );
        }
        if !matches!(
            input.extraction.safe.kind,
            TalentKind::Hiring | TalentKind::Seeking
        ) {
            score += 30;
            push_reason(
                &mut reasons,
                soft_reason("unsupported_kind", "内容类型不支持自动发布，需人工分类。"),
            );
        }
        if input.model_rule_conflict || rule_conflict {
            score += 30;
            push_reason(
                &mut reasons,
                soft_reason("model_rule_conflict", "规则分类与模型分类存在冲突，需人工审核。"),
            );
        }

        if input
            .duplicates
            .iter()
            .any(|candidate| candidate.score >= DUPLICATE_HIGH_THRESHOLD)
        {
            push_reason(
                &mut reasons,
                hard_reason("duplicate_high", "存在高相似重复类别，禁止自动发布。"),
            );
        } else if input
            .duplicates
            .iter()
            .any(|candidate| candidate.score >= DUPLICATE_MEDIUM_THRESHOLD)
        {
            score += 30;
            push_reason(
                &mut reasons,
                soft_reason("duplicate_medium", "存在中等相似重复类别，需人工审核。"),
            );
        }

        let hard_block = reasons.iter().any(|reason| reason.hard_block);
        let score = if hard_block {
            100
        } else {
            u8::try_from(score.min(100)).unwrap_or(100)
        };
        let level = risk_level_for_score(score, hard_block);
        RiskAssessment {
            version: RISK_VERSION,
            score,
            level,
            reasons,
            hard_block,
        }
    }
}

fn risk_level_for_score(score: u8, hard_block: bool) -> RiskLevel {
    if hard_block || score >= 70 {
        RiskLevel::High
    } else if score >= 30 {
        RiskLevel::Medium
    } else {
        RiskLevel::Low
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct AutoPublishInput {
    pub global_enabled: bool,
    pub source_trust: SourceTrust,
    pub source_enabled: bool,
    pub source_confirmed: bool,
    pub assessment: RiskAssessment,
    pub extraction: ValidatedExtraction,
    pub duplicates: Vec<DuplicateCandidate>,
}

impl std::fmt::Debug for AutoPublishInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AutoPublishInput")
            .field("global_enabled", &self.global_enabled)
            .field("source_trust", &self.source_trust)
            .field("source_enabled", &self.source_enabled)
            .field("source_confirmed", &self.source_confirmed)
            .field("risk_score", &self.assessment.score)
            .field("risk_level", &self.assessment.level)
            .field("risk_hard_block", &self.assessment.hard_block)
            .field("extraction", &"[REDACTED SAFE EXTRACTION]")
            .field("duplicate_count", &self.duplicates.len())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AutoPublishDecision {
    pub eligible: bool,
    pub reasons: Vec<RiskReason>,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct AutoPublishPolicy;

impl AutoPublishPolicy {
    pub(crate) fn decide(&self, input: AutoPublishInput) -> AutoPublishDecision {
        let mut reasons = Vec::new();
        if !input.global_enabled {
            push_reason(
                &mut reasons,
                policy_reason("global_auto_publish_paused", "全局自动发布开关已暂停。"),
            );
        }
        if input.source_trust != SourceTrust::Trusted {
            push_reason(
                &mut reasons,
                policy_reason("source_not_trusted", "来源未处于可信类别。"),
            );
        }
        if !input.source_enabled {
            push_reason(
                &mut reasons,
                policy_reason("source_auto_publish_paused", "来源自动发布开关已暂停。"),
            );
        }
        if !input.source_confirmed {
            push_reason(
                &mut reasons,
                policy_reason("source_not_confirmed", "来源尚未完成确认。"),
            );
        }
        if input.assessment.version != RISK_VERSION {
            push_reason(
                &mut reasons,
                policy_reason("risk_version_mismatch", "风险规则版本不符合发布要求。"),
            );
        }
        if input.assessment.level != RiskLevel::Low {
            push_reason(
                &mut reasons,
                policy_reason("risk_not_low", "风险等级不符合自动发布要求。"),
            );
        }
        if input.assessment.hard_block {
            push_reason(
                &mut reasons,
                policy_reason("risk_hard_block", "存在禁止自动发布的硬性风险类别。"),
            );
        }
        if !matches!(
            input.extraction.safe.kind,
            TalentKind::Hiring | TalentKind::Seeking
        ) {
            push_reason(
                &mut reasons,
                policy_reason("unsupported_kind", "内容类型不支持自动发布。"),
            );
        }
        if input.extraction.safe.confidence < CONFIDENCE_THRESHOLD {
            push_reason(
                &mut reasons,
                policy_reason("confidence_below_threshold", "模型置信度低于自动发布门槛。"),
            );
        }
        if !has_required_fields(&input.extraction) {
            push_reason(
                &mut reasons,
                policy_reason("missing_required_fields", "自动发布所需结构化字段不完整。"),
            );
        }
        if input
            .duplicates
            .iter()
            .any(|candidate| candidate.score >= DUPLICATE_HIGH_THRESHOLD)
        {
            push_reason(
                &mut reasons,
                policy_reason("duplicate_high", "存在高相似重复类别。"),
            );
        }
        for assessment_reason in input.assessment.reasons {
            push_reason(&mut reasons, assessment_reason);
        }
        AutoPublishDecision {
            eligible: reasons.is_empty(),
            reasons,
        }
    }
}

fn has_required_fields(extraction: &ValidatedExtraction) -> bool {
    if !extraction.safe.missing_fields.is_empty()
        || !extraction.issues.is_empty()
        || extraction.safe.roles.is_empty()
        || extraction.contacts.is_empty()
        || extraction.safe.published_at.is_none()
        || (extraction.safe.city.is_none() && extraction.safe.district.is_none())
    {
        return false;
    }
    match extraction.safe.kind {
        TalentKind::Hiring => extraction.safe.salary_min.is_some() && extraction.safe.salary_max.is_some(),
        TalentKind::Seeking => true,
        TalentKind::Irrelevant | TalentKind::Uncertain => false,
    }
}

fn push_reason(reasons: &mut Vec<RiskReason>, reason: RiskReason) {
    if !reasons.iter().any(|existing| existing.code == reason.code) {
        reasons.push(reason);
    }
}

fn hard_reason(code: &str, explanation: &str) -> RiskReason {
    RiskReason {
        code: code.to_owned(),
        explanation: explanation.to_owned(),
        evidence: None,
        hard_block: true,
    }
}

fn soft_reason(code: &str, explanation: &str) -> RiskReason {
    RiskReason {
        code: code.to_owned(),
        explanation: explanation.to_owned(),
        evidence: None,
        hard_block: false,
    }
}

fn policy_reason(code: &str, explanation: &str) -> RiskReason {
    soft_reason(code, explanation)
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use crate::talent::{
        ContactCandidate, ContactType, DuplicateCandidate, RiskLevel, RiskReason, SourceTrust,
        TalentExtraction, TalentKind, ValidatedExtraction,
    };

    use super::{
        AutoPublishInput, AutoPublishPolicy, RISK_VERSION, RiskAssessment, RiskEngine, RiskInput,
        risk_level_for_score,
    };
    use crate::talent::RuleEngine;

    fn validated(extraction: TalentExtraction, has_contact: bool) -> ValidatedExtraction {
        ValidatedExtraction {
            safe: extraction,
            contacts: if has_contact {
                vec![ContactCandidate {
                    contact_type: ContactType::Wechat,
                    value: "private_contact_2026".to_owned(),
                }]
            } else {
                Vec::new()
            },
            issues: Vec::new(),
        }
    }

    fn complete_hiring(confidence: f32) -> ValidatedExtraction {
        validated(
            TalentExtraction {
                kind: TalentKind::Hiring,
                confidence,
                roles: vec!["chef".to_owned()],
                city: Some("test city".to_owned()),
                salary_min: Some(6_000),
                salary_max: Some(8_000),
                published_at: Some(Utc::now()),
                ..TalentExtraction::default()
            },
            true,
        )
    }

    fn complete_seeking(confidence: f32) -> ValidatedExtraction {
        validated(
            TalentExtraction {
                kind: TalentKind::Seeking,
                confidence,
                roles: vec!["waiter".to_owned()],
                district: Some("test district".to_owned()),
                published_at: Some(Utc::now()),
                ..TalentExtraction::default()
            },
            true,
        )
    }

    fn duplicate(score: f32) -> DuplicateCandidate {
        DuplicateCandidate {
            candidate_item_id: None,
            candidate_record_id: None,
            score,
            reasons: vec!["stable_duplicate_category".to_owned()],
            high_similarity: score >= 0.85,
        }
    }

    fn assess(
        extraction: ValidatedExtraction,
        source_trust: SourceTrust,
        fragment: &str,
        identity_anomaly: bool,
        model_rule_conflict: bool,
        duplicates: Vec<DuplicateCandidate>,
    ) -> RiskAssessment {
        RiskEngine::default().evaluate(RiskInput {
            extraction,
            source_trust,
            rule_hints: RuleEngine::default().analyze(fragment),
            duplicates,
            identity_anomaly,
            model_rule_conflict,
        })
    }

    fn low_risk() -> RiskAssessment {
        assess(
            complete_hiring(0.95),
            SourceTrust::Trusted,
            "招聘厨师",
            false,
            false,
            vec![],
        )
    }

    fn codes(reasons: &[crate::talent::RiskReason]) -> Vec<&str> {
        reasons.iter().map(|reason| reason.code.as_str()).collect()
    }

    #[test]
    fn model_only_risks_force_review_and_server_rules_decide_hard_blocks() {
        let mut extraction = complete_hiring(0.95);
        extraction.safe.risk_candidates.push(RiskReason {
            code: "fee_solicitation".to_owned(),
            explanation: "provider-controlled explanation".to_owned(),
            evidence: Some("provider-controlled evidence".to_owned()),
            hard_block: false,
        });

        let assessment = assess(
            extraction.clone(),
            SourceTrust::Trusted,
            "hiring cook",
            false,
            false,
            vec![],
        );

        assert!(assessment.hard_block);
        assert_eq!(assessment.level, RiskLevel::High);
        assert!(codes(&assessment.reasons).contains(&"fee_solicitation"));
        let reason = assessment
            .reasons
            .iter()
            .find(|reason| reason.code == "fee_solicitation")
            .unwrap();
        assert!(reason.hard_block);
        assert!(reason.evidence.is_none());
        assert_ne!(reason.explanation, "provider-controlled explanation");

        let decision = AutoPublishPolicy.decide(AutoPublishInput {
            global_enabled: true,
            source_trust: SourceTrust::Trusted,
            source_enabled: true,
            source_confirmed: true,
            assessment,
            extraction,
            duplicates: vec![],
        });
        assert!(!decision.eligible);
    }

    #[test]
    fn assessment_uses_named_version_and_source_trust_soft_rules() {
        let trusted = assess(
            complete_hiring(0.95),
            SourceTrust::Trusted,
            "招聘厨师",
            false,
            false,
            vec![],
        );
        let new_source = assess(
            complete_hiring(0.95),
            SourceTrust::New,
            "招聘厨师",
            false,
            false,
            vec![],
        );
        let watchlist = assess(
            complete_hiring(0.95),
            SourceTrust::Watchlist,
            "招聘厨师",
            false,
            false,
            vec![],
        );

        assert_eq!(trusted.version, RISK_VERSION);
        assert_eq!(trusted.level, RiskLevel::Low);
        assert_eq!(new_source.level, RiskLevel::Low);
        assert_eq!(watchlist.level, RiskLevel::Medium);
        assert!(new_source.score < watchlist.score);
    }

    #[test]
    fn risk_level_boundaries_follow_the_global_contract() {
        assert_eq!(risk_level_for_score(29, false), RiskLevel::Low);
        assert_eq!(risk_level_for_score(30, false), RiskLevel::Medium);
        assert_eq!(risk_level_for_score(69, false), RiskLevel::Medium);
        assert_eq!(risk_level_for_score(70, false), RiskLevel::High);
        assert_eq!(risk_level_for_score(0, true), RiskLevel::High);
    }

    #[test]
    fn fee_illegal_identity_blocked_source_and_high_duplicate_are_hard_blocks() {
        let assessment = assess(
            complete_hiring(0.95),
            SourceTrust::Blocked,
            "招聘兼职，先缴纳报名费，工作内容是刷单",
            true,
            false,
            vec![duplicate(0.85)],
        );
        let reason_codes = codes(&assessment.reasons);

        assert_eq!(assessment.score, 100);
        assert_eq!(assessment.level, RiskLevel::High);
        assert!(assessment.hard_block);
        for expected in [
            "fee_solicitation",
            "illegal_content",
            "identity_anomaly",
            "source_blocked",
            "duplicate_high",
        ] {
            assert!(reason_codes.contains(&expected), "{expected}");
        }
    }

    #[test]
    fn soft_rules_accumulate_and_cap_at_one_hundred_without_becoming_hard() {
        let mut extraction = complete_hiring(0.1);
        extraction.safe.missing_fields = (0..20).map(|index| format!("field_{index}")).collect();
        let assessment = assess(
            extraction,
            SourceTrust::Watchlist,
            "招聘厨师",
            false,
            true,
            vec![duplicate(0.60)],
        );
        let reason_codes = codes(&assessment.reasons);

        assert_eq!(assessment.score, 100);
        assert_eq!(assessment.level, RiskLevel::High);
        assert!(!assessment.hard_block);
        assert!(reason_codes.contains(&"missing_fields"));
        assert!(reason_codes.contains(&"model_rule_conflict"));
        assert!(reason_codes.contains(&"confidence_below_threshold"));
        assert!(reason_codes.contains(&"source_watchlist"));
        assert!(reason_codes.contains(&"duplicate_medium"));
    }

    #[test]
    fn moderator_explanations_only_describe_categories() {
        let assessment = assess(
            complete_hiring(0.95),
            SourceTrust::Trusted,
            "招聘厨师，电话：13800138000，需缴纳报名费",
            false,
            false,
            vec![],
        );

        for reason in assessment.reasons {
            assert!(!reason.explanation.contains("13800138000"));
            assert!(!reason.explanation.contains("报名费"));
            assert!(!reason.explanation.contains("招聘厨师"));
            assert!(reason.evidence.is_none());
        }
    }

    #[test]
    fn auto_publish_retains_every_independent_failure_reason() {
        let extraction = validated(
            TalentExtraction {
                kind: TalentKind::Uncertain,
                confidence: 0.50,
                missing_fields: vec!["role".to_owned()],
                ..TalentExtraction::default()
            },
            false,
        );
        let assessment = assess(
            extraction.clone(),
            SourceTrust::Blocked,
            "招聘求职，需缴纳报名费",
            true,
            true,
            vec![duplicate(0.85)],
        );

        let decision = AutoPublishPolicy::default().decide(AutoPublishInput {
            global_enabled: false,
            source_trust: SourceTrust::Blocked,
            source_enabled: false,
            source_confirmed: false,
            assessment,
            extraction,
            duplicates: vec![duplicate(0.85)],
        });
        let reason_codes = codes(&decision.reasons);

        assert!(!decision.eligible);
        for expected in [
            "global_auto_publish_paused",
            "source_not_trusted",
            "source_auto_publish_paused",
            "source_not_confirmed",
            "risk_not_low",
            "risk_hard_block",
            "unsupported_kind",
            "confidence_below_threshold",
            "missing_required_fields",
            "duplicate_high",
            "model_rule_conflict",
        ] {
            assert!(reason_codes.contains(&expected), "{expected}");
        }
    }

    #[test]
    fn confidence_model_rule_conflict_and_missing_fields_each_prevent_publication() {
        let cases = [
            assess(
                complete_hiring(0.89),
                SourceTrust::Trusted,
                "招聘厨师",
                false,
                false,
                vec![],
            ),
            assess(
                complete_hiring(0.95),
                SourceTrust::Trusted,
                "求职厨师",
                false,
                true,
                vec![],
            ),
        ];

        for (index, assessment) in cases.into_iter().enumerate() {
            let extraction = if index == 0 {
                complete_hiring(0.89)
            } else {
                complete_hiring(0.95)
            };
            let decision = AutoPublishPolicy::default().decide(AutoPublishInput {
                global_enabled: true,
                source_trust: SourceTrust::Trusted,
                source_enabled: true,
                source_confirmed: true,
                assessment,
                extraction,
                duplicates: vec![],
            });
            assert!(!decision.eligible);
        }

        let incomplete = validated(
            TalentExtraction {
                kind: TalentKind::Hiring,
                confidence: 0.95,
                roles: vec!["chef".to_owned()],
                city: Some("test city".to_owned()),
                missing_fields: vec!["salary".to_owned()],
                published_at: Some(Utc::now()),
                ..TalentExtraction::default()
            },
            true,
        );
        let assessment = assess(
            incomplete.clone(),
            SourceTrust::Trusted,
            "招聘厨师",
            false,
            false,
            vec![],
        );
        let decision = AutoPublishPolicy::default().decide(AutoPublishInput {
            global_enabled: true,
            source_trust: SourceTrust::Trusted,
            source_enabled: true,
            source_confirmed: true,
            assessment,
            extraction: incomplete,
            duplicates: vec![],
        });
        assert!(!decision.eligible);
    }

    #[test]
    fn global_and_source_pauses_are_independent() {
        let extraction = complete_hiring(0.95);
        let assessment = low_risk();
        let decision = AutoPublishPolicy::default().decide(AutoPublishInput {
            global_enabled: false,
            source_trust: SourceTrust::Trusted,
            source_enabled: false,
            source_confirmed: true,
            assessment,
            extraction,
            duplicates: vec![],
        });
        let reason_codes = codes(&decision.reasons);

        assert!(reason_codes.contains(&"global_auto_publish_paused"));
        assert!(reason_codes.contains(&"source_auto_publish_paused"));
    }

    #[test]
    fn complete_low_risk_hiring_and_seeking_are_eligible() {
        for extraction in [complete_hiring(0.95), complete_seeking(0.95)] {
            let assessment = assess(
                extraction.clone(),
                SourceTrust::Trusted,
                match extraction.safe.kind {
                    TalentKind::Hiring => "招聘厨师",
                    TalentKind::Seeking => "求职服务员",
                    _ => unreachable!(),
                },
                false,
                false,
                vec![],
            );
            let decision = AutoPublishPolicy::default().decide(AutoPublishInput {
                global_enabled: true,
                source_trust: SourceTrust::Trusted,
                source_enabled: true,
                source_confirmed: true,
                assessment,
                extraction,
                duplicates: vec![],
            });

            assert!(decision.eligible);
            assert!(decision.reasons.is_empty());
        }
    }
}
