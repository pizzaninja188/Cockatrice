//! Sub-line Oracle selectors for printed text that is only part of a line.
//!
//! The grammar runs only in Rust, at refresh/check time and in tests. Clients receive the
//! resolved Unicode-scalar offsets, never the selector, so quote/sentence/keyword parsing has one
//! implementation. Parenthesized reminder text is never selectable.

use serde::{Deserialize, Serialize};
use std::fmt;

/// One-based selector of a printed fragment within one normalized Oracle line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum OracleFragmentPart {
    /// The text inside the nth top-level double-quoted span, quote marks excluded. Trollhide's
    /// granted `"{1}{G}: Regenerate this creature."`.
    Quote(u16),
    /// The nth top-level sentence including its period. Ray of Command's delayed trigger
    /// `When you lose control of the creature, tap it.`
    Sentence(u16),
    /// The nth item of a keyword list: the whole line, or the clause after the last top-level
    /// `has`/`have`. Humpback Whales' `ward {2}` in `Vigilance, ward {2}`.
    Keyword(u16),
}

impl OracleFragmentPart {
    pub fn ordinal(self) -> u16 {
        match self {
            Self::Quote(n) | Self::Sentence(n) | Self::Keyword(n) => n,
        }
    }

    /// Stable table encoding (`quote:1`, `sentence:3`, `keyword:2`).
    pub fn encode(self) -> String {
        let (kind, n) = match self {
            Self::Quote(n) => ("quote", n),
            Self::Sentence(n) => ("sentence", n),
            Self::Keyword(n) => ("keyword", n),
        };
        format!("{kind}:{n}")
    }

    pub fn decode(text: &str) -> Option<Self> {
        let (kind, n) = text.split_once(':')?;
        let n = n.parse::<u16>().ok().filter(|n| *n > 0)?;
        match kind {
            "quote" => Some(Self::Quote(n)),
            "sentence" => Some(Self::Sentence(n)),
            "keyword" => Some(Self::Keyword(n)),
            _ => None,
        }
    }
}

impl fmt::Display for OracleFragmentPart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.encode())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FragmentError {
    /// The line's quote or parenthesis structure, or its keyword clause, cannot be read one way.
    Ambiguous(String),
    /// The structure is clear but has no such fragment.
    Unresolvable(String),
}

impl fmt::Display for FragmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ambiguous(reason) => write!(f, "ambiguous fragment: {reason}"),
            Self::Unresolvable(reason) => write!(f, "unresolvable fragment: {reason}"),
        }
    }
}

/// Character classes of one line: whether a scalar is reminder text (inside top-level
/// parentheses, parentheses included) and the top-level quote spans.
struct LineShape {
    chars: Vec<char>,
    reminder: Vec<bool>,
    in_quote: Vec<bool>,
    quotes: Vec<(usize, usize)>,
}

fn shape(line: &str) -> Result<LineShape, FragmentError> {
    let chars: Vec<char> = line.chars().collect();
    let mut reminder = vec![false; chars.len()];
    let mut in_quote = vec![false; chars.len()];
    let mut quotes = Vec::new();
    let mut depth = 0usize;
    let mut quote_start: Option<usize> = None;
    for (index, ch) in chars.iter().copied().enumerate() {
        if depth > 0 {
            reminder[index] = true;
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            continue;
        }
        if let Some(start) = quote_start {
            if ch == '"' {
                quotes.push((start, index));
                quote_start = None;
            } else {
                in_quote[index] = true;
            }
            continue;
        }
        match ch {
            '"' => quote_start = Some(index + 1),
            '(' => {
                depth = 1;
                reminder[index] = true;
            }
            ')' => return Err(FragmentError::Ambiguous("unbalanced ')'".into())),
            _ => {}
        }
    }
    if depth > 0 {
        return Err(FragmentError::Ambiguous("unbalanced '('".into()));
    }
    if quote_start.is_some() {
        return Err(FragmentError::Ambiguous("unbalanced '\"'".into()));
    }
    Ok(LineShape {
        chars,
        reminder,
        in_quote,
        quotes,
    })
}

impl LineShape {
    fn top_level(&self, index: usize) -> bool {
        !self.reminder[index] && !self.in_quote[index]
    }

    /// Shrink `[start, end)` past whitespace and reminder text at both ends.
    fn trim(&self, mut start: usize, mut end: usize) -> (usize, usize) {
        while start < end && (self.chars[start].is_whitespace() || self.reminder[start]) {
            start += 1;
        }
        while end > start && (self.chars[end - 1].is_whitespace() || self.reminder[end - 1]) {
            end -= 1;
        }
        (start, end)
    }

