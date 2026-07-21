#[allow(dead_code)]
pub(crate) mod model;

pub(crate) mod crypto;

pub(crate) mod ai;

pub(crate) mod validation;

pub(crate) mod splitter;

#[allow(unused_imports)]
pub(crate) use ai::*;
#[allow(unused_imports)]
pub(crate) use crypto::*;
#[allow(unused_imports)]
pub(crate) use model::*;
pub(crate) use splitter::*;
#[allow(unused_imports)]
#[allow(unused_imports)]
pub(crate) use validation::*;

#[cfg(test)]
mod tests {
    use super::{
        ContactCandidate, ContactType, DuplicateCandidate, ImportItemStatus, RiskReason, SplitError,
        SplitLimits, TalentExtraction, TextFragment, split_messages,
    };

    #[test]
    fn import_item_transitions_follow_the_review_workflow() {
        assert!(ImportItemStatus::Queued.can_transition_to(ImportItemStatus::Extracting));
        assert!(ImportItemStatus::Extracting.can_transition_to(ImportItemStatus::NeedsReview));
        assert!(ImportItemStatus::NeedsReview.can_transition_to(ImportItemStatus::Published));
        assert!(ImportItemStatus::NeedsReview.can_transition_to(ImportItemStatus::Rejected));
        assert!(ImportItemStatus::Rejected.can_transition_to(ImportItemStatus::Extracting));
        assert!(!ImportItemStatus::Published.can_transition_to(ImportItemStatus::Extracting));
    }

    #[test]
    fn extraction_uses_camel_case_at_the_browser_boundary() {
        let value = serde_json::to_value(TalentExtraction::default()).unwrap();

        assert!(value.get("salaryMin").is_some());
        assert!(value.get("salary_min").is_none());
        assert!(
            serde_json::from_value::<TalentExtraction>(serde_json::json!({
                "kind": "uncertain",
                "confidence": 0.0,
                "roles": [],
                "skills": [],
                "contacts": [],
                "missingFields": [],
                "riskCandidates": [],
                "unexpected": true,
            }))
            .is_err()
        );
    }

    #[test]
    fn splits_nickname_timestamp_headers_and_continuations() {
        const SAMPLE: &str = "虚构甲 10:01:02\n招聘切配，测试市测试区，月薪6000至8000\n合成联络：candidate_alpha_01\n\n虚构乙 10:03:00\n求职炒锅，期望测试市\n合成联络：candidate_beta_02";

        let fragments = split_messages(SAMPLE, SplitLimits::default()).unwrap();

        assert_eq!(fragments.len(), 2);
        assert_eq!(fragments[0].position, 0);
        assert_eq!(fragments[0].start_byte, 0);
        assert_eq!(
            fragments[0].text,
            "招聘切配，测试市测试区，月薪6000至8000\n合成联络：candidate_alpha_01"
        );
        assert_eq!(
            fragments[1].text,
            "求职炒锅，期望测试市\n合成联络：candidate_beta_02"
        );
    }

    #[test]
    fn recognizes_dated_headers_and_normalizes_crlf_without_changing_offsets() {
        let raw = "虚构丙: 07-13 19:39:54\r\n第一行\r\n第二行\r\n\r\n虚构丁: 2026/07/14 00:00:01\r\n第三行";

        let fragments = split_messages(raw, SplitLimits::default()).unwrap();

        assert_eq!(fragments.len(), 2);
        assert_eq!(fragments[0].text, "第一行\n第二行");
        assert_eq!(fragments[0].start_byte, 0);
        assert_eq!(fragments[0].end_byte, raw.find("\r\n\r\n").unwrap());
        assert_eq!(fragments[1].start_byte, raw.find("虚构丁").unwrap());
        assert_eq!(fragments[1].end_byte, raw.len());
    }

    #[test]
    fn double_blank_boundaries_keep_unrelated_text_as_separate_fragments() {
        let raw = "无关公告\n仍然保留\n\n\n另一段无关文本";

        let fragments = split_messages(raw, SplitLimits::default()).unwrap();

        assert_eq!(fragments.len(), 2);
        assert_eq!(fragments[0].text, "无关公告\n仍然保留");
        assert_eq!(fragments[1].text, "另一段无关文本");
    }

    #[test]
    fn quoted_headers_remain_context_and_do_not_start_a_fragment() {
        let raw = "虚构甲 10:01:02\n正文\n> 虚构乙 10:02:03\n> 引用内容\n[引用] 虚构丙: 07-13 10:03:04\n虚构丁 10:04:05\n新正文";

        let fragments = split_messages(raw, SplitLimits::default()).unwrap();

        assert_eq!(fragments.len(), 2);
        assert_eq!(
            fragments[0].text,
            "正文\n> 虚构乙 10:02:03\n> 引用内容\n[引用] 虚构丙: 07-13 10:03:04"
        );
        assert_eq!(fragments[1].text, "新正文");
    }

    #[test]
    fn preserves_independently_posted_identical_messages() {
        let raw = "虚构甲 10:01:02\n同一条合成消息\n虚构乙 10:02:02\n同一条合成消息";

        let fragments = split_messages(raw, SplitLimits::default()).unwrap();

        assert_eq!(fragments.len(), 2);
        assert_eq!(fragments[0].text, fragments[1].text);
        assert_ne!(fragments[0].start_byte, fragments[1].start_byte);
    }

