//! Stable authored identity and presentation metadata for card definitions.

use crate::oracle_fragment::{resolve_oracle_fragment, slice_scalars, OracleFragmentPart};
use serde::{Deserialize, Serialize};
use std::fmt;

const RESERVED_IDS: &[&str] = &["fallback", "keyword", "spell"];

fn validate_stable_id(value: &str, kind: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{kind} must not be empty"));
    }
    if RESERVED_IDS.contains(&value) {
        return Err(format!("{kind} '{value}' is reserved"));
    }
    let mut chars = value.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_lowercase())
        || !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        || value.ends_with('_')
        || value.contains("__")
    {
        return Err(format!(
            "{kind} '{value}' must be canonical snake_case beginning with a letter"
        ));
    }
    Ok(())
}

macro_rules! stable_id {
    ($name:ident, $kind:literal) => {
        #[derive(
            Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, String> {
                let value = value.into();
                validate_stable_id(&value, $kind)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub(crate) fn validate(&self) -> Result<(), String> {
                validate_stable_id(&self.0, $kind)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

stable_id!(CardFaceId, "card face id");
stable_id!(AbilityId, "ability id");
stable_id!(ModeId, "mode id");
stable_id!(ChoiceId, "choice id");
stable_id!(ExiledCohortId, "exiled cohort id");
stable_id!(AbilityLinkId, "ability link id");

/// Exact card-face definition identity used as the parent of face-owned programs and modes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CardFaceDefinitionId {
    pub card_id: String,
    pub face_id: CardFaceId,
}

impl CardFaceDefinitionId {
    pub fn new(card_id: impl Into<String>, face_id: CardFaceId) -> Self {
        Self {
            card_id: card_id.into(),
            face_id,
        }
    }
}

/// Exact authored ability slot, independent of display names and flattened live ability indexes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AbilityDefinitionId {
    pub card_id: String,
    pub face_id: CardFaceId,
    pub ability_path: Vec<AbilityId>,
}

/// Exact authored owner of one compiled resolution program.
///
/// The owner includes its complete parent chain: a mode is scoped by its face or ability, and an
/// ability by its card face and full nested ability path. Local mode and ability IDs therefore
/// never become registry-wide keys.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ProgramOwnerId {
    Face(CardFaceDefinitionId),
    FaceMode {
        face: CardFaceDefinitionId,
        mode_id: ModeId,
    },
    Ability(AbilityDefinitionId),
    AbilityMode {
        ability: AbilityDefinitionId,
        mode_id: ModeId,
    },
}

impl ProgramOwnerId {
    pub fn face(card_id: impl Into<String>, face_id: CardFaceId) -> Self {
        Self::Face(CardFaceDefinitionId::new(card_id, face_id))
    }

    pub fn face_mode(card_id: impl Into<String>, face_id: CardFaceId, mode_id: ModeId) -> Self {
        Self::FaceMode {
            face: CardFaceDefinitionId::new(card_id, face_id),
            mode_id,
        }
    }

    pub fn ability(definition: AbilityDefinitionId) -> Self {
        Self::Ability(definition)
    }

    pub fn ability_mode(definition: AbilityDefinitionId, mode_id: ModeId) -> Self {
        Self::AbilityMode {
            ability: definition,
            mode_id,
        }
    }
}

/// Stable identity and non-mechanical presentation metadata around an authored definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentifiedAbility<T> {
    pub ability_id: AbilityId,
    pub presentation: AbilityPresentation,
    pub definition: T,
}

impl<T> IdentifiedAbility<T> {
    pub fn fallback(ability_id: impl Into<String>, definition: T) -> Result<Self, String> {
        Ok(Self {
            ability_id: AbilityId::new(ability_id)?,
            presentation: AbilityPresentation::Fallback,
            definition,
        })
    }

    pub(crate) fn validate_metadata(&self) -> Result<(), String> {
        self.ability_id.validate()?;
        self.presentation.validate()
    }
}

impl<T: PartialEq> PartialEq<T> for IdentifiedAbility<T> {
    fn eq(&self, other: &T) -> bool {
        self.definition == *other
    }
}

/// Non-mechanical instructions for presenting an authored printed spell, ability, mode or cost
/// group. Interactive choice labels use [`ChoicePresentation`] instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbilityPresentation {
    /// One-based lines in the external Oracle text for this exact card face.
    OracleLines(Vec<u16>),
    /// Printed text that is only part of one line: a quoted grant, one sentence or one keyword.
    /// Offsets are resolved at refresh time into the generated fragment table.
    OracleFragment { line: u16, part: OracleFragmentPart },
    /// No exact external mapping exists; callers must use the deterministic fallback.
    Fallback,
}

