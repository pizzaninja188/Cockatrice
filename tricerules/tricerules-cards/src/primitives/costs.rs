//! Costs paid to activate abilities.

use super::{GameCondition, TargetFilter};
use crate::mana::ManaCost;
use serde::{Deserialize, Serialize};

/// Cost to activate an activated ability (CR 602). Shared by every activated ability,
/// including mana abilities: an ability is classified as a mana ability (CR 605.1a) by its
/// *effect* being [`SpellEffectKind::ProduceMana`], not by its cost — so a `{T}` land, a
/// `{1}, {T}` filter land, and a sacrifice-for-mana rock all use these same cost kinds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbilityCost {
    /// {T}: tap the source permanent.
    Tap,
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
}

/// Mandatory additional costs paid while casting a spell (CR 118.8, 601.2f-h).
///
/// These components deliberately exclude mana: the face's normal or alternate mana cost remains
/// the single mana component, while this ordered list supplies authored nonmana choices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdditionalCost {
    /// Discard one other card chosen from the caster's hand.
    DiscardCard,
    /// Sacrifice one permanent the caster controls that matches `filter`.
    SacrificePermanent { filter: TargetFilter },
}

/// A face-authored modifier applied while determining that spell's total cost (CR 601.2f).
///
/// The condition vocabulary is shared with triggers, activated abilities, and conditional
/// effects so card data never reimplements public game-state queries. Winged Words uses a
/// controller-owned flying-creature condition; Purple Worm and Bone Picker can reuse the same
/// modifier with the existing creature-deaths-this-turn condition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpellCostModifier {
    /// Reduce only the generic component of the selected normal or alternative mana cost.
    ConditionalGenericReduction {
        amount: u32,
        condition: GameCondition,
    },
}

impl SpellCostModifier {
    pub(crate) fn validate(&self) -> Result<(), String> {
        match self {
            SpellCostModifier::ConditionalGenericReduction { amount, condition } => {
                if *amount == 0 {
                    return Err("conditional generic cost reduction must be nonzero".into());
                }
                condition.validate()
            }
        }
    }
}
