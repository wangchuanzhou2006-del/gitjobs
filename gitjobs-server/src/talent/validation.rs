use std::{collections::HashSet, sync::OnceLock};

use regex::Regex;
use serde_json::Value;

use crate::{
    talent::{ContactCandidate, RiskReason, TalentExtraction, normalize_contact},
    validation::{MAX_ITEMS, MAX_LEN_ENTITY_NAME, MAX_LEN_M, MAX_LEN_TAG},
};

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const MODEL_RISK_ALLOWLIST_VERSION: &str = "talent-model-risk-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidationIssue {
    pub code: String,
    pub field: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ValidatedExtraction {
    pub safe: TalentExtraction,
    pub contacts: Vec<ContactCandidate>,
    pub issues: Vec<ValidationIssue>,
}

/// Validates data produced by an external model before it can cross the safe-data boundary.
pub(crate) fn validate_model_extraction(
    mut extraction: TalentExtraction,
    source_fragment: &str,
) -> Result<ValidatedExtraction, Vec<ValidationIssue>> {
    let mut issues = canonicalize_model_risks(&mut extraction.risk_candidates);
    let contacts = extraction.contacts.clone();
    match serde_json::to_value(&extraction) {
        Ok(value) => {
            collect_unsafe_model_strings(&value, "model_output", source_fragment, &contacts, &mut issues);
        }
        Err(_) => issue(&mut issues, "invalid_model_output", "model_output"),
    }

    match validate_extraction(extraction) {
        Ok(validated) if issues.is_empty() => Ok(validated),
        Ok(_) => Err(issues),
        Err(mut validation_issues) => {
            issues.append(&mut validation_issues);
            Err(issues)
        }
    }
}

/// Defense in depth for callers constructing a supposedly safe extraction directly.
pub(crate) fn safe_extraction_contains_private_data(extraction: &TalentExtraction) -> bool {
    let Ok(value) = serde_json::to_value(extraction) else {
        return true;
    };
    value_contains_unsafe_model_string(&value, "", &[])
}

pub(crate) fn canonical_model_risk(code: &str) -> Option<RiskReason> {
    let code = normalize(code).to_ascii_lowercase();
    let (explanation, hard_block) = match code.as_str() {
        "fee_solicitation" => ("Model identified a fee-solicitation risk category.", true),
        "illegal_content" => ("Model identified an illegal-content risk category.", true),
        "identity_anomaly" => ("Model identified an identity-anomaly risk category.", true),
        _ => return None,
    };
    Some(RiskReason {
        code,
        explanation: explanation.to_owned(),
        evidence: None,
        hard_block,
    })
}

fn canonicalize_model_risks(risks: &mut Vec<RiskReason>) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let mut canonical = Vec::with_capacity(risks.len());
    for risk in risks.drain(..) {
        match canonical_model_risk(&risk.code) {
            Some(risk)
                if !canonical
                    .iter()
                    .any(|existing: &RiskReason| existing.code == risk.code) =>
            {
                canonical.push(risk);
            }
            Some(_) => {}
            None => issue(&mut issues, "unsupported_model_risk", "risk_candidates.code"),
        }
    }
    *risks = canonical;
    issues
}

fn collect_unsafe_model_strings(
    value: &Value,
    path: &str,
    source_fragment: &str,
    contacts: &[ContactCandidate],
    issues: &mut Vec<ValidationIssue>,
) {
    match value {
        Value::Object(values) => {
            for (field, value) in values {
                if field == "contacts" {
                    continue;
                }
                let child_path = format!("{path}.{field}");
                collect_unsafe_model_strings(value, &child_path, source_fragment, contacts, issues);
            }
        }
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                let child_path = format!("{path}[{index}]");
                collect_unsafe_model_strings(value, &child_path, source_fragment, contacts, issues);
            }
        }
        Value::String(value) if model_string_is_unsafe(value, source_fragment, contacts) => {
            issue(issues, "unsafe_model_string", path);
        }
        _ => {}
    }
}

fn value_contains_unsafe_model_string(
    value: &Value,
    source_fragment: &str,
    contacts: &[ContactCandidate],
) -> bool {
    match value {
        Value::Object(values) => values.iter().any(|(field, value)| {
            field != "contacts" && value_contains_unsafe_model_string(value, source_fragment, contacts)
        }),
        Value::Array(values) => values
            .iter()
            .any(|value| value_contains_unsafe_model_string(value, source_fragment, contacts)),
        Value::String(value) => model_string_is_unsafe(value, source_fragment, contacts),
        _ => false,
    }
}

