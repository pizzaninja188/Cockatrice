//! Target kinds and composable target filters.

use super::{Color, Keyword, SpellEffectKind};
use serde::{Deserialize, Serialize};

/// Authored target declaration for one spell, mode, activated ability, or triggered ability.
/// Omit it to retain the legacy single required group containing every targeted effect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetingDef {
    pub groups: Vec<TargetGroupDef>,
}

/// One independently prompted target group. `effect_indices` names the effects in the sibling
/// effect list that consume this group's ordered targets. Candidate publication intersects the
/// requirements of every referenced effect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetGroupDef {
    pub min: u32,
    pub max: u32,
    pub prompt: String,
    pub effect_indices: Vec<u32>,
    /// Indices of other groups whose chosen objects must be distinct from this group.
    #[serde(default)]
    pub distinct_from: Vec<u32>,
}

impl TargetingDef {
    fn validate_player_recipient_target_groups(
        targeting: Option<&Self>,
        effects: &[SpellEffectKind],
    ) -> Result<(), String> {
        let referenced_groups = effects.iter().filter_map(|effect| match effect {
            SpellEffectKind::CreateTokens {
                who: super::PlayerRecipient::ControllerOfTargetGroup { group_index },
                ..
            }
            | SpellEffectKind::DamagePlayer {
                who: super::PlayerRecipient::ControllerOfTargetGroup { group_index },
                ..
            }
            | SpellEffectKind::LoseLife {
                who: super::PlayerRecipient::ControllerOfTargetGroup { group_index },
                ..
            }
            | SpellEffectKind::Mill {
                who: super::PlayerRecipient::ControllerOfTargetGroup { group_index },
                ..
            } => Some(*group_index),
            _ => None,
        });

        for group_index in referenced_groups {
            let permanent_filters = if let Some(targeting) = targeting {
                let group = targeting.groups.get(group_index as usize).ok_or_else(|| {
                    "player recipient references an unknown target group".to_string()
                })?;
                if group.min != 1 || group.max != 1 {
                    return Err(
                        "controller-of-target-group recipient requires exactly one target".into(),
                    );
                }
                group
                    .effect_indices
                    .iter()
                    .filter_map(|effect_index| effects.get(*effect_index as usize))
                    .flat_map(SpellEffectKind::target_filters)
                    .collect::<Vec<_>>()
            } else {
                if group_index != 0 {
                    return Err(
                        "player recipient references an unknown implicit target group".into(),
                    );
                }
                effects
                    .iter()
                    .flat_map(SpellEffectKind::target_filters)
                    .collect::<Vec<_>>()
            };
            if permanent_filters.is_empty()
                || permanent_filters.iter().any(|filter| {
                    !filter.all_terminal_filters_match(|leaf| {
                        matches!(leaf.kind, TargetKind::Creature | TargetKind::AnyPermanent)
                    })
                })
            {
                return Err(
                    "controller-of-target-group recipient requires a permanent-only target group"
                        .into(),
                );
            }
        }
        Ok(())
    }

