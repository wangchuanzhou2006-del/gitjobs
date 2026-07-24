use std::collections::HashSet;

use uuid::Uuid;

use crate::{
    db::talent::TalentDuplicateCandidate,
    talent::{DuplicateCandidate, TalentExtraction, ValidatedExtraction},
};

pub(crate) const DUPLICATE_HIGH_THRESHOLD: f32 = 0.85;
pub(crate) const DUPLICATE_MEDIUM_THRESHOLD: f32 = 0.60;

#[derive(Clone, PartialEq)]
pub(crate) struct DuplicateProbe {
    pub extraction: ValidatedExtraction,
    pub source_id: Uuid,
    normalized_text: String,
}

impl DuplicateProbe {
    pub(crate) fn new(extraction: ValidatedExtraction, source_id: Uuid) -> Self {
        let normalized_text = normalized_text_for_extraction(&extraction.safe);
        Self {
            extraction,
            source_id,
            normalized_text,
        }
    }
}

impl std::fmt::Debug for DuplicateProbe {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DuplicateProbe")
            .field("extraction", &"[REDACTED SAFE EXTRACTION]")
            .field("source_id", &self.source_id)
            .field("normalized_text", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DuplicateService;

impl DuplicateService {
    pub(crate) fn evaluate(
        &self,
        probe: DuplicateProbe,
        candidates: Vec<TalentDuplicateCandidate>,
    ) -> Vec<DuplicateCandidate> {
        let mut evaluated = candidates
            .into_iter()
            .map(|candidate| evaluate_candidate(&probe, candidate))
            .collect::<Vec<_>>();
        evaluated.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| left.candidate_item_id.cmp(&right.candidate_item_id))
                .then_with(|| left.candidate_record_id.cmp(&right.candidate_record_id))
        });
        evaluated
    }
}

fn evaluate_candidate(probe: &DuplicateProbe, candidate: TalentDuplicateCandidate) -> DuplicateCandidate {
    let exact_reason = candidate
        .reasons
        .iter()
        .find(|reason| matches!(reason.as_str(), "exact_contact_hash" | "exact_fingerprint"))
        .cloned();
    let (score, reasons) = if let Some(reason) = exact_reason {
        (1.0, vec![reason])
    } else {
        structured_score(probe, &candidate)
    };

    DuplicateCandidate {
        candidate_item_id: candidate.candidate_item_id,
        candidate_record_id: candidate.candidate_record_id,
        score,
        reasons,
        high_similarity: score >= DUPLICATE_HIGH_THRESHOLD,
    }
}

fn structured_score(probe: &DuplicateProbe, candidate: &TalentDuplicateCandidate) -> (f32, Vec<String>) {
    let mut score = 0.0_f32;
    let mut reasons = Vec::new();
    if roles_match(&probe.extraction.safe.roles, &candidate.roles) {
        score += 0.35;
        reasons.push("role_match".to_owned());
    }
    if location_matches(&probe.extraction.safe, candidate) {
        score += 0.25;
        reasons.push("location_match".to_owned());
    }
    if salary_overlaps(&probe.extraction.safe, candidate) {
        score += 0.20;
        reasons.push("salary_overlap".to_owned());
    }
    if candidate.source_id == Some(probe.source_id) {
        score += 0.10;
        reasons.push("same_source".to_owned());
    }
    let text_similarity = text_similarity(&probe.normalized_text, &candidate.normalized_text);
    if text_similarity > 0.0 {
        score += 0.10 * text_similarity;
        reasons.push("text_similarity".to_owned());
    }
    (score.min(1.0), reasons)
}

fn roles_match(left: &[String], right: &[String]) -> bool {
    let right = right.iter().map(|value| normalize(value)).collect::<HashSet<_>>();
    left.iter()
        .map(|value| normalize(value))
        .any(|value| !value.is_empty() && right.contains(&value))
}