fn model_string_is_unsafe(value: &str, source_fragment: &str, contacts: &[ContactCandidate]) -> bool {
    let value = value.trim();
    if value.is_empty() {
        return false;
    }
    if phone_pattern().is_match(value)
        || labelled_contact_pattern().is_match(value)
        || truth_claim_pattern().is_match(value)
        || protected_marker_pattern().is_match(value)
    {
        return true;
    }
    if contacts.iter().any(|contact| {
        let needle = normalize_contact(contact.contact_type, &contact.value);
        needle.chars().count() >= 3 && normalize_contact(contact.contact_type, value).contains(&needle)
    }) {
        return true;
    }

    let value = normalize(value);
    let source = normalize(source_fragment);
    !source.is_empty()
        && !value.is_empty()
        && source.contains(&value)
        && (source == value || value.chars().count() >= 32)
}

fn phone_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?:^|[^\d])(?:\+?86[- ()]*)?1[3-9]\d(?:[- ()]*\d){8}(?:[^\d]|$)")
            .expect("complete phone regex is valid")
    })
}

fn labelled_contact_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(
            r"(?i)(?:\u{5fae}\u{4fe1}(?:\u{53f7})?|wx|wechat|qq|\u{6263}\u{6263})\s*[:\u{ff1a}]?\s*(?:[A-Za-z][A-Za-z0-9_-]{5,19}|[1-9]\d{4,11})",
        )
        .expect("labelled contact regex is valid")
    })
}

fn truth_claim_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)\b(?:verified|authenticated|genuine)\b|\u{5df2}\u{8ba4}\u{8bc1}|\u{5df2}\u{6838}\u{5b9e}|\u{786e}\u{8ba4}\u{771f}\u{5b9e}|\u{771f}\u{5b9e}\u{53ef}\u{9760}|\u{8ba4}\u{8bc1}\u{901a}\u{8fc7}")
            .expect("truth-claim regex is valid")
    })
}

fn protected_marker_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)(?:private|raw(?:[_ -]?(?:text|marker))|sentinel)")
            .expect("protected marker regex is valid")
    })
}

pub(crate) fn validate_extraction(
    mut extraction: TalentExtraction,
) -> Result<ValidatedExtraction, Vec<ValidationIssue>> {
    let contacts = std::mem::take(&mut extraction.contacts);
    let mut issues = Vec::new();
    validate_collection_size(&mut issues, "roles", extraction.roles.len());
    validate_collection_size(&mut issues, "skills", extraction.skills.len());
    validate_collection_size(&mut issues, "missing_fields", extraction.missing_fields.len());
    validate_collection_size(&mut issues, "risk_candidates", extraction.risk_candidates.len());
    validate_collection_size(&mut issues, "contacts", contacts.len());

    normalize_optional(&mut extraction.title);
    normalize_optional(&mut extraction.scene);
    normalize_optional(&mut extraction.region);
    normalize_optional(&mut extraction.city);
    normalize_optional(&mut extraction.district);
    normalize_optional(&mut extraction.lodging);
    normalize_optional(&mut extraction.availability);
    normalize_optional(&mut extraction.urgency);
    normalize_values(&mut extraction.roles);
    normalize_values(&mut extraction.skills);
    normalize_values(&mut extraction.missing_fields);
    normalize_risks(&mut extraction.risk_candidates);

    validate_optional(
        &mut issues,
        "title",
        extraction.title.as_deref(),
        MAX_LEN_ENTITY_NAME,
    );
    for (field, value) in [
        ("scene", extraction.scene.as_deref()),
        ("region", extraction.region.as_deref()),
        ("city", extraction.city.as_deref()),
        ("district", extraction.district.as_deref()),
        ("lodging", extraction.lodging.as_deref()),
        ("availability", extraction.availability.as_deref()),
        ("urgency", extraction.urgency.as_deref()),
    ] {
        validate_optional(&mut issues, field, value, MAX_LEN_M);
    }
    validate_values(&mut issues, "roles", &extraction.roles, MAX_LEN_TAG);
    validate_values(&mut issues, "skills", &extraction.skills, MAX_LEN_TAG);
    validate_values(
        &mut issues,
        "missing_fields",
        &extraction.missing_fields,
        MAX_LEN_M,
    );
    validate_risks(&mut issues, &extraction.risk_candidates);
    validate_contacts(&mut issues, &contacts);

    if !(0.0..=1.0).contains(&extraction.confidence) {
        issue(&mut issues, "confidence_out_of_range", "confidence");
    }
    if extraction
        .salary_min
        .zip(extraction.salary_max)
        .is_some_and(|(min, max)| min > max)
    {
        issue(&mut issues, "salary_range_inverted", "salary");
    }
    if extraction.hours_per_day.is_some_and(|hours| hours > 24) {
        issue(&mut issues, "hours_per_day_out_of_range", "hours_per_day");
    }
    if extraction.rest_days_per_month.is_some_and(|days| days > 31) {
        issue(
            &mut issues,
            "rest_days_per_month_out_of_range",
            "rest_days_per_month",
        );
    }
    if extraction.experience_years.is_some_and(|years| years > 70) {
        issue(&mut issues, "experience_years_out_of_range", "experience_years");
    }

    if issues.is_empty() {
        Ok(ValidatedExtraction {
            safe: extraction,
            contacts,
            issues,
        })
    } else {
        Err(issues)
    }
}

