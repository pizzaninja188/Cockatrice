use tricerules_cards::primitives::{PowerComparison, SpellEffectKind, TargetKind};
use tricerules_cards::{CardRegistry, Keyword};

#[test]
fn issue_71_cards_use_shared_power_and_keyword_filters() {
    let registry = CardRegistry::global();

    let judgment_definition = registry
        .get("legions_judgment")
        .expect("Legion's Judgment is registered");
    assert!(judgment_definition.partial.is_none());
    let judgment = judgment_definition.primary_face();
    assert!(matches!(
        judgment.spell_effect.as_slice(),
        [SpellEffectKind::DestroyTarget { target }]
            if target.kind == TargetKind::Creature
                && target.power == Some(PowerComparison::AtLeast(4))
    ));

    let air_strike_definition = registry
        .get("reckless_air_strike")
        .expect("Reckless Air Strike is registered");
    assert!(air_strike_definition.partial.is_none());
    let air_strike = air_strike_definition.primary_face();
    let modal = air_strike
        .modal_spell
        .as_ref()
        .expect("Reckless Air Strike is modal");
    assert_eq!((modal.min_modes, modal.max_modes), (1, 1));
    assert!(matches!(
        modal.modes[0].effects.as_slice(),
        [SpellEffectKind::DamageTarget { target, .. }]
            if target.kind == TargetKind::Creature
                && target.required_keywords == [Keyword::Flying]
    ));
    assert!(matches!(
        modal.modes[1].effects.as_slice(),
        [SpellEffectKind::DestroyTarget { target }]
            if target.kind == TargetKind::AnyPermanent
                && target.permanent_types.len() == 1
    ));

    let run_afoul_definition = registry.get("run_afoul").expect("Run Afoul is registered");
    assert!(run_afoul_definition.partial.is_none());
    let run_afoul = run_afoul_definition.primary_face();
    assert!(matches!(
        run_afoul.spell_effect.as_slice(),
        [SpellEffectKind::TargetPlayerSacrifices { target, filter }]
            if target.kind == TargetKind::OpponentPlayer
                && filter.kind == TargetKind::Creature
                && filter.required_keywords == [Keyword::Flying]
    ));
}
