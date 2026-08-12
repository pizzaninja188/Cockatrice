//! Costs paid to activate abilities.

use super::TargetFilter;
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
