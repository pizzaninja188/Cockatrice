use tricerules_cards::primitives::{
    Amount, EffectSubject, GameCondition, PermanentTypeFilter, RelativePlayerSet,
    SpellCostModifier, SpellEffectKind, StaticAbilityDef, TargetKind,
};
use tricerules_cards::{AbilityPresentation, CardRegistry};

#[test]
fn issue_misc47_uncounterable_removal_cards_are_registered() {
    let registry = CardRegistry::global();
    for (id, name, mana, types) in [
        ("long_goodbye", "Long Goodbye", "{1}{B}", &["Instant"][..]),
        (
            "slice_from_the_shadows",
            "Slice from the Shadows",
            "{X}{B}",
            &["Instant"][..],
        ),
        (
            "suspicious_detonation",
            "Suspicious Detonation",
            "{4}{R}",
            &["Sorcery"][..],
        ),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing registered card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id);
        assert_eq!(face.mana_cost.to_string(), mana);
        assert_eq!(face.types, types);
    }
}

#[test]
fn issue_misc47_maps_each_uncounterable_spell_to_its_complete_effect() {
    let registry = CardRegistry::global();

    let long_goodbye = registry
        .get("long_goodbye")
        .expect("Long Goodbye")
        .primary_face();
    assert_eq!(
        long_goodbye.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        long_goodbye.static_abilities[0].definition,
        StaticAbilityDef::SpellCannotBeCountered
    );
    let [SpellEffectKind::Destroy {
        subject: EffectSubject::Chosen(filter),
    }] = long_goodbye.spell_effect.as_slice()
    else {
        panic!(
            "Long Goodbye must destroy one chosen permanent: {:?}",
            long_goodbye.spell_effect
        );
    };
    assert_eq!(filter.kind, TargetKind::AnyPermanent);
    assert_eq!(
        filter.permanent_types,
        [
            PermanentTypeFilter::Creature,
            PermanentTypeFilter::Planeswalker
        ]
    );
    assert_eq!(filter.max_mana_value, Some(3));
    assert_eq!(
        long_goodbye.targeting.as_ref().unwrap().groups[0].effect_indices,
        [0]
    );

    let slice = registry
        .get("slice_from_the_shadows")
        .expect("Slice from the Shadows")
        .primary_face();
    assert_eq!(
        slice.static_abilities[0].definition,
        StaticAbilityDef::SpellCannotBeCountered
    );
    assert!(matches!(
        slice.spell_effect.as_slice(),
        [SpellEffectKind::PumpTarget {
            power: 0,
            toughness: 0,
            scale: Some(scale),
            subject: EffectSubject::Chosen(filter),
        }] if scale.basis == tricerules_cards::primitives::PtScaleBasis::Amount(Amount::X)
            && scale.power_per_unit == -1
            && scale.toughness_per_unit == -1
            && filter.kind == TargetKind::Creature
    ));

    let detonation = registry
        .get("suspicious_detonation")
        .expect("Suspicious Detonation")
        .primary_face();
    assert_eq!(
        detonation.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        detonation.static_abilities[0].definition,
        StaticAbilityDef::SpellCannotBeCountered
    );
    assert!(matches!(
        detonation.cost_modifiers.as_slice(),
        [SpellCostModifier::ConditionalGenericReduction { amount: 3, condition }]
            if matches!(condition,
                GameCondition::PermanentsSacrificedThisTurn {
                    players: RelativePlayerSet::Controller,
                    permanent_type: Some(PermanentTypeFilter::Artifact),
                    min: Some(1),
                    max: None,
                })
    ));
    assert!(matches!(
        detonation.spell_effect.as_slice(),
        [SpellEffectKind::DamageTarget { amount: Amount::Fixed(4), target }]
            if target.kind == TargetKind::Creature
    ));
    assert_eq!(
        detonation.targeting.as_ref().unwrap().groups[0].effect_indices,
        [0]
    );
}

#[test]
fn issue_misc47_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "long_goodbye",
            "Long Goodbye",
            "74e98d941ed5a7fc3cf1393c971173bfc35db6a0e18e4e39a20643e118153882",
        ),
        (
            "slice_from_the_shadows",
            "Slice from the Shadows",
            "1e0bb6e3bc1dfe30089a8986b267069ce9907eb11ad5048340f1cb5c7a04dcdd",
        ),
        (
            "suspicious_detonation",
            "Suspicious Detonation",
            "d2e27cccb31be3f6536bfb358190a43e52dd335f307f30fb64322c03dfd7da6d",
        ),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[1], name);
        assert_eq!(
            fields[4], fingerprint,
            "pinned Oracle fingerprint drift for {id}"
        );
    }
}