    pub(crate) fn validate(&self, effects: &[SpellEffectKind]) -> Result<(), String> {
        if self.groups.is_empty() {
            return Err("targeting requires at least one group".into());
        }
        let mut referenced = vec![0u32; effects.len()];
        for (group_index, group) in self.groups.iter().enumerate() {
            if group.min > group.max || group.max == 0 {
                return Err("target group requires min <= max and max > 0".into());
            }
            if group.prompt.trim().is_empty() {
                return Err("target group prompt must not be empty".into());
            }
            if group.effect_indices.is_empty() {
                return Err("target group must reference at least one effect".into());
            }
            let mut within_group = std::collections::HashSet::new();
            for &effect_index in &group.effect_indices {
                let effect = effects
                    .get(effect_index as usize)
                    .ok_or_else(|| "target group references an unknown effect".to_string())?;
                if !within_group.insert(effect_index) {
                    return Err("target group references an effect more than once".into());
                }
                if !effect.needs_target() {
                    return Err("target group references an untargeted effect".into());
                }
                referenced[effect_index as usize] += 1;
            }
            let mut distinct = std::collections::HashSet::new();
            for &other in &group.distinct_from {
                if other as usize >= self.groups.len() || other as usize == group_index {
                    return Err("target group has an invalid distinctness reference".into());
                }
                if !distinct.insert(other) {
                    return Err("target group repeats a distinctness reference".into());
                }
            }
        }
        for (index, effect) in effects.iter().enumerate() {
            let expected = if effect.needs_target() {
                effect.target_filters().len().max(1) as u32
            } else {
                0
            };
            if referenced[index] != expected {
                return Err(format!(
                    "targeted effect {index} must belong to exactly {expected} target group(s)"
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn validate_optional(
        targeting: Option<&Self>,
        effects: &[SpellEffectKind],
    ) -> Result<(), String> {
        if let Some(targeting) = targeting {
            targeting.validate(effects)?;
            return Self::validate_player_recipient_target_groups(Some(targeting), effects);
        }
        if effects
            .iter()
            .any(|effect| effect.target_filters().len() > 1)
        {
            return Err("an effect with multiple target roles requires grouped targeting".into());
        }
        Self::validate_player_recipient_target_groups(None, effects)
    }
}

/// Base kind for a [`TargetFilter`] — what category of object is targeted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TargetKind {
    /// Creature or player (later expands to planeswalker/battle).
    #[default]
    AnyTarget,
    /// A creature on the battlefield.
    Creature,
    /// Any player still in the game, including the caster.
    AnyPlayer,
    /// Any player still in the game except the caster.
    OpponentPlayer,
    /// Any permanent on the battlefield (artifact, creature, or land).
    AnyPermanent,
}

/// Card-type predicate shared by spells on the stack and cards in other zones. `None` on a
/// containing field means no type restriction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardTypeFilter {
    /// A land card with the Basic supertype (Evolving Wilds, Rampant Growth).
    BasicLand,
    /// Any land card, basic or nonbasic.
    Land,
    Enchantment,
    Instant,
    Sorcery,
    /// Matches instants and sorceries (Talrand, Young Pyromancer, Mystical Tutor).
    InstantOrSorcery,
    Creature,
    Artifact,
    /// Matches planeswalker spells and permanents. Loyalty rules remain a separate engine
    /// capability; this vocabulary is also needed by mana restrictions such as Chandra's
    /// Embercat.
    Planeswalker,
    /// Matches any card that does not have the land card type (Coercion, Thoughtseize, and
    /// Cracked Skull).
    Nonland,
    /// Matches any object that does not have the creature card type.
    Noncreature,
}

/// Controller relationship required of a permanent target, relative to the spell or ability
/// controller. This remains independently composable with the other `TargetFilter` predicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TargetController {
    /// A permanent controlled by any player.
    #[default]
    Any,
    /// A permanent controlled by the spell or ability's controller ("you control").
    You,
    /// A permanent controlled by an opponent of the spell or ability's controller.
    Opponent,
    /// A permanent not controlled by the spell or ability's controller. Unlike `Opponent`, this
    /// remains correct for teammates in player-set-aware formats.
    NotYou,
    /// A permanent controlled by the event-time defending player of the attack that caused this
    /// triggered ability. Valid only on attack triggers that publish that context.
    DefendingPlayer,
}

/// Inclusive comparison against a permanent's current derived power.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerComparison {
    AtLeast(u32),
    AtMost(u32),
}

/// Which player's graveyard a [`GraveyardFilter`] targets. Defaults to [`Controller`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GraveyardOwner {
    /// Only the effect controller's own graveyard ("your graveyard" — Raise Dead, Disentomb).
    #[default]
    Controller,
    /// Any player's graveyard ("a graveyard" — Grim Return, Beacon of Unrest).
    AnyPlayer,
}