impl AbilityPresentation {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::OracleLines(lines) => validate_oracle_lines(lines),
            Self::OracleFragment { line, part } => {
                if *line == 0 {
                    return Err("OracleFragment line is one-based".into());
                }
                if part.ordinal() == 0 {
                    return Err("OracleFragment part ordinals are one-based".into());
                }
                Ok(())
            }
            Self::Fallback => Ok(()),
        }
    }

    /// External spans for this presentation. `fragment` supplies the refresh-resolved offsets
    /// of a fragment selector on this face; a missing entry yields no spans (fallback).
    pub fn oracle_spans(
        &self,
        fragment: impl FnOnce(u16, OracleFragmentPart) -> Option<(u32, u32)>,
    ) -> Vec<OracleTextSpan> {
        if self.validate().is_err() {
            return Vec::new();
        }
        match self {
            Self::OracleLines(lines) => whole_line_spans(lines),
            Self::OracleFragment { line, part } => fragment(*line, *part)
                .map(|range| {
                    vec![OracleTextSpan {
                        line: *line,
                        range: Some(range),
                    }]
                })
                .unwrap_or_default(),
            Self::Fallback => Vec::new(),
        }
    }
}

/// Presentation for an interactive choice label: a resolution branch, cast-cost option, search
/// slot, mana restriction or draw-count choice. Its natural label names the specific choice, so
/// generated text is the default and needs no exception.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChoicePresentation {
    /// The engine-generated label for this exact choice.
    #[default]
    Generated,
    /// A choice that really is one or more whole printed lines (a bulleted mode-like branch).
    OracleLines(Vec<u16>),
}

impl ChoicePresentation {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Generated => Ok(()),
            Self::OracleLines(lines) => validate_oracle_lines(lines),
        }
    }

    pub fn is_generated(&self) -> bool {
        matches!(self, Self::Generated)
    }

    pub fn oracle_spans(&self) -> Vec<OracleTextSpan> {
        match self {
            Self::OracleLines(lines) if validate_oracle_lines(lines).is_ok() => {
                whole_line_spans(lines)
            }
            _ => Vec::new(),
        }
    }
}

fn validate_oracle_lines(lines: &[u16]) -> Result<(), String> {
    if lines.is_empty() {
        return Err("OracleLines must contain at least one line".into());
    }
    if lines[0] == 0 {
        return Err("OracleLines indices are one-based".into());
    }
    if lines.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err("OracleLines indices must be unique and ascending".into());
    }
    Ok(())
}

fn whole_line_spans(lines: &[u16]) -> Vec<OracleTextSpan> {
    lines
        .iter()
        .map(|line| OracleTextSpan {
            line: *line,
            range: None,
        })
        .collect()
}

/// One selected piece of a normalized external face: a whole one-based line, or the
/// Unicode-scalar `[start, end)` range of that line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleTextSpan {
    pub line: u16,
    pub range: Option<(u32, u32)>,
}

/// Select spans from normalized lines as one unit: any invalid, unordered or overlapping span
/// selects nothing. Multiple spans join with newlines.
pub fn select_oracle_spans(lines: &[String], spans: &[OracleTextSpan]) -> Option<String> {
    if spans.is_empty() {
        return None;
    }
    let mut previous: Option<(u16, u32)> = None;
    let mut selected = Vec::with_capacity(spans.len());
    for span in spans {
        let text = lines.get(usize::from(span.line).checked_sub(1)?)?;
        let scalars = u32::try_from(text.chars().count()).ok()?;
        let (start, end) = span.range.unwrap_or((0, scalars));
        if let Some((line, last_end)) = previous {
            if span.line < line || (span.line == line && start < last_end) {
                return None;
            }
        }
        previous = Some((span.line, end));
        selected.push(slice_scalars(text, start, end)?);
    }
    Some(selected.join("\n"))
}

