use std::collections::HashSet;

use crate::{
    talent::{ContactCandidate, RiskReason, TalentExtraction},
    validation::{MAX_ITEMS, MAX_LEN_ENTITY_NAME, MAX_LEN_M, MAX_LEN_TAG},
};

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
    use crate::talent::{ContactCandidate, ContactType, TalentExtraction, TalentKind};

    use super::validate_extraction;

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
}