fn location_matches(left: &TalentExtraction, right: &TalentDuplicateCandidate) -> bool {
    optional_matches(left.city.as_deref(), right.city.as_deref())
        || optional_matches(left.district.as_deref(), right.district.as_deref())
}

fn optional_matches(left: Option<&str>, right: Option<&str>) -> bool {
    left.zip(right).is_some_and(|(left, right)| {
        let left = normalize(left);
        !left.is_empty() && left == normalize(right)
    })
}

fn salary_overlaps(left: &TalentExtraction, right: &TalentDuplicateCandidate) -> bool {
    let Some((left_minimum, left_maximum)) = left.salary_min.zip(left.salary_max) else {
        return false;
    };
    let Some((right_minimum, right_maximum)) = right.salary_min.zip(right.salary_max) else {
        return false;
    };
    left_minimum <= right_maximum && right_minimum <= left_maximum
}

fn text_similarity(left: &str, right: &str) -> f32 {
    let left = tokens(left);
    let right = tokens(right);
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let intersection = left.intersection(&right).count();
    (2 * intersection) as f32 / (left.len() + right.len()) as f32
}

fn tokens(value: &str) -> HashSet<String> {
    value
        .split_whitespace()
        .map(normalize)
        .filter(|token| !token.is_empty())
        .collect()
}

fn normalize(value: &str) -> String {
    value.trim().to_lowercase()
}

