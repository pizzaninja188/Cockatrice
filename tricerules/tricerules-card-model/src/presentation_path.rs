//! The stable identity of one authored presentation node.
//!
//! The engine publishes exactly this path as `PresentationRef.path`, and the authoring audit,
//! exception rows and review packets key nodes by its text form. Program structure (step indexes,
//! serde variant wrappers, field names) never appears in it.

use crate::identity::{AbilityId, ChoiceId, ModeId};
use std::fmt;
use std::str::FromStr;

/// The reserved id the `Spell` component publishes.
pub const SPELL_PATH_ID: &str = "spell";

/// The kind of one path component. Mirrors the protocol's `PresentationPathKind` without the card
/// model depending on the protocol crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PresentationPathKind {
    Spell,
    Ability,
    Mode,
    CastCostGroup,
    CastCostOption,
    ResolutionBranch,
    SearchSlot,
    ManaRestriction,
}

impl PresentationPathKind {
    pub const ALL: [Self; 8] = [
        Self::Spell,
        Self::Ability,
        Self::Mode,
        Self::CastCostGroup,
        Self::CastCostOption,
        Self::ResolutionBranch,
        Self::SearchSlot,
        Self::ManaRestriction,
    ];

    /// The snake_case name used in the text form (the protocol enum suffix).
    pub fn name(self) -> &'static str {
        match self {
            Self::Spell => "spell",
            Self::Ability => "ability",
            Self::Mode => "mode",
            Self::CastCostGroup => "cast_cost_group",
            Self::CastCostOption => "cast_cost_option",
            Self::ResolutionBranch => "resolution_branch",
            Self::SearchSlot => "search_slot",
            Self::ManaRestriction => "mana_restriction",
        }
    }
}

/// One component of a [`PresentationPath`]. Every payload is a stable authored (or engine-owned)
/// id; positional indexes never appear.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PresentationPathComponent {
    Spell,
    Ability(AbilityId),
    Mode(ModeId),
    CastCostGroup(ChoiceId),
    CastCostOption(ChoiceId),
    ResolutionBranch(ChoiceId),
    SearchSlot(ChoiceId),
    ManaRestriction(ChoiceId),
}

impl PresentationPathComponent {
    pub fn kind(&self) -> PresentationPathKind {
        match self {
            Self::Spell => PresentationPathKind::Spell,
            Self::Ability(_) => PresentationPathKind::Ability,
            Self::Mode(_) => PresentationPathKind::Mode,
            Self::CastCostGroup(_) => PresentationPathKind::CastCostGroup,
            Self::CastCostOption(_) => PresentationPathKind::CastCostOption,
            Self::ResolutionBranch(_) => PresentationPathKind::ResolutionBranch,
            Self::SearchSlot(_) => PresentationPathKind::SearchSlot,
            Self::ManaRestriction(_) => PresentationPathKind::ManaRestriction,
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Spell => SPELL_PATH_ID,
            Self::Ability(id) => id.as_str(),
            Self::Mode(id) => id.as_str(),
            Self::CastCostGroup(id)
            | Self::CastCostOption(id)
            | Self::ResolutionBranch(id)
            | Self::SearchSlot(id)
            | Self::ManaRestriction(id) => id.as_str(),
        }
    }

    /// Rebuilds a component from its published kind and id.
    pub fn from_parts(kind: PresentationPathKind, id: &str) -> Result<Self, String> {
        let choice = || ChoiceId::new(id);
        Ok(match kind {
            PresentationPathKind::Spell if id == SPELL_PATH_ID => Self::Spell,
            PresentationPathKind::Spell => {
                return Err(format!(
                    "spell path component must have id '{SPELL_PATH_ID}', not '{id}'"
                ))
            }
            PresentationPathKind::Ability => Self::Ability(AbilityId::new(id)?),
            PresentationPathKind::Mode => Self::Mode(ModeId::new(id)?),
            PresentationPathKind::CastCostGroup => Self::CastCostGroup(choice()?),
            PresentationPathKind::CastCostOption => Self::CastCostOption(choice()?),
            PresentationPathKind::ResolutionBranch => Self::ResolutionBranch(choice()?),
            PresentationPathKind::SearchSlot => Self::SearchSlot(choice()?),
            PresentationPathKind::ManaRestriction => Self::ManaRestriction(choice()?),
        })
    }
}

impl fmt::Display for PresentationPathComponent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spell => formatter.write_str(SPELL_PATH_ID),
            other => write!(formatter, "{}:{}", other.kind().name(), other.id()),
        }
    }
}

impl FromStr for PresentationPathComponent {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text == SPELL_PATH_ID {
            return Ok(Self::Spell);
        }
        let (kind, id) = text
            .split_once(':')
            .ok_or_else(|| format!("presentation path component '{text}' must be 'kind:id'"))?;
        let kind = PresentationPathKind::ALL
            .into_iter()
            .find(|candidate| candidate.name() == kind && *candidate != PresentationPathKind::Spell)
            .ok_or_else(|| format!("unknown presentation path kind '{kind}' in '{text}'"))?;
        Self::from_parts(kind, id)
    }
}

/// The complete stable path of one presentation node within one card face. Its text form joins
/// components with `/`, e.g. `ability:static_01/ability:triggered_01` or
/// `spell/cast_cost_group:kicker/cast_cost_option:pay`.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PresentationPath(pub Vec<PresentationPathComponent>);

impl PresentationPath {
    pub fn new(components: impl IntoIterator<Item = PresentationPathComponent>) -> Self {
        Self(components.into_iter().collect())
    }

    pub fn spell() -> Self {
        Self(vec![PresentationPathComponent::Spell])
    }

    pub fn components(&self) -> &[PresentationPathComponent] {
        &self.0
    }

    pub fn push(&mut self, component: PresentationPathComponent) {
        self.0.push(component);
    }

    /// This path extended by one component.
    pub fn child(&self, component: PresentationPathComponent) -> Self {
        let mut child = self.clone();
        child.push(component);
        child
    }
}

impl fmt::Display for PresentationPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, component) in self.0.iter().enumerate() {
            if index > 0 {
                formatter.write_str("/")?;
            }
            component.fmt(formatter)?;
        }
        Ok(())
    }
}

impl FromStr for PresentationPath {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.is_empty() {
            return Err("presentation path must not be empty".to_owned());
        }
        text.split('/')
            .map(str::parse)
            .collect::<Result<_, _>>()
            .map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_form_round_trips_every_kind() {
        for text in [
            "spell",
            "ability:static_01/ability:triggered_01",
            "spell/mode:mode_02",
            "spell/cast_cost_group:kicker/cast_cost_option:pay",
            "ability:parent/resolution_branch:child",
            "spell/search_slot:land",
            "ability:activated_01/mana_restriction:creature_spells",
        ] {
            let path: PresentationPath = text.parse().unwrap();
            assert_eq!(path.to_string(), text);
        }
    }

    #[test]
    fn text_form_rejects_malformed_paths() {
        for text in [
            "",
            "static_abilities/static_01",
            "ability:",
            "spell:spell",
            "choice:x",
            "ability:Static",
            "ability:a//mode:b",
        ] {
            assert!(text.parse::<PresentationPath>().is_err(), "{text}");
        }
    }
}