/// Filter for graveyard-zone targets (cards in a graveyard, not battlefield permanents).
/// Parallel to [`TargetFilter`] but for a different zone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GraveyardFilter {
    /// A pure disjunction of two or more recursively validated filters. When present, every
    /// leaf field on this node must retain its default value.
    #[serde(default)]
    pub any_of: Option<Vec<Self>>,
    /// Which player's graveyard. Defaults to the caster's own graveyard.
    #[serde(default)]
    pub owner: GraveyardOwner,
    /// Optional card-type restriction. `None` = any card.
    #[serde(default)]
    pub card_type: Option<CardTypeFilter>,
    /// Card types a matching card must not have.
    #[serde(default)]
    pub excluded_card_types: Vec<CardTypeFilter>,
}

impl GraveyardFilter {
    pub(crate) fn validate(&self) -> Result<(), String> {
        self.validate_tree()?;
        let mut leaves = Vec::new();
        self.collect_terminal_filters(&mut leaves);
        if terminal_filters_have_duplicates(&leaves) {
            return Err("graveyard filter cannot contain duplicate terminal predicates".into());
        }
        Ok(())
    }

    fn validate_tree(&self) -> Result<(), String> {
        if let Some(branches) = &self.any_of {
            let mut leaf_fields = self.clone();
            leaf_fields.any_of = None;
            if leaf_fields != Self::default() {
                return Err("graveyard filter any_of must be a pure OR node".into());
            }
            if branches.len() < 2 {
                return Err("graveyard filter any_of requires at least two alternatives".into());
            }
            for branch in branches {
                branch.validate_tree()?;
            }
            return Ok(());
        }

        if has_duplicates(&self.excluded_card_types) {
            return Err("graveyard filter has a duplicate excluded card type".into());
        }
        if self
            .card_type
            .is_some_and(|required| self.excluded_card_types.contains(&required))
        {
            return Err("graveyard filter cannot both require and exclude a card type".into());
        }
        Ok(())
    }

    fn collect_terminal_filters<'a>(&'a self, out: &mut Vec<&'a Self>) {
        if let Some(branches) = &self.any_of {
            for branch in branches {
                branch.collect_terminal_filters(out);
            }
        } else {
            out.push(self);
        }
    }
}

/// Where a card returned from the graveyard lands (CR 400.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraveyardDestination {
    /// The card goes to its owner's hand (Raise Dead, Disentomb, Gravedigger ETB).
    Hand,
    /// The card enters the battlefield under the caster's control (reanimation spells).
    Battlefield,
}

fn default_creature_filter() -> TargetFilter {
    TargetFilter {
        kind: TargetKind::Creature,
        ..TargetFilter::default()
    }
}

