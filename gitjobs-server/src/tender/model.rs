//! Strict wire contract shared with the internal Python service.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

const VERSION: &str = "tender-analysis.v1";

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum ContractError {
    #[error("invalid tender analysis contract")]
    Invalid,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DocumentChunk {
    pub chunk_id: String,
    pub page_number: u16,
    pub text: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ParsedPage {
    pub page_number: u16,
    pub text: String,
    pub chunks: Vec<DocumentChunk>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ParsedDocument {
    pub page_count: u16,
    pub pages: Vec<ParsedPage>,
}

impl ParsedDocument {
    pub(crate) fn validate(&self) -> Result<(), ContractError> {
        if self.page_count == 0 || self.page_count > 1000 || self.pages.len() != usize::from(self.page_count)
        {
            return Err(ContractError::Invalid);
        }
        let mut seen_chunks = HashSet::new();
        let mut text_bytes = 0_usize;
        for (index, page) in self.pages.iter().enumerate() {
            if usize::from(page.page_number) != index + 1 {
                return Err(ContractError::Invalid);
            }
            text_bytes = text_bytes
                .checked_add(page.text.len())
                .ok_or(ContractError::Invalid)?;
            if text_bytes > 10 * 1024 * 1024 {
                return Err(ContractError::Invalid);
            }
            for chunk in &page.chunks {
                if chunk.text.is_empty()
                    || !seen_chunks.insert(chunk.chunk_id.as_str())
                    || chunk.page_number != page.page_number
                    || chunk_id_page(&chunk.chunk_id) != Some(page.page_number)
                    || !page.text.contains(&chunk.text)
                {
                    return Err(ContractError::Invalid);
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ParseResponse {
    pub status: ParseStatus,
    pub parsed_document: Option<ParsedDocument>,
    pub error_code: Option<ParseErrorCode>,
}

impl ParseResponse {
    pub(crate) fn validate(&self) -> Result<(), ContractError> {
        match (&self.status, &self.parsed_document, &self.error_code) {
            (ParseStatus::Completed, Some(document), None) => document.validate(),
            (ParseStatus::Failed, None, Some(_)) => Ok(()),
            _ => Err(ContractError::Invalid),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ParseStatus {
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ParseErrorCode {
    InvalidPdf,
    EncryptedPdf,
    TooManyPages,
    ExtractedTextTooLarge,
    OcrRequired,
}

#[derive(Clone, Serialize)]
pub(crate) struct AnalyzeRequest<'a> {
    pub job_id: String,
    pub document: &'a ParsedDocument,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Citation {
    pub page_number: u16,
    pub chunk_id: String,
    pub quote: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EvidenceValue {
    pub value: Option<String>,
    pub confidence: f64,
    pub requires_review: bool,
    pub citations: Vec<Citation>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProjectInfo {
    pub project_name: EvidenceValue,
    pub procurement_entity: EvidenceValue,
    pub budget_amount: EvidenceValue,
    pub budget_currency: EvidenceValue,
    pub service_period: EvidenceValue,
    pub bid_deadline: EvidenceValue,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum QualificationKind {
    HardQualification,
    ScoringItem,
    SubmissionMaterial,
    ContractPerformance,
    GeneralNote,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Qualification {
    pub kind: QualificationKind,
    pub requirement: String,
    pub original_text: String,
    pub confidence: f64,
    pub requires_review: bool,
    pub citations: Vec<Citation>,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RiskSeverity {
    Low,
    Medium,
    High,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Risk {
    pub severity: RiskSeverity,
    pub fact: String,
    pub risk_judgment: String,
    pub impact: String,
    pub suggestion: String,
    pub confidence: f64,
    pub requires_review: bool,
    pub citations: Vec<Citation>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RequiredMaterial {
    pub name: String,
    pub description: String,
    pub source_requirement: String,
    pub confidence: f64,
    pub requires_review: bool,
    pub citations: Vec<Citation>,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UnknownReason {
    Missing,
    Conflict,
    LowConfidence,
    Unsupported,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UnknownItem {
    pub field_name: String,
    pub reason: UnknownReason,
    pub detail: String,
    pub citations: Vec<Citation>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PromptVersions {
    pub project_info: String,
    pub qualifications: String,
    pub risks: String,
    pub materials: String,
    pub review: String,
}

impl PromptVersions {
    fn is_v1(&self) -> bool {
        self.project_info == "tender-project-info-v1"
            && self.qualifications == "tender-qualifications-v1"
            && self.risks == "tender-risks-v1"
            && self.materials == "tender-materials-v1"
            && self.review == "tender-review-v1"
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Completeness {
    Complete,
    Partial,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AnalysisMeta {
    pub model_provider: String,
    pub model_name: String,
    pub prompt_versions: PromptVersions,
    pub contract_version: String,
    pub completeness: Completeness,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TenderAnalysisV1 {
    pub schema_version: String,
    pub project_info: ProjectInfo,
    pub qualifications: Vec<Qualification>,
    pub risks: Vec<Risk>,
    pub required_materials: Vec<RequiredMaterial>,
    pub unknowns: Vec<UnknownItem>,
    pub analysis_meta: AnalysisMeta,
}

impl TenderAnalysisV1 {
    pub(crate) fn validate_against(&self, parsed: &ParsedDocument) -> Result<(), ContractError> {
        parsed.validate()?;
        if self.schema_version != VERSION
            || self.analysis_meta.contract_version != VERSION
            || self.qualifications.len() > 200
            || self.risks.len() > 200
            || self.required_materials.len() > 300
            || self.unknowns.len() > 200
            || !self.analysis_meta.prompt_versions.is_v1()
        {
            return Err(ContractError::Invalid);
        }

        let chunks: HashMap<&str, &DocumentChunk> = parsed
            .pages
            .iter()
            .flat_map(|p| &p.chunks)
            .map(|c| (c.chunk_id.as_str(), c))
            .collect();
        let check_citations = |citations: &[Citation], required: bool| -> Result<(), ContractError> {
            if citations.len() > 20 || (required && citations.is_empty()) {
                return Err(ContractError::Invalid);
            }
            for citation in citations {
                let Some(chunk) = chunks.get(citation.chunk_id.as_str()) else {
                    return Err(ContractError::Invalid);
                };
                if citation.page_number == 0
                    || citation.page_number > 1000
                    || citation.page_number != chunk.page_number
                    || chunk_id_page(&citation.chunk_id) != Some(citation.page_number)
                    || citation.quote.is_empty()
                    || citation.quote.chars().count() > 2000
                    || !chunk.text.contains(&citation.quote)
                {
                    return Err(ContractError::Invalid);
                }
            }
            Ok(())
        };
        let check_confidence = |confidence: f64| -> Result<(), ContractError> {
            if confidence.is_finite() && (0.0..=1.0).contains(&confidence) {
                Ok(())
            } else {
                Err(ContractError::Invalid)
            }
        };
        for evidence in [
            &self.project_info.project_name,
            &self.project_info.procurement_entity,
            &self.project_info.budget_amount,
            &self.project_info.budget_currency,
            &self.project_info.service_period,
            &self.project_info.bid_deadline,
        ] {
            check_confidence(evidence.confidence)?;
            if evidence.value.is_none() && !evidence.requires_review {
                return Err(ContractError::Invalid);
            }
            check_citations(&evidence.citations, evidence.value.is_some())?;
        }
        for qualification in &self.qualifications {
            if qualification.requirement.is_empty() || qualification.original_text.is_empty() {
                return Err(ContractError::Invalid);
            }
            check_confidence(qualification.confidence)?;
            check_citations(&qualification.citations, true)?;
        }
        for risk in &self.risks {
            if risk.fact.is_empty() || risk.risk_judgment.is_empty() {
                return Err(ContractError::Invalid);
            }
            check_confidence(risk.confidence)?;
            check_citations(&risk.citations, true)?;
        }
        for material in &self.required_materials {
            if material.name.is_empty() {
                return Err(ContractError::Invalid);
            }
            check_confidence(material.confidence)?;
            check_citations(&material.citations, true)?;
        }
        for unknown in &self.unknowns {
            if unknown.field_name.is_empty() {
                return Err(ContractError::Invalid);
            }
            check_citations(&unknown.citations, false)?;
            if matches!(unknown.reason, UnknownReason::Conflict)
                && unknown
                    .citations
                    .iter()
                    .map(|citation| citation.chunk_id.as_str())
                    .collect::<HashSet<_>>()
                    .len()
                    < 2
            {
                return Err(ContractError::Invalid);
            }
        }
        Ok(())
    }
}

fn chunk_id_page(value: &str) -> Option<u16> {
    let (page, chunk) = value.strip_prefix("page-")?.split_once("-chunk-")?;
    if page.starts_with('0')
        || page.is_empty()
        || chunk.is_empty()
        || !chunk.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    page.parse::<u16>().ok().filter(|p| *p > 0 && *p <= 1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> TenderAnalysisV1 {
        serde_json::from_str(include_str!(
            "../../../contracts/fixtures/tender-analysis.v1.json"
        ))
        .expect("fixture must deserialize")
    }

    fn parsed_fixture() -> ParsedDocument {
        let report = fixture();
        let citations = report
            .project_info
            .project_name
            .citations
            .iter()
            .chain(report.project_info.procurement_entity.citations.iter())
            .chain(report.project_info.budget_amount.citations.iter())
            .chain(report.project_info.budget_currency.citations.iter())
            .chain(report.project_info.service_period.citations.iter())
            .chain(report.project_info.bid_deadline.citations.iter())
            .chain(report.qualifications.iter().flat_map(|v| &v.citations))
            .chain(report.risks.iter().flat_map(|v| &v.citations))
            .chain(report.required_materials.iter().flat_map(|v| &v.citations));
        let mut pages = (1..=6)
            .map(|number| ParsedPage {
                page_number: number,
                text: String::new(),
                chunks: Vec::new(),
            })
            .collect::<Vec<_>>();
        for citation in citations {
            let page = &mut pages[usize::from(citation.page_number - 1)];
            if let Some(chunk) = page.chunks.iter_mut().find(|c| c.chunk_id == citation.chunk_id) {
                chunk.text.push_str(&citation.quote);
            } else {
                page.chunks.push(DocumentChunk {
                    chunk_id: citation.chunk_id.clone(),
                    page_number: citation.page_number,
                    text: citation.quote.clone(),
                });
            }
        }
        for page in &mut pages {
            page.text = page
                .chunks
                .iter()
                .map(|c| c.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
        }
        ParsedDocument { page_count: 6, pages }
    }

    #[test]
    fn shared_fixture_matches_parsed_evidence() {
        assert_eq!(fixture().validate_against(&parsed_fixture()), Ok(()));
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let mut value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../contracts/fixtures/tender-analysis.v1.json"
        ))
        .unwrap();
        value["surprise"] = serde_json::json!(true);
        assert!(serde_json::from_value::<TenderAnalysisV1>(value).is_err());
    }

    #[test]
    fn fabricated_quote_is_rejected() {
        let mut report = fixture();
        report.project_info.project_name.citations[0].quote = "invented".to_owned();
        assert_eq!(
            report.validate_against(&parsed_fixture()),
            Err(ContractError::Invalid)
        );
    }

    #[test]
    fn wrong_page_and_missing_citation_are_rejected() {
        let document = parsed_fixture();
        let mut report = fixture();
        report.project_info.project_name.citations[0].page_number = 2;
        assert_eq!(report.validate_against(&document), Err(ContractError::Invalid));
        report.project_info.project_name.citations.clear();
        assert_eq!(report.validate_against(&document), Err(ContractError::Invalid));
    }
}