    fn sentences(&self) -> Vec<(usize, usize)> {
        let len = self.chars.len();
        let boundary_follows =
            |index: usize| index + 1 >= len || self.chars[index + 1].is_whitespace();
        let mut spans = Vec::new();
        let mut start = 0;
        for index in 0..len {
            let ch = self.chars[index];
            let ends_top_level = ch == '.' && self.top_level(index);
            // American-style punctuation puts the period inside the closing quote:
            // `gains "When this creature dies, return it."` ends its sentence at the quote.
            let ends_quoted = ch == '"'
                && !self.reminder[index]
                && index > 0
                && self.in_quote[index - 1]
                && self.chars[index - 1] == '.';
            if (ends_top_level || ends_quoted) && boundary_follows(index) {
                spans.push(self.trim(start, index + 1));
                start = index + 1;
            }
        }
        spans.push(self.trim(start, len));
        spans.retain(|(start, end)| start < end);
        spans
    }

    fn keywords(&self) -> Result<Vec<(usize, usize)>, FragmentError> {
        let len = self.chars.len();
        let mut region_start = 0;
        for index in 0..len {
            for word in ["has ", "have "] {
                let word: Vec<char> = word.chars().collect();
                let fits = index + word.len() <= len
                    && (index == 0 || self.chars[index - 1] == ' ')
                    && (index..index + word.len()).all(|i| self.top_level(i))
                    && self.chars[index..index + word.len()] == word[..];
                if fits {
                    region_start = index + word.len();
                }
            }
        }
        let mut items = Vec::new();
        let mut start = region_start;
        let mut index = region_start;
        while index < len {
            let separator = if self.top_level(index) && self.chars[index] == ',' {
                Some(1)
            } else if self.top_level(index)
                && self.chars[index..].starts_with(&[' ', 'a', 'n', 'd', ' '])
            {
                Some(5)
            } else {
                None
            };
            if let Some(width) = separator {
                items.push(self.trim(start, index));
                start = index + width;
                index += width;
            } else {
                index += 1;
            }
        }
        items.push(self.trim(start, len));
        // The list ends the clause; its trailing period is punctuation, not keyword text.
        for item in &mut items {
            if item.1 > item.0 && self.chars[item.1 - 1] == '.' && self.top_level(item.1 - 1) {
                item.1 -= 1;
            }
        }
        items.retain(|(start, end)| start < end);
        if items.is_empty() {
            return Err(FragmentError::Unresolvable("no keyword list".into()));
        }
        Ok(items)
    }
}

/// Resolve one selector to Unicode-scalar `[start, end)` offsets within `line`.
pub fn resolve_oracle_fragment(
    line: &str,
    part: OracleFragmentPart,
) -> Result<(u32, u32), FragmentError> {
    let ordinal = usize::from(part.ordinal());
    if ordinal == 0 {
        return Err(FragmentError::Unresolvable("ordinals are one-based".into()));
    }
    let shape = shape(line)?;
    let spans = match part {
        OracleFragmentPart::Quote(_) => shape.quotes.clone(),
        OracleFragmentPart::Sentence(_) => shape.sentences(),
        OracleFragmentPart::Keyword(_) => shape.keywords()?,
    };
    let (start, end) = spans.get(ordinal - 1).copied().ok_or_else(|| {
        FragmentError::Unresolvable(format!("{part} of {} available", spans.len()))
    })?;
    if start >= end {
        return Err(FragmentError::Unresolvable(format!("{part} is empty")));
    }
    let offset = |value: usize| u32::try_from(value).expect("Oracle lines are short");
    Ok((offset(start), offset(end)))
}