/// Composable target predicate. A leaf combines its base [`TargetKind`] and optional
/// characteristic constraints with AND. An `any_of` node is a pure recursive OR whose other
/// fields remain default. Use only `kind` to get the same semantics as the original five
/// TargetSpec variants; add constraints to narrow further.
///
/// Example RON:
/// - `(kind: AnyTarget)` — any creature or player
/// - `(kind: Creature, not_artifact: true)` — non-artifact creature
/// - `(kind: Creature, tapped: true)` — tapped creature (for future use)
/// - `(kind: Creature, not_color: Black)` — nonblack creature (Doom Blade, Terror)
/// - `(kind: Creature, is_color: Green)` — green creature (Perish, Virtue's Ruin)
/// - `(kind: Creature, attacking_or_blocking: true)` — Divine Verdict, Hunt Down
/// - `(kind: Creature, controller: You)` — "target creature you control" (Equip, Regenerate,
///   many activated abilities). Enforced at targeting time relative to the activating player.
/// - `(kind: AnyPermanent, controller: Opponent, permanent_types: [Artifact, Enchantment])` —
///   Rambunctious Mutt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TargetFilter {
    /// A pure disjunction of two or more recursively validated filters. When present, every
    /// leaf field on this node must retain its default value.
    #[serde(default)]
    pub any_of: Option<Vec<Self>>,
    #[serde(default)]
    pub kind: TargetKind,
    /// If true, the target must not be an artifact.
    #[serde(default)]
    pub not_artifact: bool,
    /// If true, the target must not be a land. Totally Lost uses this with `AnyPermanent`.
    #[serde(default)]
    pub not_land: bool,
    /// If Some(true), target must be tapped; Some(false) must be untapped; None = either.
    #[serde(default)]
    pub tapped: Option<bool>,
    /// CR 508/509: if true, the target must currently be attacking or blocking. Combat-only
    /// removal/tricks — Divine Verdict, Hunt Down ("destroy target attacking or blocking creature").
    #[serde(default)]
    pub attacking_or_blocking: bool,
    /// CR 105/202.2: if `Some`, the target must NOT be of this color (derived from its mana cost).
    /// Doom Blade ("nonblack creature"), Terror ("nonblack" — paired with `not_artifact`).
    #[serde(default)]
    pub not_color: Option<Color>,
    /// CR 105/202.2: if `Some`, the object must BE of this color — the inclusive mirror of
    /// `not_color`. Perish ("all green creatures") and Virtue's Ruin ("all white creatures") on
    /// the untargeted side; Hydroblast / Pyroblast ("target red permanent") on the targeted side.
    #[serde(default)]
    pub is_color: Option<Color>,
    /// Controller relationship for a permanent target, relative to the spell or ability's
    /// controller. Covers "you control" (Equipment, regeneration) and "an opponent controls"
    /// (Glaring Aegis, Rambunctious Mutt) while composing with every other filter field.
    #[serde(default)]
    pub controller: TargetController,
    /// If true, the object that sourced this spell or ability is not a legal target. The engine
    /// compares full object identity (ObjectId plus zone-change generation), so a card that leaves
    /// and returns is a new object under CR 400.7 and is no longer excluded.
    #[serde(default)]
    pub exclude_source: bool,
    /// Optional OR-combined permanent type restriction. For example, Icy Manipulator uses
    /// `[Artifact, Creature, Land]`; an empty list means no additional type restriction.
    #[serde(default)]
    pub permanent_types: Vec<super::PermanentTypeFilter>,
    /// Subtypes that disqualify the target (for example, Eyeblight's Ending's "non-Elf"
    /// restriction). Empty means no excluded subtypes.
    #[serde(default)]
    pub excluded_subtypes: Vec<String>,
    /// Optional comparison against current derived power. A noncreature has no power and cannot
    /// match a power-constrained filter.
    #[serde(default)]
    pub power: Option<PowerComparison>,
    /// Keywords every matching permanent must currently have. Shared by targeted, untargeted,
    /// and cost-selection predicates (for example Defender on Portcullis Vine / Run Afoul).
    #[serde(default)]
    pub required_keywords: Vec<Keyword>,
    /// Keywords every matching permanent must currently lack. This is the negative counterpart
    /// to `required_keywords` and is shared by targets and untargeted selections.
    #[serde(default)]
    pub excluded_keywords: Vec<Keyword>,
}

impl TargetFilter {
    /// Default: any creature (the most common implicit filter).
    pub fn default_creature() -> Self {
        default_creature_filter()
    }

    /// Default filter for the equip ability: "target creature you control" (CR 702.6a).
    pub fn default_equip() -> Self {
        TargetFilter {
            kind: TargetKind::Creature,
            controller: TargetController::You,
            ..TargetFilter::default()
        }
    }

    /// Validate constraints that depend on the filter representing an actual target. Player and
    /// `AnyTarget` filters cannot carry a permanent-controller predicate, while source exclusion
    /// is meaningful for every object-capable kind (including `AnyTarget`) but never player-only
    /// kinds.
    pub(crate) fn validate_target_constraints(&self) -> Result<(), String> {
        self.validate_tree(true)?;
        self.reject_duplicate_terminal_filters()?;
        Ok(())
    }

    fn validate_target_leaf(&self) -> Result<(), String> {
        self.validate_characteristic_leaf()?;
        if self.controller != TargetController::Any
            && !matches!(self.kind, TargetKind::Creature | TargetKind::AnyPermanent)
        {
            return Err(format!(
                "controller-relative target filter requires Creature or AnyPermanent kind, got {:?}",
                self.kind
            ));
        }
        if self.exclude_source && self.is_player() {
            return Err(format!(
                "source-excluding target filter requires an object-capable kind, got {:?}",
                self.kind
            ));
        }
        Ok(())
    }

