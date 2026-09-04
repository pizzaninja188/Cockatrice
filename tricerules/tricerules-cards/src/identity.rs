//! Stable authored identity and presentation metadata for card definitions.

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
stable_id!(SearchResultId, "search result id");

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

/// Non-mechanical instructions for presenting an authored spell or ability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbilityPresentation {
    /// One-based lines in the external Oracle text for this exact card face.
    OracleLines(Vec<u16>),
    /// No exact external mapping exists; callers must use the deterministic fallback.
    Fallback,
}

impl AbilityPresentation {
    pub fn validate(&self) -> Result<(), String> {
        let Self::OracleLines(lines) = self else {
            return Ok(());
        };
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

/// Resolve an all-or-fallback external presentation mapping.
pub fn resolve_external_presentation(
    presentation: &AbilityPresentation,
    external_face_text: Option<&str>,
    fallback: impl FnOnce() -> String,
) -> String {
    if let (AbilityPresentation::OracleLines(indices), Some(text)) =
        (presentation, external_face_text)
    {
        let lines = external_oracle_lines(text);
        let selected = indices
            .iter()
            .map(|index| lines.get(usize::from(*index) - 1))
            .collect::<Option<Vec<_>>>();
        if let Some(selected) = selected {
            return selected
                .into_iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join("\n");
        }
    }
    fallback()
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
}