/// Slice `line` by Unicode-scalar offsets, or `None` when out of range.
pub fn slice_scalars(line: &str, start: u32, end: u32) -> Option<String> {
    let (start, end) = (usize::try_from(start).ok()?, usize::try_from(end).ok()?);
    if start >= end || end > line.chars().count() {
        return None;
    }
    Some(line.chars().skip(start).take(end - start).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use OracleFragmentPart::{Keyword, Quote, Sentence};

    fn text(line: &str, part: OracleFragmentPart) -> Result<String, FragmentError> {
        let (start, end) = resolve_oracle_fragment(line, part)?;
        Ok(slice_scalars(line, start, end).expect("resolved offsets are in range"))
    }

    const TROLLHIDE: &str = "Enchanted creature gets +2/+2 and has \"{1}{G}: Regenerate this creature.\" (The next time this creature would be destroyed this turn, it isn't. Instead tap it, remove all damage from it, and remove it from combat.)";
    const RAY: &str = "Untap target creature an opponent controls and gain control of it until end of turn. That creature gains haste until end of turn. When you lose control of the creature, tap it.";

    #[test]
    fn quote_selects_inside_the_marks_and_ignores_reminder_quotes() {
        assert_eq!(
            text(TROLLHIDE, Quote(1)).unwrap(),
            "{1}{G}: Regenerate this creature."
        );
        let curious = "Enchanted creature gets +1/+1 and has \"Whenever this creature deals combat damage to a player, investigate.\" (Create a Clue token. It's an artifact with \"{2}, Sacrifice this token: Draw a card.\")";
        assert_eq!(
            text(curious, Quote(1)).unwrap(),
            "Whenever this creature deals combat damage to a player, investigate."
        );
        assert!(matches!(
            text(curious, Quote(2)),
            Err(FragmentError::Unresolvable(_))
        ));
        let ichormoon =
            "Planeswalkers you control have \"[0]: Proliferate\" and \"[−12]: Take an extra turn after this one.\"";
        assert_eq!(text(ichormoon, Quote(1)).unwrap(), "[0]: Proliferate");
        assert_eq!(
            text(ichormoon, Quote(2)).unwrap(),
            "[−12]: Take an extra turn after this one."
        );
    }

    #[test]
    fn sentence_ignores_periods_in_quotes_and_reminder_text() {
        assert_eq!(
            text(RAY, Sentence(3)).unwrap(),
            "When you lose control of the creature, tap it."
        );
        let turn = "Target creature gets +3/+0 until end of turn. When it dies this turn, manifest dread. (Look at the top two cards of your library. Put one onto the battlefield face down as a 2/2 creature and the other into your graveyard.)";
        assert_eq!(
            text(turn, Sentence(2)).unwrap(),
            "When it dies this turn, manifest dread."
        );
        assert!(matches!(
            text(turn, Sentence(3)),
            Err(FragmentError::Unresolvable(_))
        ));
        assert_eq!(
            text(TROLLHIDE, Sentence(1)).unwrap(),
            "Enchanted creature gets +2/+2 and has \"{1}{G}: Regenerate this creature.\""
        );
        assert!(matches!(
            text(TROLLHIDE, Sentence(2)),
            Err(FragmentError::Unresolvable(_))
        ));
    }

    #[test]
    fn keyword_selects_list_items_of_the_line_or_granting_clause() {
        let whales = "Vigilance, ward {2} (Whenever this creature becomes the target of a spell or ability an opponent controls, counter it unless that player pays {2}.)";
        assert_eq!(text(whales, Keyword(1)).unwrap(), "Vigilance");
        assert_eq!(text(whales, Keyword(2)).unwrap(), "ward {2}");
        assert_eq!(
            text("Flying, vigilance, ward {2}", Keyword(3)).unwrap(),
            "ward {2}"
        );
        let thorin = "As long as you have an enduring story, artifacts and creatures you control have ward {1}.";
        assert_eq!(text(thorin, Keyword(1)).unwrap(), "ward {1}");
        let wrench =
            "Equipped creature gets +1/+1 and has vigilance and \"{3}, {T}: Tap target creature.\"";
        assert_eq!(text(wrench, Keyword(1)).unwrap(), "vigilance");
    }

    #[test]
    fn unbalanced_structure_is_ambiguous() {
        for line in ["Has \"open", "Has (open", "Has close)"] {
            for part in [Quote(1), Sentence(1), Keyword(1)] {
                assert!(
                    matches!(text(line, part), Err(FragmentError::Ambiguous(_))),
                    "{line} {part}"
                );
            }
        }
    }

    #[test]
    fn offsets_are_unicode_scalars() {
        let line = "Gains \"𝔸 — draw.\"";
        let (start, end) = resolve_oracle_fragment(line, Quote(1)).unwrap();
        assert_eq!((start, end), (7, 16));
        assert_eq!(slice_scalars(line, start, end).unwrap(), "𝔸 — draw.");
        assert_eq!(slice_scalars(line, 7, 99), None);
        assert_eq!(slice_scalars(line, 7, 7), None);
    }

    #[test]
    fn part_encoding_round_trips_and_rejects_zero() {
        for part in [Quote(1), Sentence(3), Keyword(2)] {
            assert_eq!(OracleFragmentPart::decode(&part.encode()), Some(part));
        }
        assert_eq!(OracleFragmentPart::decode("quote:0"), None);
        assert_eq!(OracleFragmentPart::decode("line:1"), None);
        assert!(resolve_oracle_fragment("x", Quote(0)).is_err());
    }
}