fn normalized_text_for_extraction(extraction: &TalentExtraction) -> String {
    extraction
        .title
        .iter()
        .chain(extraction.roles.iter())
        .chain(extraction.city.iter())
        .chain(extraction.district.iter())
        .map(|value| normalize(value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::{
        db::talent::TalentDuplicateCandidate,
        talent::{TalentExtraction, TalentKind, ValidatedExtraction},
    };

    use super::{DuplicateProbe, DuplicateService};

    fn validated(extraction: TalentExtraction) -> ValidatedExtraction {
        ValidatedExtraction {
            safe: extraction,
            contacts: Vec::new(),
            issues: Vec::new(),
        }
    }

    fn extraction(role: &str, city: &str, district: &str, salary: Option<(u32, u32)>) -> TalentExtraction {
        TalentExtraction {
            kind: TalentKind::Hiring,
            roles: vec![role.to_owned()],
            city: Some(city.to_owned()),
            district: Some(district.to_owned()),
            salary_min: salary.map(|value| value.0),
            salary_max: salary.map(|value| value.1),
            ..TalentExtraction::default()
        }
    }

    fn repository_candidate(
        reasons: Vec<&str>,
        extraction: TalentExtraction,
        source_id: Uuid,
        normalized_text: &str,
    ) -> TalentDuplicateCandidate {
        TalentDuplicateCandidate {
            candidate_item_id: Some(Uuid::from_u128(1)),
            candidate_record_id: None,
            kind: TalentKind::Hiring,
            status: "needs_review".to_owned(),
            title: Some("private candidate title".to_owned()),
            roles: extraction.roles,
            city: extraction.city,
            district: extraction.district,
            salary_min: extraction.salary_min,
            salary_max: extraction.salary_max,
            source_id: Some(source_id),
            normalized_text: normalized_text.to_owned(),
            score: 0.99,
            reasons: reasons.into_iter().map(str::to_owned).collect(),
            high_similarity: false,
        }
    }

    fn record(
        reasons: Vec<&str>,
        extraction: TalentExtraction,
        source_id: Uuid,
        normalized_text: &str,
    ) -> TalentDuplicateCandidate {
        repository_candidate(reasons, extraction, source_id, normalized_text)
    }

    #[test]
    fn exact_contact_and_fingerprint_markers_always_score_one() {
        let source_id = Uuid::from_u128(8);
        let probe = DuplicateProbe::new(
            validated(extraction("chef", "alpha city", "north", Some((6_000, 8_000)))),
            source_id,
        );
        let candidates = vec![
            record(
                vec!["exact_contact_hash"],
                extraction("waiter", "beta city", "south", None),
                Uuid::from_u128(9),
                "unrelated",
            ),
            record(
                vec!["exact_fingerprint"],
                extraction("barista", "gamma city", "west", None),
                Uuid::from_u128(10),
                "unrelated again",
            ),
        ];

        let matches = DuplicateService::default().evaluate(probe, candidates);

        assert_eq!(matches.len(), 2);
        assert!(matches.iter().all(|candidate| candidate.score == 1.0));
        assert!(matches.iter().all(|candidate| candidate.high_similarity));
    }

    #[test]
    fn structured_weights_reach_medium_at_point_six() {
        let source_id = Uuid::from_u128(8);
        let probe = DuplicateProbe::new(
            validated(extraction("Chef ", "Alpha City", "North", Some((6_000, 8_000)))),
            source_id,
        );
        let candidate = record(
            vec!["model_suggested_similarity"],
            extraction("chef", "alpha city", "other", Some((20_000, 30_000))),
            Uuid::from_u128(9),
            "unrelated",
        );

        let matches = DuplicateService::default().evaluate(probe, vec![candidate]);

        assert_eq!(matches.len(), 1);
        assert!((matches[0].score - 0.60).abs() < f32::EPSILON);
        assert!(!matches[0].high_similarity);
        assert_eq!(matches[0].reasons, vec!["role_match", "location_match"]);
    }

    #[test]
    fn high_threshold_is_inclusive_at_point_eight_five() {
        let source_id = Uuid::from_u128(8);
        let mut probe_extraction = extraction("chef", "east", "north", Some((6_000, 8_000)));
        probe_extraction.title = Some("alpha".to_owned());
        let probe = DuplicateProbe::new(validated(probe_extraction), source_id);
        let mut candidate_extraction = extraction("CHEF", "west", "north", Some((8_000, 9_000)));
        candidate_extraction.title = Some("beta".to_owned());
        let candidate = record(
            vec![],
            candidate_extraction,
            Uuid::from_u128(9),
            "beta CHEF west north",
        );

        let matches = DuplicateService::default().evaluate(probe, vec![candidate]);

        assert!((matches[0].score - 0.85).abs() < f32::EPSILON);
        assert!(matches[0].high_similarity);
    }

    #[test]
    fn salary_source_and_text_weights_are_deterministic_and_repository_score_is_ignored() {
        let source_id = Uuid::from_u128(8);
        let mut probe_extraction = extraction("chef", "alpha", "north", Some((6_000, 8_000)));
        probe_extraction.title = Some("same normalized text".to_owned());
        let probe = DuplicateProbe::new(validated(probe_extraction), source_id);
        let candidate = record(
            vec!["ai_score_0_99"],
            extraction("waiter", "beta", "south", Some((7_000, 10_000))),
            source_id,
            "same normalized text waiter beta south",
        );

        let matches = DuplicateService::default().evaluate(probe, vec![candidate]);

        assert!((matches[0].score - 0.35).abs() < f32::EPSILON);
        assert_eq!(
            matches[0].reasons,
            vec!["salary_overlap", "same_source", "text_similarity"]
        );
        assert!(!matches[0].high_similarity);
    }

    #[test]
    fn repository_rows_are_directly_consumable_without_private_source_material() {
        let source_id = Uuid::from_u128(8);
        let probe = DuplicateProbe::new(
            validated(extraction("chef", "alpha city", "north", Some((6_000, 8_000)))),
            source_id,
        );
        let row = repository_candidate(
            vec![],
            extraction("chef", "alpha city", "other", Some((20_000, 30_000))),
            Uuid::from_u128(9),
            "private candidate title chef alpha city other",
        );

        let matches = DuplicateService::default().evaluate(probe, vec![row]);

        assert_eq!(matches.len(), 1);
        assert_eq!(
            matches[0].reasons,
            vec!["role_match", "location_match", "text_similarity"]
        );
        assert!(matches[0].score >= 0.60);
        assert!(matches[0].score < 0.85);
    }
}
