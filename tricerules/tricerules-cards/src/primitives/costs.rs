//! Costs paid to activate abilities.

use super::{BattlefieldPermanentFilter, GameCondition, TargetFilter, ZoneCardFilter};
use crate::mana::ManaCost;
use serde::{Deserialize, Serialize};

/// Cost to activate an activated ability (CR 602). Shared by every activated ability,
/// including mana abilities: an ability is classified as a mana ability (CR 605.1a) by its
/// *effect* being [`SpellEffectKind::ProduceMana`], not by its cost — so a `{T}` land, a
/// `{1}, {T}` filter land, and a sacrifice-for-mana rock all use these same cost kinds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbilityCost {
    /// CR 701.68: Gristle Glutton and Spiral into Solitude put all counters on one creature.
    Blight { count: u32 },
    /// CR 606.4: add (positive), remove (negative), or leave unchanged (zero) loyalty counters
    /// as the cost of activating a planeswalker's loyalty ability.
    Loyalty(i32),
    /// {T}: tap the source permanent.
    Tap,
    /// Tap exactly `count` untapped permanents the activating player controls that match
    /// `filter`. This is a selection cost, not targeting, and summoning sickness does not apply
    /// unless the selected object is also paying a separate `{T}` source cost.
    TapPermanents {
        count: u32,
        filter: TargetFilter,
        #[serde(default)]
        exclude_source: bool,
    },
    /// Pay mana (e.g. `"{4}"`, `"{2}{R}"`). Same brace syntax as `CardDefinition.mana_cost`.
    Mana(ManaCost),
    /// Discard one card chosen from the activating player's hand.
    Discard,
    /// Discard the source object itself from its owner's hand (cycling and typecycling).
    DiscardSelf,
    /// Exile the source object itself from its owner's graveyard (renew).
    ExileSelf,
    /// Sacrifice the source permanent as cost (e.g. Bottle Gnomes).
    SacrificeSelf,
    /// Sacrifice another or the source permanent when it matches the filter (e.g. Portcullis
    /// Vine's "Sacrifice a creature with defender"). This is selection, not targeting: shroud
    /// and hexproof do not apply.
    SacrificePermanent { filter: TargetFilter },
    /// Exile exactly `count` matching cards from the activating player's graveyard. This is a
    /// selection cost, not targeting. Say Its Name excludes its own source object; Bearscape and
    /// Grim Lavamancer reuse the same bounded graveyard-cohort payment without source exclusion.
    ExileGraveyardCards {
        count: u32,
        filter: ZoneCardFilter,
        #[serde(default)]
        exclude_source: bool,
    },
}

impl AbilityCost {
    pub(crate) fn validate(&self) -> Result<(), String> {
        match self {
            Self::Blight { count: 0 } => Err("blight cost requires a positive count".into()),
            Self::TapPermanents { count: 0, .. } => {
                Err("permanent tap cost requires a positive count".into())
            }
            Self::TapPermanents { filter, .. } => filter.validate_characteristic_constraints(),
            Self::ExileGraveyardCards { count: 0, .. } => {
                Err("graveyard-card exile cost requires a positive count".into())
            }
            Self::ExileGraveyardCards { filter, .. } => filter.validate(),
            _ => Ok(()),
        }
    }
}

/// Mandatory additional costs paid while casting a spell (CR 118.8, 601.2f-h).
///
/// These components deliberately exclude mana: the face's normal or alternate mana cost remains
/// the single mana component, while this ordered list supplies authored nonmana choices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdditionalCost {
    /// Mandatory Blight payment, sharing the cast-option and activated-cost operation.
    Blight { count: u32 },
    /// Discard one other card chosen from the caster's hand.
    DiscardCard,
    /// Sacrifice one permanent the caster controls that matches `filter`.
    SacrificePermanent { filter: TargetFilter },
    /// Tap exactly `count` matching untapped permanents the caster controls.
    TapPermanents {
        count: u32,
        filter: TargetFilter,
        #[serde(default)]
        exclude_source: bool,
    },
}

/// One announced cast-time cost choice group (CR 601.2b). Kicker and behold share this
/// vocabulary: both record an option before targets are chosen, then let later rules text query
/// the stable receipt instead of re-examining the paid object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CastCostGroupDef {
    pub prompt: String,
    #[serde(default)]
    pub min: u32,
    #[serde(default = "default_one")]
    pub max: u32,
    pub options: Vec<CastCostOptionDef>,
}

fn default_one() -> u32 {
    1
}

