//! Firdoch Core is pinned in the Scryfall `oracle_cards` snapshot
//! `27bf3214-1271-490b-bdfe-c0be6c23d02e` (oracle id
//! `126c75c0-414b-4e55-8747-2b5566ce716f`). Its ECL 255 Oracle text has a `{4}` activated
//! ability that makes this artifact a 4/4 artifact creature until end of turn. Scryfall dates the
//! ruling 2025-11-17; WotC's Lorwyn Eclipsed release notes, published 2026-01-09, confirm this
//! ability overwrites earlier base P/T-setting effects; later ones overwrite it, while ordinary
//! modifiers and counters still apply. CR 205.1b, 611.2a, 613.1d,
//! and 613.4b govern retaining prior types, duration, type-changing layer, and setting base P/T.

use tricerules_cards::mana::ManaCost;
use tricerules_cards::primitives::{
    AbilityCost, CreatureTypeChange, ResolvingEffectDuration, SpellEffectKind,
};
use tricerules_cards::CardRegistry;

#[test]
fn firdoch_core_has_its_temporary_artifact_creature_ability() {
    let face = CardRegistry::global()
        .get("firdoch_core")
        .expect("Firdoch Core is registered")
        .primary_face();
    let [mana, animation] = face.activated_abilities.as_slice() else {
        panic!("Firdoch Core has mana and self-animation activated abilities");
    };

    assert_eq!(mana.costs, [AbilityCost::Tap]);
    assert_eq!(
        animation.costs,
        [AbilityCost::Mana(ManaCost::parse("{4}").unwrap())]
    );
    assert!(matches!(
        animation.effect.as_slice(),
        [SpellEffectKind::AnimateSelf {
            base_power: 4,
            base_toughness: 4,
            colors: None,
            creature_types: CreatureTypeChange::Preserve,
            keywords,
            duration: ResolvingEffectDuration::UntilEndOfTurn,
        }] if keywords.is_empty()
    ));
}
