use tricerules_cards::primitives::{
    EffectContext, RelativePlayerSet, SpellEffectKind, TargetFilter,
};

#[test]
fn filtered_mass_tap_loads_and_validates() {
    for source in [
        "TapAll(players: Opponents, filter: (kind: Creature))",
        "TapAll(players: All, filter: (kind: Creature, not_color: Some(White)))",
        "TapAll(players: Controller, filter: (kind: AnyPermanent, permanent_types: [Land]))",
    ] {
        let effect: SpellEffectKind = ron::from_str(source).expect("filtered mass tap loads");
        effect.validate(EffectContext::Spell).expect("valid filter");
        assert!(!effect.needs_target(), "mass selection is not targeting");
        SpellEffectKind::validate_list(&[effect]).expect("filtered mass tap validates");
    }
}

#[test]
fn mass_tap_defaults_to_creatures_without_retaining_the_old_variant() {
    let effect: SpellEffectKind = ron::from_str("TapAll(players: Controller)").unwrap();
    assert_eq!(
        effect,
        SpellEffectKind::TapAll {
            players: RelativePlayerSet::Controller,
            filter: TargetFilter::default_creature(),
        }
    );
    assert!(ron::from_str::<SpellEffectKind>("TapAllCreatures(players: Controller)").is_err());
}

#[test]
fn tap_and_untap_reject_the_same_invalid_mass_filters() {
    for filter in [
        "(kind: AnyPlayer)",
        "(kind: AnyTarget)",
        "(kind: Creature, controller: You)",
        "(kind: Creature, excluded_objects: [Source])",
        "(kind: Creature, is_color: Some(White), not_color: Some(White))",
        "(any_of: Some([(kind: Creature), (kind: AnyPermanent, controller: Opponent)]))",
        "(any_of: Some([(kind: Creature), (kind: AnyPlayer)]))",
        "(kind: Creature, not_color: Some(White), any_of: Some([(kind: Creature), (kind: AnyPermanent)]))",
    ] {
        for kind in ["TapAll", "UntapAll"] {
            let source = format!("{kind}(players: All, filter: {filter})");
            let effect: SpellEffectKind = ron::from_str(&source).expect("valid RON shape");
            assert!(effect.validate(EffectContext::Spell).is_err(), "{source}");
        }
    }
}

#[test]
fn overlapping_characteristic_branches_are_valid_mass_selection() {
    for kind in ["TapAll", "UntapAll"] {
        let effect: SpellEffectKind = ron::from_str(&format!(
            "{kind}(players: All, filter: (any_of: Some([(kind: Creature), (kind: AnyPermanent, permanent_types: [Artifact])])))"
        )).unwrap();
        effect
            .validate(EffectContext::Spell)
            .expect("valid disjunction");
    }
}