    /// Reject contradictory characteristic predicates wherever this filter is used, including
    /// untargeted mass effects and sacrifice selections.
    pub(crate) fn validate_characteristic_constraints(&self) -> Result<(), String> {
        self.validate_tree(false)?;
        self.reject_duplicate_terminal_filters()
    }

    fn validate_tree(&self, target_context: bool) -> Result<(), String> {
        if let Some(branches) = &self.any_of {
            let mut leaf_fields = self.clone();
            leaf_fields.any_of = None;
            if leaf_fields != Self::default() {
                return Err("target filter any_of must be a pure OR node".into());
            }
            if branches.len() < 2 {
                return Err("target filter any_of requires at least two alternatives".into());
            }
            for branch in branches {
                branch.validate_tree(target_context)?;
            }
            return Ok(());
        }

        if target_context {
            self.validate_target_leaf()
        } else {
            self.validate_characteristic_leaf()
        }
    }

    fn validate_characteristic_leaf(&self) -> Result<(), String> {
        if self.is_player() && self.has_permanent_only_constraints() {
            return Err(format!(
                "player-only target filter cannot carry permanent-only constraints, got {:?}",
                self.kind
            ));
        }
        if self.is_color.is_some() && self.is_color == self.not_color {
            return Err("target filter cannot both require and exclude the same color".into());
        }
        if has_duplicates(&self.permanent_types) {
            return Err("target filter cannot repeat a required permanent type".into());
        }
        if self.not_artifact
            && self
                .permanent_types
                .contains(&super::PermanentTypeFilter::Artifact)
        {
            return Err("target filter cannot both require and exclude Artifact".into());
        }
        if self.not_land
            && self
                .permanent_types
                .contains(&super::PermanentTypeFilter::Land)
        {
            return Err("target filter cannot both require and exclude Land".into());
        }
        if has_duplicates(&self.required_keywords) {
            return Err("target filter cannot repeat a required keyword".into());
        }
        if has_duplicates(&self.excluded_keywords) {
            return Err("target filter cannot repeat an excluded keyword".into());
        }
        if let Some(keyword) = self
            .required_keywords
            .iter()
            .find(|keyword| self.excluded_keywords.contains(keyword))
        {
            return Err(format!(
                "target filter cannot both require and exclude keyword {keyword:?}"
            ));
        }
        if self
            .excluded_subtypes
            .iter()
            .any(|subtype| subtype.trim().is_empty())
        {
            return Err("target filter excluded subtype names must not be empty".into());
        }
        Ok(())
    }

    fn has_permanent_only_constraints(&self) -> bool {
        self.not_artifact
            || self.not_land
            || self.tapped.is_some()
            || self.attacking_or_blocking
            || self.not_color.is_some()
            || self.is_color.is_some()
            || self.controller != TargetController::Any
            || self.exclude_source
            || !self.permanent_types.is_empty()
            || !self.excluded_subtypes.is_empty()
            || self.power.is_some()
            || !self.required_keywords.is_empty()
            || !self.excluded_keywords.is_empty()
    }

    fn reject_duplicate_terminal_filters(&self) -> Result<(), String> {
        let mut leaves = Vec::new();
        self.collect_terminal_filters(&mut leaves);
        if terminal_filters_have_duplicates(&leaves) {
            return Err("target filter cannot contain duplicate terminal predicates".into());
        }
        Ok(())
    }

