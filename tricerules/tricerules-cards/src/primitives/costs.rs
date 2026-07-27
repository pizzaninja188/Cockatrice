//! Costs paid to activate abilities.

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
    /// {T} plus mana (e.g. Jayemdae Tome: `"{4}"` + tap).
    TapAndMana(ManaCost),
    /// Sacrifice the source permanent as cost (e.g. Bottle Gnomes).
    Sacrifice,
}