/// One mutually distinguishable option in a cast-cost group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CastCostOptionDef {
    /// Cinder Strike's optional payment and Wild Unraveling's Blight-or-mana group.
    Blight { label: String, count: u32 },
    /// Pay this additional mana as part of the spell's single total cost. Grow from the Ashes and
    /// Gnarlid Colony use this for kicker.
    Mana { label: String, cost: ManaCost },
    /// CR 701.4: reveal one matching hand card or choose one matching permanent you control.
    /// Caustic Exhale and Osseous Exhale use the same typed selection.
    Behold {
        label: String,
        hand_filter: ZoneCardFilter,
        permanent_filter: Box<TargetFilter>,
    },
}

impl CastCostGroupDef {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.prompt.trim().is_empty() {
            return Err("cast cost group prompt must not be empty".into());
        }
        if self.options.is_empty() || self.min > self.max || self.max != 1 {
            return Err("cast cost group requires min <= max = 1".into());
        }
        let mut labels = std::collections::HashSet::new();
        for option in &self.options {
            let label = match option {
                CastCostOptionDef::Mana { label, .. }
                | CastCostOptionDef::Behold { label, .. }
                | CastCostOptionDef::Blight { label, .. } => label,
            };
            if !labels.insert(label.trim()) {
                return Err("cast cost group option labels must be unique".into());
            }
            match option {
                CastCostOptionDef::Blight { label, count } => {
                    if label.trim().is_empty() || *count == 0 {
                        return Err("blight option requires a label and positive count".into());
                    }
                }
                CastCostOptionDef::Mana { label, cost } => {
                    if label.trim().is_empty() || cost.is_empty() {
                        return Err("cast mana option requires a label and nonempty cost".into());
                    }
                }
                CastCostOptionDef::Behold {
                    label,
                    hand_filter,
                    permanent_filter,
                } => {
                    if label.trim().is_empty() {
                        return Err("behold option requires a label".into());
                    }
                    hand_filter.validate()?;
                    permanent_filter.validate_target_constraints()?;
                }
            }
        }
        Ok(())
    }
}

/// A typed linked condition over an announced cast-cost option. `expected_selected = false`
/// supports the inverse branch without requiring clients or effects to infer a default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CastCostReceiptCondition {
    pub group_index: u32,
    pub option_index: u32,
    #[serde(default = "default_true")]
    pub expected_selected: bool,
}

fn default_true() -> bool {
    true
}

/// A face-authored modifier applied while determining that spell's total cost (CR 601.2f).
///
/// The condition vocabulary is shared with triggers, activated abilities, and conditional
/// effects so card data never reimplements public game-state queries. Winged Words uses a
/// controller-owned flying-creature condition; Purple Worm and Bone Picker can reuse the same
/// modifier with the existing creature-deaths-this-turn condition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpellCostModifier {
    /// Automatic public quantities, e.g. Witchstalker Frenzy's attackers and affinity cohorts.
    GenericReduction { amount: super::Amount },
    /// Reduce only the generic component of the selected normal or alternative mana cost.
    ConditionalGenericReduction {
        amount: u32,
        condition: GameCondition,
    },
    /// Reduce this spell's generic cost once when an announced target matches `filter`.
    /// Luminous Rebuke inspects battlefield creatures; No One Left Behind inspects graveyard cards.
    TargetMatchGenericReduction {
        amount: u32,
        filter: super::TargetMatchFilter,
    },
    /// Reduce this spell's generic cost for each matching battlefield permanent. Affinity for
    /// creatures and future affinity-style cohorts share this counted form.
    BattlefieldCountGenericReduction {
        amount_per_match: u32,
        filter: BattlefieldPermanentFilter,
        #[serde(default)]
        aggregate: super::BattlefieldAggregate,
    },
}

/// One modifier applied while determining an activated ability's mana cost (CR 602.2b).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivatedCostModifier {
    ConditionalGenericReduction {
        amount: u32,
        condition: GameCondition,
    },
}

impl ActivatedCostModifier {
    pub(crate) fn validate(&self) -> Result<(), String> {
        match self {
            Self::ConditionalGenericReduction { amount, condition } => {
                if *amount == 0 {
                    return Err("activated generic cost reduction must be nonzero".into());
                }
                condition.validate_live()
            }
        }
    }
}

impl SpellCostModifier {
    pub(crate) fn validate(&self) -> Result<(), String> {
        match self {
            Self::GenericReduction { amount } => amount.validate_cost(false),
            Self::ConditionalGenericReduction { amount, condition } => {
                if *amount == 0 {
                    return Err("conditional generic cost reduction must be nonzero".into());
                }
                condition.validate_live()
            }
            Self::TargetMatchGenericReduction { amount, filter } => {
                if *amount == 0 {
                    return Err("target-matching generic cost reduction must be nonzero".into());
                }
                filter.validate()
            }
            Self::BattlefieldCountGenericReduction {
                amount_per_match,
                filter,
                ..
            } => {
                if *amount_per_match == 0 {
                    return Err("battlefield-count generic cost reduction must be nonzero".into());
                }
                filter.validate()
            }
        }
    }
}