/// Normalize one external face's Oracle text into its addressable line sequence.
pub fn external_oracle_lines(text: &str) -> Vec<String> {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Whether an external (Scryfall) record layout is a display-only identity rather than a game
/// card. Tokens, emblems, art-series cards and Jumpstart/Mystery Booster front cards can share a
/// name with a real card (Ajani's Pridemate, Drowned) while carrying different text, so
/// name-keyed lookups must skip them instead of treating the name as ambiguous.
pub fn is_non_game_layout(layout: &str) -> bool {
    matches!(
        layout,
        "token" | "double_faced_token" | "emblem" | "art_series" | "front_card"
    )
}

/// Resolve an all-or-fallback external presentation mapping against one external face text.
pub fn resolve_external_presentation(
    presentation: &AbilityPresentation,
    external_face_text: Option<&str>,
    fallback: impl FnOnce() -> String,
) -> String {
    let Some(text) = external_face_text else {
        return fallback();
    };
    let lines = external_oracle_lines(text);
    let spans = presentation.oracle_spans(|line, part| {
        let text = lines.get(usize::from(line).checked_sub(1)?)?;
        resolve_oracle_fragment(text, part).ok()
    });
    select_oracle_spans(&lines, &spans).unwrap_or_else(fallback)
}

pub fn ability_fallback(face_name: &str, kind: &str, path: &[AbilityId]) -> String {
    let path = path
        .iter()
        .map(AbilityId::as_str)
        .collect::<Vec<_>>()
        .join("/");
    format!("{face_name} — {kind} ({path})")
}

pub fn mode_fallback(parent_fallback: &str, mode_id: &ModeId) -> String {
    format!("{parent_fallback} — mode ({mode_id})")
}

/// Deterministic presentation for an identified nested choice when external Oracle text is not
/// available. The stable ID remains visible so sibling choices never collapse to the same label.
pub fn choice_fallback(kind: &str, choice_id: &ChoiceId) -> String {
    format!("{kind} ({choice_id})")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct FaceIdProbe {
        id: String,
        face_id: CardFaceId,
    }

    #[test]
    fn non_game_layouts_are_display_identities_only() {
        for layout in [
            "token",
            "double_faced_token",
            "emblem",
            "art_series",
            "front_card",
        ] {
            assert!(is_non_game_layout(layout), "{layout}");
        }
        for layout in [
            "normal",
            "transform",
            "modal_dfc",
            "split",
            "flip",
            "adventure",
            "meld",
            "saga",
            "class",
            "leveler",
            "prototype",
            "",
        ] {
            assert!(!is_non_game_layout(layout), "{layout}");
        }
    }

    #[test]
    fn ron_distinguishes_id_from_face_id() {
        let parsed: FaceIdProbe =
            ron::from_str("(id: \"token\", face_id: \"token_face\")").expect("distinct RON fields");
        assert_eq!(parsed.id, "token");
        assert_eq!(parsed.face_id.as_str(), "token_face");
    }

    #[test]
    fn external_lines_normalize_newlines_and_ignore_blank_lines() {
        assert_eq!(
            external_oracle_lines(" Choose one —\r\n\r\n• First\r  • Second  \n"),
            ["Choose one —", "• First", "• Second"]
        );
    }

    #[test]
    fn external_resolution_is_all_or_fallback() {
        let mapping = AbilityPresentation::OracleLines(vec![1, 3]);
        assert_eq!(
            resolve_external_presentation(&mapping, Some("One\nTwo\nThree"), || "fallback".into()),
            "One\nThree"
        );
        assert_eq!(
            resolve_external_presentation(&mapping, Some("One\nTwo"), || "fallback".into()),
            "fallback"
        );
        assert_eq!(
            resolve_external_presentation(&mapping, None, || "fallback".into()),
            "fallback"
        );
    }

    #[test]
    fn fragment_resolution_selects_the_printed_part_or_falls_back() {
        let text = "Enchant creature\nEnchanted creature has \"{T}: Draw a card.\" (Reminder.)";
        let quote = AbilityPresentation::OracleFragment {
            line: 2,
            part: OracleFragmentPart::Quote(1),
        };
        assert_eq!(
            resolve_external_presentation(&quote, Some(text), || "fallback".into()),
            "{T}: Draw a card."
        );
        for missing in [
            AbilityPresentation::OracleFragment {
                line: 3,
                part: OracleFragmentPart::Quote(1),
            },
            AbilityPresentation::OracleFragment {
                line: 2,
                part: OracleFragmentPart::Quote(2),
            },
            AbilityPresentation::OracleFragment {
                line: 0,
                part: OracleFragmentPart::Quote(1),
            },
        ] {
            assert_eq!(
                resolve_external_presentation(&missing, Some(text), || "fallback".into()),
                "fallback"
            );
        }
        assert!(quote.validate().is_ok());
        assert!(AbilityPresentation::OracleFragment {
            line: 1,
            part: OracleFragmentPart::Sentence(0)
        }
        .validate()
        .is_err());
    }

    #[test]
    fn fragment_ron_names_line_and_part() {
        let parsed: AbilityPresentation =
            ron::from_str("OracleFragment(line: 2, part: Keyword(3))").unwrap();
        assert_eq!(
            parsed,
            AbilityPresentation::OracleFragment {
                line: 2,
                part: OracleFragmentPart::Keyword(3)
            }
        );
    }

    #[test]
    fn choice_labels_default_to_generated_and_never_accept_fallback() {
        #[derive(Deserialize)]
        struct Probe {
            #[serde(default)]
            presentation: ChoicePresentation,
        }
        let parsed: Probe = ron::from_str("()").unwrap();
        assert!(parsed.presentation.is_generated());
        assert!(parsed.presentation.oracle_spans().is_empty());
        let mapped: Probe = ron::from_str("(presentation: OracleLines([2]))").unwrap();
        assert_eq!(
            mapped.presentation.oracle_spans(),
            [OracleTextSpan {
                line: 2,
                range: None
            }]
        );
        assert!(ron::from_str::<Probe>("(presentation: Fallback)").is_err());
        assert!(ChoicePresentation::OracleLines(vec![]).validate().is_err());
    }

    #[test]
    fn span_selection_is_ordered_non_overlapping_and_all_or_nothing() {
        let lines = vec!["One two".to_string(), "Three".to_string()];
        let span = |line, range| OracleTextSpan { line, range };
        assert_eq!(
            select_oracle_spans(&lines, &[span(1, Some((4, 7))), span(2, None)]).as_deref(),
            Some("two\nThree")
        );
        assert_eq!(
            select_oracle_spans(&lines, &[span(1, Some((0, 3))), span(1, Some((4, 7)))]).as_deref(),
            Some("One\ntwo")
        );
        for invalid in [
            vec![],
            vec![span(3, None)],
            vec![span(0, None)],
            vec![span(1, Some((4, 8)))],
            vec![span(1, Some((3, 3)))],
            vec![span(2, None), span(1, None)],
            vec![span(1, Some((0, 4))), span(1, Some((3, 7)))],
        ] {
            assert_eq!(select_oracle_spans(&lines, &invalid), None, "{invalid:?}");
        }
    }

    #[test]
    fn malformed_external_mapping_uses_fallback_without_panicking() {
        for indices in [vec![], vec![0], vec![2, 1], vec![1, 1]] {
            assert_eq!(
                resolve_external_presentation(
                    &AbilityPresentation::OracleLines(indices),
                    Some("One\nTwo"),
                    || "fallback".into()
                ),
                "fallback"
            );
        }
    }

    #[test]
    fn program_owner_identity_uses_the_exact_parent_and_typed_mode_id() {
        let mode_id = ModeId::new("choose_mode").unwrap();
        let ability_id = AbilityId::new("activated_01").unwrap();
        let nested_id = AbilityId::new("nested_01").unwrap();
        let face_front = CardFaceId::new("front").unwrap();
        let face_back = CardFaceId::new("back").unwrap();

        let definition_front: crate::AbilityDefinitionId = crate::identity::AbilityDefinitionId {
            card_id: "owner_probe".into(),
            face_id: face_front.clone(),
            ability_path: vec![ability_id.clone()],
        };
        let definition_back = crate::AbilityDefinitionId {
            card_id: "owner_probe".into(),
            face_id: face_back.clone(),
            ability_path: vec![ability_id.clone()],
        };
        let nested_definition = crate::AbilityDefinitionId {
            card_id: "owner_probe".into(),
            face_id: face_front.clone(),
            ability_path: vec![ability_id.clone(), nested_id],
        };

        let owners = [
            ProgramOwnerId::face("owner_probe", face_front.clone()),
            ProgramOwnerId::face_mode("owner_probe", face_front.clone(), mode_id.clone()),
            ProgramOwnerId::face_mode("owner_probe", face_back, mode_id.clone()),
            ProgramOwnerId::ability(definition_front.clone()),
            ProgramOwnerId::ability(definition_back.clone()),
            ProgramOwnerId::ability_mode(definition_front, mode_id.clone()),
            ProgramOwnerId::ability_mode(nested_definition, mode_id),
        ];

        assert_eq!(
            owners
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            owners.len(),
            "equal local ability and mode IDs under different parents stay distinct"
        );
    }
}
