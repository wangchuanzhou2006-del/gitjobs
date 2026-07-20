use thiserror::Error;

use super::model::TextFragment;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SplitLimits {
    pub max_raw_bytes: usize,
    pub max_fragments: usize,
    pub max_fragment_bytes: usize,
}

impl Default for SplitLimits {
    fn default() -> Self {
        Self {
            max_raw_bytes: 200_000,
            max_fragments: 500,
            max_fragment_bytes: 8_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum SplitError {
    #[error("raw input is empty")]
    EmptyInput,
    #[error("raw input has {actual_bytes} bytes, exceeding the {max_raw_bytes}-byte limit")]
    RawInputTooLarge {
        actual_bytes: usize,
        max_raw_bytes: usize,
    },
    #[error("split produced {actual_fragments} fragments, exceeding the {max_fragments}-fragment limit")]
    TooManyFragments {
        actual_fragments: usize,
        max_fragments: usize,
    },
    #[error("fragment {position} has {actual_bytes} bytes, exceeding the {max_fragment_bytes}-byte limit")]
    FragmentTooLarge {
        position: usize,
        actual_bytes: usize,
        max_fragment_bytes: usize,
    },
}

#[derive(Debug)]
struct PendingFragment {
    start_byte: usize,
    end_byte: usize,
    lines: Vec<String>,
}

impl PendingFragment {
    fn new(start_byte: usize) -> Self {
        Self {
            start_byte,
            end_byte: start_byte,
            lines: Vec::new(),
        }
    }

    fn push_line(&mut self, line: &Line<'_>) {
        self.lines.push(line.text.to_owned());
        if !line.is_blank() {
            self.end_byte = line.content_end;
        }
    }

    fn into_fragment(
        mut self,
        position: usize,
        limits: SplitLimits,
    ) -> Result<Option<TextFragment>, SplitError> {
        while self.lines.last().is_some_and(|line| line.trim().is_empty()) {
            self.lines.pop();
        }
        let text = self.lines.join("\n");
        if text.trim().is_empty() {
            return Ok(None);
        }
        let actual_bytes = text.len();
        if actual_bytes > limits.max_fragment_bytes {
            return Err(SplitError::FragmentTooLarge {
                position,
                actual_bytes,
                max_fragment_bytes: limits.max_fragment_bytes,
            });
        }

        Ok(Some(TextFragment {
            position,
            start_byte: self.start_byte,
            end_byte: self.end_byte,
            text,
        }))
    }
}

#[derive(Debug)]
struct Line<'a> {
    start: usize,
    content_end: usize,
    text: &'a str,
}

impl Line<'_> {
    fn is_blank(&self) -> bool {
        self.text.trim().is_empty()
    }
}

/// Splits copied group-chat text into ordered fragments without deduplicating content.
pub(crate) fn split_messages(raw: &str, limits: SplitLimits) -> Result<Vec<TextFragment>, SplitError> {
    if raw.is_empty() {
        return Err(SplitError::EmptyInput);
    }
    if raw.len() > limits.max_raw_bytes {
        return Err(SplitError::RawInputTooLarge {
            actual_bytes: raw.len(),
            max_raw_bytes: limits.max_raw_bytes,
        });
    }

    let mut fragments = Vec::new();
    let mut pending = None;
    let mut blank_lines = 0;

    for line in lines(raw) {
        if is_group_header(line.text) {
            finish_fragment(&mut pending, &mut fragments, limits)?;
            pending = Some(PendingFragment::new(line.start));
            blank_lines = 0;
            continue;
        }

        if line.is_blank() {
            let Some(fragment) = pending.as_mut() else {
                continue;
            };
            blank_lines += 1;
            if blank_lines >= 2 {
                finish_fragment(&mut pending, &mut fragments, limits)?;
            } else {
                fragment.push_line(&line);
            }
            continue;
        }

        blank_lines = 0;
        pending
            .get_or_insert_with(|| PendingFragment::new(line.start))
            .push_line(&line);
    }

    finish_fragment(&mut pending, &mut fragments, limits)?;
    Ok(fragments)
}

fn finish_fragment(
    pending: &mut Option<PendingFragment>,
    fragments: &mut Vec<TextFragment>,
    limits: SplitLimits,
) -> Result<(), SplitError> {
    let Some(fragment) = pending.take() else {
        return Ok(());
    };

    let position = fragments.len();
    let Some(fragment) = fragment.into_fragment(position, limits)? else {
        return Ok(());
    };
    fragments.push(fragment);
    if fragments.len() > limits.max_fragments {
        return Err(SplitError::TooManyFragments {
            actual_fragments: fragments.len(),
            max_fragments: limits.max_fragments,
        });
    }
    Ok(())
}

fn lines(raw: &str) -> impl Iterator<Item = Line<'_>> {
    let mut start = 0;
    std::iter::from_fn(move || {
        if start >= raw.len() {
            return None;
        }

        let remainder = &raw[start..];
        let newline = remainder.find('\n');
        let line_end = newline.map_or(raw.len(), |offset| start + offset);
        let content_end = if raw.as_bytes().get(line_end.wrapping_sub(1)) == Some(&b'\r') {
            line_end - 1
        } else {
            line_end
        };
        let line = Line {
            start,
            content_end,
            text: &raw[start..content_end],
        };
        start = newline.map_or(raw.len(), |_| line_end + 1);
        Some(line)
    })
}

