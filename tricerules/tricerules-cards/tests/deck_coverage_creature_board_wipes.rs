use tricerules_cards::primitives::{
    BattlefieldPermanentFilter, CardTypeFilter, RelativePlayerSet, TargetFilter,
};
use tricerules_cards::{
    Amount, BattlefieldAggregate, CardRegistry, Color, Layout, SpellCostModifier, SpellEffectKind,
};

#[test]
fn creature_board_wipe_cards_have_complete_typed_definitions() {
    let registry = CardRegistry::global();

    let act = registry
        .get("blasphemous_act")
        .expect("Blasphemous Act should be registered");
    assert_eq!(act.name, "Blasphemous Act");
    assert_eq!(act.layout, Layout::Normal);
    assert_eq!(act.face_count(), 1);
    let face = act.primary_face();
    assert_eq!(face.face_id.as_str(), "blasphemous_act");
    assert_eq!(face.mana_cost.to_string(), "{8}{R}");
    assert_eq!(face.types, ["Sorcery"]);
    assert_eq!(face.colors(), vec![Color::Red]);
    assert_battlefield_creature_reduction(face.cost_modifiers.as_slice());
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::DamageAll {
            amount: Amount::Fixed(13),
            players: RelativePlayerSet::All,
            kind: TargetFilter::default_creature(),
        }]
    );
    assert!(face.activated_abilities.is_empty());
    assert!(face.triggered_abilities.is_empty());
    assert!(face.targeting.is_none());

    let horde = registry
        .get("vanquish_the_horde")
        .expect("Vanquish the Horde should be registered");
    assert_eq!(horde.name, "Vanquish the Horde");
    assert_eq!(horde.layout, Layout::Normal);
    assert_eq!(horde.face_count(), 1);
    let face = horde.primary_face();
    assert_eq!(face.face_id.as_str(), "vanquish_the_horde");
    assert_eq!(face.mana_cost.to_string(), "{6}{W}{W}");
    assert_eq!(face.types, ["Sorcery"]);
    assert_eq!(face.colors(), vec![Color::White]);
    assert_battlefield_creature_reduction(face.cost_modifiers.as_slice());
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::DestroyAll {
            kind: TargetFilter::default_creature(),
            prevent_regeneration: false,
        }]
    );
    assert!(face.activated_abilities.is_empty());
    assert!(face.triggered_abilities.is_empty());
    assert!(face.targeting.is_none());
}

fn assert_battlefield_creature_reduction(modifiers: &[SpellCostModifier]) {
    let [SpellCostModifier::BattlefieldCountGenericReduction {
        amount_per_match,
        filter,
        aggregate,
    }] = modifiers
    else {
        panic!("expected one reduction per creature on the battlefield");
    };
    assert_eq!(*amount_per_match, 1);
    assert_eq!(
        filter,
        &BattlefieldPermanentFilter {
            token: None,
            any_of: None,
            controllers: RelativePlayerSet::All,
            card_type: Some(CardTypeFilter::Creature),
            color: None,
            name: None,
            required_subtypes: Vec::new(),
            exclude_source: false,
        }
    );
    assert_eq!(*aggregate, BattlefieldAggregate::Count);
}