    #[test]
    fn rejects_empty_and_oversized_raw_input() {
        assert_eq!(
            split_messages("", SplitLimits::default()),
            Err(SplitError::EmptyInput)
        );

        let limits = SplitLimits {
            max_raw_bytes: 3,
            ..SplitLimits::default()
        };
        assert_eq!(
            split_messages("四字文本", limits),
            Err(SplitError::RawInputTooLarge {
                actual_bytes: 12,
                max_raw_bytes: 3,
            })
        );
    }

    #[test]
    fn rejects_more_than_the_fragment_limit() {
        let raw = (0..501)
            .map(|index| format!("虚构{index} 10:00:00\n消息{index}"))
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(
            split_messages(&raw, SplitLimits::default()),
            Err(SplitError::TooManyFragments {
                actual_fragments: 501,
                max_fragments: 500,
            })
        );
    }

    #[test]
    fn rejects_an_oversized_normalized_fragment() {
        let raw = format!("虚构甲 10:00:00\n{}", "合".repeat(2_667));
        let error = split_messages(&raw, SplitLimits::default()).unwrap_err();

        assert_eq!(
            error,
            SplitError::FragmentTooLarge {
                position: 0,
                actual_bytes: 8_001,
                max_fragment_bytes: 8_000,
            }
        );
    }
    #[test]
    fn malformed_dated_headers_remain_message_text() {
        let raw = "虚构甲: abcd-07-13 10:00:00\n正文";
        let fragments = split_messages(raw, SplitLimits::default()).unwrap();

        assert_eq!(fragments.len(), 1);
        assert_eq!(fragments[0].text, raw);
    }

    #[test]
    fn auxiliary_domain_structs_serialize_rust_field_names_as_snake_case() {
        let contact = serde_json::to_value(ContactCandidate {
            contact_type: ContactType::Phone,
            value: "reserved_contact_01".to_owned(),
        })
        .unwrap();
        let risk = serde_json::to_value(RiskReason {
            code: "synthetic_risk".to_owned(),
            explanation: "synthetic explanation".to_owned(),
            evidence: None,
            hard_block: true,
        })
        .unwrap();
        let duplicate = serde_json::to_value(DuplicateCandidate {
            candidate_item_id: None,
            candidate_record_id: None,
            score: 0.5,
            reasons: vec!["synthetic reason".to_owned()],
            high_similarity: true,
        })
        .unwrap();
        let fragment = serde_json::to_value(TextFragment {
            position: 0,
            start_byte: 1,
            end_byte: 2,
            text: "synthetic text".to_owned(),
        })
        .unwrap();

        assert!(contact.get("contact_type").is_some());
        assert!(contact.get("contactType").is_none());
        assert!(risk.get("hard_block").is_some());
        assert!(risk.get("hardBlock").is_none());
        assert!(duplicate.get("candidate_item_id").is_some());
        assert!(duplicate.get("candidateItemId").is_none());
        assert!(fragment.get("start_byte").is_some());
        assert!(fragment.get("startByte").is_none());
    }

    #[test]
    fn leading_blank_runs_do_not_emit_empty_fragments() {
        let fragments = split_messages("\n\n\n正文", SplitLimits::default()).unwrap();

        assert_eq!(fragments.len(), 1);
        assert_eq!(fragments[0].text, "正文");
    }

    #[test]
    fn long_interior_blank_runs_terminate_a_fragment_once() {
        let fragments = split_messages("前文\n\n\n\n后文", SplitLimits::default()).unwrap();

        assert_eq!(fragments.len(), 2);
        assert_eq!(fragments[0].text, "前文");
        assert_eq!(fragments[1].text, "后文");
    }

    #[test]
    fn trailing_blank_runs_do_not_emit_empty_fragments() {
        let fragments = split_messages("正文\n\n\n", SplitLimits::default()).unwrap();

        assert_eq!(fragments.len(), 1);
        assert_eq!(fragments[0].text, "正文");
    }

    #[test]
    fn headers_accept_only_the_confirmed_date_and_timestamp_forms() {
        for header in [
            "虚构甲: 07/13 10:00:00".to_owned(),
            "虚构乙: 2026-07-13 10:00:00".to_owned(),
            "虚构丙: 2026/07/13 10:00:00".to_owned(),
            format!("{} 10:00:00", "虚".repeat(80)),
        ] {
            let raw = format!("{header}\n正文");
            let fragments = split_messages(&raw, SplitLimits::default()).unwrap();
            assert_eq!(fragments.len(), 1, "{header}");
            assert_eq!(fragments[0].text, "正文", "{header}");
        }

        for header in [
            "虚构甲: 2026-07/13 10:00:00".to_owned(),
            "虚构乙: 2026/07-13 10:00:00".to_owned(),
            "虚构丙 24:00:00".to_owned(),
            "虚构丁 10:60:00".to_owned(),
            "虚构戊 10:00:60".to_owned(),
            "虚构己: 00-13 10:00:00".to_owned(),
            "虚构庚: 13-13 10:00:00".to_owned(),
            "虚构辛: 07-00 10:00:00".to_owned(),
            "虚构壬: 07-32 10:00:00".to_owned(),
            format!("{} 10:00:00", "虚".repeat(81)),
            "虚构癸 [10:00:00]".to_owned(),
            "虚构子 10:00:00 正文".to_owned(),
        ] {
            let raw = format!("{header}\n正文");
            let fragments = split_messages(&raw, SplitLimits::default()).unwrap();
            assert_eq!(fragments.len(), 1, "{header}");
            assert_eq!(fragments[0].text, raw, "{header}");
        }
    }
}