    fn collect_terminal_filters<'a>(&'a self, out: &mut Vec<&'a Self>) {
        if let Some(branches) = &self.any_of {
            for branch in branches {
                branch.collect_terminal_filters(out);
            }
        } else {
            out.push(self);
        }
    }

    /// True when every terminal predicate satisfies `predicate`.
    pub fn all_terminal_filters_match(&self, predicate: impl Fn(&Self) -> bool + Copy) -> bool {
        self.any_of.as_ref().map_or_else(
            || predicate(self),
            |branches| {
                branches
                    .iter()
                    .all(|branch| branch.all_terminal_filters_match(predicate))
            },
        )
    }

    /// True when at least one terminal predicate satisfies `predicate`.
    pub fn any_terminal_filter_matches(&self, predicate: impl Fn(&Self) -> bool + Copy) -> bool {
        self.any_of.as_ref().map_or_else(
            || predicate(self),
            |branches| {
                branches
                    .iter()
                    .any(|branch| branch.any_terminal_filter_matches(predicate))
            },
        )
    }

    /// True for player-only kinds (used by startup validation).
    pub fn is_player(&self) -> bool {
        self.all_terminal_filters_match(|filter| {
            matches!(
                filter.kind,
                TargetKind::AnyPlayer | TargetKind::OpponentPlayer
            )
        })
    }

    /// True when every terminal predicate selects battlefield permanents only.
    pub fn is_permanent_only(&self) -> bool {
        self.all_terminal_filters_match(|filter| {
            matches!(filter.kind, TargetKind::Creature | TargetKind::AnyPermanent)
        })
    }
}

fn has_duplicates<T: PartialEq>(values: &[T]) -> bool {
    values
        .iter()
        .enumerate()
        .any(|(index, value)| values[..index].contains(value))
}