fn is_group_header(line: &str) -> bool {
    let line = line.trim();
    if line.starts_with('>') || line.starts_with("[引用]") {
        return false;
    }

    let Some((prefix, timestamp)) = split_last_whitespace(line) else {
        return false;
    };
    if !is_time(timestamp) {
        return false;
    }

    let (nickname, is_dated_header) = match split_last_whitespace(prefix) {
        Some((before_date, date)) if before_date.trim_end().ends_with(':') => {
            let nickname = before_date.trim_end().trim_end_matches(':').trim();
            if !is_date(date) {
                return false;
            }
            (nickname, true)
        }
        _ => (prefix.trim(), false),
    };

    !nickname.is_empty() && nickname.chars().count() <= 80 && (is_dated_header || !nickname.ends_with(':'))
}

fn split_last_whitespace(value: &str) -> Option<(&str, &str)> {
    let (index, whitespace) = value.char_indices().rev().find(|(_, ch)| ch.is_whitespace())?;
    let before = value[..index].trim_end();
    let after = value[index + whitespace.len_utf8()..].trim();
    (!before.is_empty() && !after.is_empty()).then_some((before, after))
}

fn is_time(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 8
        && bytes[2] == b':'
        && bytes[5] == b':'
        && decimal(&bytes[0..2]).is_some_and(|hour| hour <= 23)
        && decimal(&bytes[3..5]).is_some_and(|minute| minute <= 59)
        && decimal(&bytes[6..8]).is_some_and(|second| second <= 59)
}

fn is_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    let (month, day) = match bytes.len() {
        5 if matches!(bytes[2], b'-' | b'/') => (decimal(&bytes[0..2]), decimal(&bytes[3..5])),
        10 if bytes[0..4].iter().all(|byte| byte.is_ascii_digit())
            && ((bytes[4] == b'-' && bytes[7] == b'-') || (bytes[4] == b'/' && bytes[7] == b'/')) =>
        {
            (decimal(&bytes[5..7]), decimal(&bytes[8..10]))
        }
        _ => return false,
    };

    month.is_some_and(|month| (1..=12).contains(&month)) && day.is_some_and(|day| (1..=31).contains(&day))
}

fn decimal(bytes: &[u8]) -> Option<u8> {
    bytes.iter().try_fold(0_u8, |value, byte| {
        byte.is_ascii_digit().then_some(value * 10 + (byte - b'0'))
    })
}