fn normalize_optional(value: &mut Option<String>) {
    if let Some(value) = value {
        *value = normalize(value);
    }
}

fn normalize_values(values: &mut Vec<String>) {
    let mut seen = HashSet::new();
    values.retain_mut(|value| {
        *value = normalize(value);
        seen.insert(value.clone())
    });
}

fn normalize_risks(risks: &mut [RiskReason]) {
    for risk in risks {
        risk.code = normalize(&risk.code);
        risk.explanation = normalize(&risk.explanation);
        normalize_optional(&mut risk.evidence);
    }
}

fn normalize(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn validate_optional(issues: &mut Vec<ValidationIssue>, field: &str, value: Option<&str>, maximum: usize) {
    if let Some(value) = value {
        validate_string(issues, field, value, maximum);
    }
}

fn validate_collection_size(issues: &mut Vec<ValidationIssue>, field: &str, length: usize) {
    if length > MAX_ITEMS {
        issue(issues, &format!("{field}_too_many"), field);
    }
}

fn validate_values(issues: &mut Vec<ValidationIssue>, field: &str, values: &[String], maximum: usize) {
    if values.len() > MAX_ITEMS {
        issue(issues, &format!("{field}_too_many"), field);
    }
    for value in values {
        validate_string(issues, &format!("{field}_item"), value, maximum);
    }
}

fn validate_contacts(issues: &mut Vec<ValidationIssue>, contacts: &[ContactCandidate]) {
    if contacts.len() > MAX_ITEMS {
        issue(issues, "contacts_too_many", "contacts");
    }
    for contact in contacts {
        validate_string(issues, "contact_value", &contact.value, MAX_LEN_M);
    }
}

fn validate_risks(issues: &mut Vec<ValidationIssue>, risks: &[RiskReason]) {
    if risks.len() > MAX_ITEMS {
        issue(issues, "risk_candidates_too_many", "risk_candidates");
    }
    for risk in risks {
        validate_string(issues, "risk_code", &risk.code, MAX_LEN_TAG);
        validate_string(issues, "risk_explanation", &risk.explanation, MAX_LEN_M);
        validate_optional(issues, "risk_evidence", risk.evidence.as_deref(), MAX_LEN_M);
    }
}

fn validate_string(issues: &mut Vec<ValidationIssue>, field: &str, value: &str, maximum: usize) {
    if value.is_empty() {
        issue(issues, &format!("{field}_empty"), field);
    } else if value.chars().count() > maximum {
        issue(issues, &format!("{field}_too_long"), field);
    }
}

fn issue(issues: &mut Vec<ValidationIssue>, code: &str, field: &str) {
    issues.push(ValidationIssue {
        code: code.to_owned(),
        field: field.to_owned(),
    });
}

#[cfg(test)]
mod tests {
    use crate::talent::{ContactCandidate, ContactType, RiskReason, TalentExtraction, TalentKind};

    use super::{MODEL_RISK_ALLOWLIST_VERSION, validate_extraction, validate_model_extraction};

    fn valid_extraction() -> TalentExtraction {
        TalentExtraction {
            kind: TalentKind::Hiring,
            confidence: 0.8,
            title: Some(" Developer ".to_owned()),
            roles: vec![" Engineer ".to_owned(), "Engineer".to_owned()],
            skills: vec![" Rust ".to_owned(), "Rust".to_owned()],
            contacts: vec![ContactCandidate {
                contact_type: ContactType::Wechat,
                value: "candidate_01".to_owned(),
            }],
            ..TalentExtraction::default()
        }
    }

    #[test]
    fn validation_returns_normalized_safe_data_and_separate_contacts() {
        let validated = validate_extraction(valid_extraction()).unwrap();

        assert_eq!(validated.safe.title.as_deref(), Some("Developer"));
        assert_eq!(validated.safe.roles, ["Engineer"]);
        assert_eq!(validated.safe.skills, ["Rust"]);
        assert!(validated.safe.contacts.is_empty());
        assert_eq!(validated.contacts.len(), 1);
        assert!(validated.issues.is_empty());
    }

    #[test]
    fn validation_reports_numeric_ranges() {
        let invalid = TalentExtraction {
            confidence: 1.2,
            salary_min: Some(8_000),
            salary_max: Some(6_000),
            hours_per_day: Some(25),
            rest_days_per_month: Some(32),
            experience_years: Some(71),
            ..valid_extraction()
        };
        let issues = validate_extraction(invalid).unwrap_err();

        for code in [
            "confidence_out_of_range",
            "salary_range_inverted",
            "hours_per_day_out_of_range",
            "rest_days_per_month_out_of_range",
            "experience_years_out_of_range",
        ] {
            assert!(issues.iter().any(|issue| issue.code == code), "missing {code}");
        }
    }

    #[test]
    fn validation_uses_unicode_character_limits_and_collection_limits() {
        let invalid = TalentExtraction {
            title: Some("?".repeat(121)),
            scene: Some("?".repeat(251)),
            roles: vec!["?".repeat(51); 26],
            skills: vec!["?".repeat(51); 26],
            ..valid_extraction()
        };
        let issues = validate_extraction(invalid).unwrap_err();

        for code in [
            "title_too_long",
            "scene_too_long",
            "roles_too_many",
            "roles_item_too_long",
            "skills_too_many",
            "skills_item_too_long",
        ] {
            assert!(issues.iter().any(|issue| issue.code == code), "missing {code}");
        }
    }

    #[test]
    fn validation_leaves_html_and_script_text_as_plain_strings() {
        let extraction = TalentExtraction {
            title: Some(" <script>alert(1)</script> ".to_owned()),
            skills: vec!["<b>Rust</b>".to_owned()],
            ..valid_extraction()
        };
        let validated = validate_extraction(extraction).unwrap();

        assert_eq!(validated.safe.title.as_deref(), Some("<script>alert(1)</script>"));
        assert_eq!(validated.safe.skills, ["<b>Rust</b>"]);
    }

    #[test]
    fn model_validation_recursively_rejects_private_data_and_truth_claims() {
        let source = "PRIVATE-RAW-MARKER-42 招聘厨师，电话：13800138000，微信：candidate_2026";
        let unsafe_values = [
            "13800138000",
            "微信：candidate_2026",
            "PRIVATE-RAW-MARKER-42",
            "该岗位已认证",
            "verified employer",
        ];

        for value in unsafe_values {
            let mut extraction = valid_extraction();
            extraction.title = Some(value.to_owned());

            let issues = validate_model_extraction(extraction, source).unwrap_err();

            assert!(
                issues.iter().any(|issue| issue.code == "unsafe_model_string"),
                "value should be rejected: {value}"
            );
        }
    }

    #[test]
    fn model_validation_rejects_contact_values_repeated_outside_contacts() {
        let mut extraction = valid_extraction();
        extraction.contacts[0].value = "fictional_wechat_2026".to_owned();
        extraction.skills.push("fictional_wechat_2026".to_owned());

        let issues = validate_model_extraction(extraction, "synthetic source").unwrap_err();

        assert!(issues.iter().any(|issue| issue.code == "unsafe_model_string"));
    }

    #[test]
    fn model_risk_allowlist_is_versioned_and_replaces_model_owned_details() {
        let mut extraction = valid_extraction();
        extraction.risk_candidates.push(RiskReason {
            code: " fee_solicitation ".to_owned(),
            explanation: "13800138000 model-controlled explanation".to_owned(),
            evidence: Some("PRIVATE-RAW-MARKER-42".to_owned()),
            hard_block: false,
        });

        let validated = validate_model_extraction(extraction, "PRIVATE-RAW-MARKER-42").unwrap();
        let risk = &validated.safe.risk_candidates[0];

        assert_eq!(MODEL_RISK_ALLOWLIST_VERSION, "talent-model-risk-v1");
        assert_eq!(risk.code, "fee_solicitation");
        assert_eq!(
            risk.explanation,
            "Model identified a fee-solicitation risk category."
        );
        assert!(risk.evidence.is_none());
        assert!(risk.hard_block);
    }

    #[test]
    fn model_validation_rejects_risk_codes_outside_the_server_allowlist() {
        let mut extraction = valid_extraction();
        extraction.risk_candidates.push(RiskReason {
            code: "provider_invented_category".to_owned(),
            explanation: "provider controlled".to_owned(),
            evidence: None,
            hard_block: true,
        });

        let issues = validate_model_extraction(extraction, "synthetic source").unwrap_err();

        assert!(issues.iter().any(|issue| issue.code == "unsupported_model_risk"));
    }
}