fn terminal_filters_have_duplicates<T: PartialEq>(leaves: &[&T]) -> bool {
    leaves
        .iter()
        .enumerate()
        .any(|(index, leaf)| leaves[..index].contains(leaf))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::{Amount, PermanentTypeFilter, PlayerRecipient};

    fn target_filter(ron: &str) -> TargetFilter {
        ron::from_str(ron).expect("deserialize target filter")
    }

    fn graveyard_filter(ron: &str) -> GraveyardFilter {
        ron::from_str(ron).expect("deserialize graveyard filter")
    }

    fn controller_of_target_effects(
        target_kind: TargetKind,
        group_index: u32,
    ) -> Vec<SpellEffectKind> {
        vec![
            SpellEffectKind::DamageTarget {
                amount: Amount::Fixed(4),
                target: TargetFilter {
                    kind: target_kind,
                    ..Default::default()
                },
            },
            SpellEffectKind::DamagePlayer {
                amount: Amount::Fixed(2),
                who: PlayerRecipient::ControllerOfTargetGroup { group_index },
            },
        ]
    }

    #[test]
    fn controller_of_target_group_requires_an_existing_permanent_only_group() {
        assert!(TargetingDef::validate_optional(
            None,
            &controller_of_target_effects(TargetKind::Creature, 0)
        )
        .is_ok());
        assert!(TargetingDef::validate_optional(
            None,
            &controller_of_target_effects(TargetKind::Creature, 1)
        )
        .is_err());
        assert!(TargetingDef::validate_optional(
            None,
            &controller_of_target_effects(TargetKind::AnyPlayer, 0)
        )
        .is_err());
    }

    #[test]
    fn controller_of_target_group_rejects_optional_or_multiple_targets() {
        let effects = controller_of_target_effects(TargetKind::Creature, 0);
        for (min, max) in [(0, 1), (1, 2)] {
            let targeting = TargetingDef {
                groups: vec![TargetGroupDef {
                    min,
                    max,
                    prompt: "Choose a creature".into(),
                    effect_indices: vec![0],
                    distinct_from: Vec::new(),
                }],
            };
            assert!(TargetingDef::validate_optional(Some(&targeting), &effects).is_err());
        }
    }

    #[test]
    fn issue_114_disjunctive_filter_shapes_deserialize_and_validate() {
        let make_your_move = target_filter(
            "(any_of: Some([(kind: AnyPermanent, permanent_types: [Artifact]), (kind: AnyPermanent, permanent_types: [Enchantment]), (kind: Creature, power: Some(AtLeast(4)))]))",
        );
        assert!(make_your_move.validate_target_constraints().is_ok());

        let broken_wings = target_filter(
            "(any_of: Some([(kind: AnyPermanent, permanent_types: [Artifact]), (kind: AnyPermanent, permanent_types: [Enchantment]), (kind: Creature, required_keywords: [Flying])]))",
        );
        assert!(broken_wings.validate_target_constraints().is_ok());

        let say_its_name = graveyard_filter(
            "(any_of: Some([(card_type: Some(Creature)), (card_type: Some(Land))]))",
        );
        assert!(say_its_name.validate().is_ok());

        let monastery_messenger =
            graveyard_filter("(card_type: Some(Noncreature), excluded_card_types: [Land])");
        assert!(monastery_messenger.validate().is_ok());
        assert_eq!(
            monastery_messenger.excluded_card_types,
            vec![CardTypeFilter::Land]
        );
    }

    #[test]
    fn issue_114_rejects_malformed_target_or_nodes() {
        for (ron, expected) in [
            ("(any_of: Some([]))", "at least two"),
            ("(any_of: Some([(kind: Creature)]))", "at least two"),
            (
                "(kind: Creature, any_of: Some([(kind: Creature), (kind: AnyPermanent)]))",
                "pure OR node",
            ),
            (
                "(any_of: Some([(kind: Creature), (kind: Creature)]))",
                "duplicate terminal",
            ),
            (
                "(any_of: Some([(kind: Creature), (any_of: Some([(kind: AnyPermanent)]))]))",
                "at least two",
            ),
        ] {
            let error = target_filter(ron)
                .validate_target_constraints()
                .expect_err("malformed target filter should fail");
            assert!(
                error.contains(expected),
                "{error:?} did not contain {expected:?}"
            );
        }
    }

    #[test]
    fn issue_114_rejects_contradictory_or_redundant_target_leaves() {
        let invalid = [
            TargetFilter {
                is_color: Some(Color::Green),
                not_color: Some(Color::Green),
                ..Default::default()
            },
            TargetFilter {
                permanent_types: vec![PermanentTypeFilter::Artifact],
                not_artifact: true,
                ..Default::default()
            },
            TargetFilter {
                permanent_types: vec![PermanentTypeFilter::Land],
                not_land: true,
                ..Default::default()
            },
            TargetFilter {
                permanent_types: vec![PermanentTypeFilter::Creature, PermanentTypeFilter::Creature],
                ..Default::default()
            },
            TargetFilter {
                required_keywords: vec![Keyword::Flying, Keyword::Flying],
                ..Default::default()
            },
            TargetFilter {
                excluded_keywords: vec![Keyword::Flying, Keyword::Flying],
                ..Default::default()
            },
            TargetFilter {
                required_keywords: vec![Keyword::Flying],
                excluded_keywords: vec![Keyword::Flying],
                ..Default::default()
            },
            TargetFilter {
                excluded_subtypes: vec!["  ".into()],
                ..Default::default()
            },
            TargetFilter {
                kind: TargetKind::AnyPlayer,
                power: Some(PowerComparison::AtLeast(4)),
                ..Default::default()
            },
        ];

        for filter in invalid {
            assert!(
                filter.validate_target_constraints().is_err(),
                "invalid target leaf unexpectedly passed: {filter:?}"
            );
        }
    }

    #[test]
    fn issue_114_rejects_malformed_or_contradictory_graveyard_filters() {
        for (ron, expected) in [
            ("(any_of: Some([]))", "at least two"),
            ("(any_of: Some([(card_type: Some(Creature))]))", "at least two"),
            (
                "(owner: AnyPlayer, any_of: Some([(card_type: Some(Creature)), (card_type: Some(Land))]))",
                "pure OR node",
            ),
            (
                "(any_of: Some([(card_type: Some(Creature)), (card_type: Some(Creature))]))",
                "duplicate terminal",
            ),
            (
                "(card_type: Some(Land), excluded_card_types: [Land])",
                "require and exclude",
            ),
            (
                "(excluded_card_types: [Creature, Creature])",
                "duplicate excluded card type",
            ),
        ] {
            let error = graveyard_filter(ron)
                .validate()
                .expect_err("malformed graveyard filter should fail");
            assert!(error.contains(expected), "{error:?} did not contain {expected:?}");
        }
    }
}
