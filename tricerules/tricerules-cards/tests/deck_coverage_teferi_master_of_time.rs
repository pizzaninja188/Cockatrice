//! Teferi, Master of Time's exact identity and complete printed ability set.
use tricerules_cards::primitives::{
    AbilityCost, Amount, CounterKind, EntersWithCountersAffected, SpellEffectKind, StaticAbilityDef,
};
use tricerules_cards::AbilityPresentation;

const TEFERI: &str = "teferi,_master_of_time";

#[test]
fn teferi_exact_identity_and_all_printed_abilities_are_registered() {
    let registry = tricerules_cards::registry::global();
    assert_eq!(registry.id_for_name("Teferi, Master of Time"), Some(TEFERI));
    let card = registry
        .get(TEFERI)
        .expect("Teferi, Master of Time must be registered");
    assert_eq!(card.id, TEFERI);
    assert_eq!(card.name, "Teferi, Master of Time");
    assert!(
        !card.commander_setup_only,
        "the full Teferi definition is admitted as an ordinary mainboard card"
    );

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "teferi_master_of_time");
    assert_eq!(face.mana_cost.to_string(), "{2}{U}{U}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Planeswalker", "Teferi"]);
    assert_eq!(face.loyalty, Some(3));
    let timing = face
        .static_abilities
        .iter()
        .filter(|ability| {
            ability.definition == StaticAbilityDef::SourceLoyaltyAbilitiesAtInstantTiming
        })
        .collect::<Vec<_>>();
    assert_eq!(
        timing.len(),
        1,
        "Teferi has exactly one self-source instant-loyalty timing static"
    );
    let timing = timing[0];
    assert_eq!(timing.ability_id.as_str(), "static_01");
    assert_eq!(
        timing.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        timing.definition,
        StaticAbilityDef::SourceLoyaltyAbilitiesAtInstantTiming
    );
    assert!(
        face.static_abilities.iter().any(|ability| {
            ability.ability_id.as_str() == "intrinsic_loyalty"
                && ability.definition
                    == StaticAbilityDef::EntersWithCounters {
                        affected: EntersWithCountersAffected::Self_,
                        counter: CounterKind::Loyalty,
                        amount: Amount::Fixed(3),
                        cast_cost_condition: None,
                    }
        }),
        "the printed loyalty value is materialized as its intrinsic entry counter"
    );
    assert_eq!(face.activated_abilities.len(), 3);

    let plus = &face.activated_abilities[0];
    assert_eq!(plus.ability_id.as_str(), "activated_01");
    assert_eq!(plus.presentation, AbilityPresentation::OracleLines(vec![2]));
    assert_eq!(plus.costs, [AbilityCost::Loyalty(1)]);
    assert!(plus.targeting.is_none());
    assert!(plus.effect.is_empty());
    let program = plus
        .program
        .as_ref()
        .expect("+1 draws, then resolves its mandatory discard choice");
    assert_eq!(program.body.steps.len(), 2);
    assert_eq!(program.body.steps[0].id, "draw_one");
    assert_eq!(program.body.steps[1].id, "discard_one");

    let minus_three = &face.activated_abilities[1];
    assert_eq!(minus_three.ability_id.as_str(), "activated_02");
    assert_eq!(
        minus_three.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(minus_three.costs, [AbilityCost::Loyalty(-3)]);
    assert!(matches!(
        minus_three.effect.as_slice(),
        [SpellEffectKind::PhaseOut { .. }]
    ));
    let targets = minus_three
        .targeting
        .as_ref()
        .expect("-3 targets one creature Teferi's controller does not control");
    assert_eq!(targets.groups.len(), 1);
    assert_eq!((targets.groups[0].min, targets.groups[0].max), (1, 1));
    assert_eq!(targets.groups[0].effect_indices, [0]);

    let minus_ten = &face.activated_abilities[2];
    assert_eq!(minus_ten.ability_id.as_str(), "activated_03");
    assert_eq!(
        minus_ten.presentation,
        AbilityPresentation::OracleLines(vec![4])
    );
    assert_eq!(minus_ten.costs, [AbilityCost::Loyalty(-10)]);
    assert_eq!(
        minus_ten.effect,
        [SpellEffectKind::TakeExtraTurns { count: 2 }]
    );
}
