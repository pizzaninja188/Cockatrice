//! Exact identity and the three printed loyalty abilities of Narset Transcendent.
use tricerules_cards::primitives::{
    AbilityCost, Amount, CardTypeFilter, CounterKind, EntersWithCountersAffected, SpellEffectKind,
    StaticAbilityDef, StaticEmblemEffect, ZoneCardFilter,
};
use tricerules_cards::AbilityPresentation;

#[test]
fn narset_exact_identity_and_all_three_loyalty_abilities() {
    const ID: &str = "narset_transcendent";

    let registry = tricerules_cards::registry::global();
    let card = registry
        .get(ID)
        .expect("Narset Transcendent must be registered");
    assert_eq!(registry.id_for_name("Narset Transcendent"), Some(ID));
    assert_eq!(card.id, ID);
    assert_eq!(card.name, "Narset Transcendent");
    assert_eq!(card.face_count(), 1);
    assert!(
        !card.commander_setup_only,
        "Narset Transcendent must be admitted as an ordinary mainboard card"
    );

    let face = card.primary_face();
    assert_eq!(face.name, "Narset Transcendent");
    assert_eq!(face.face_id.as_str(), "narset_transcendent");
    assert_eq!(face.mana_cost.to_string(), "{2}{W}{U}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Planeswalker", "Narset"]);
    assert_eq!(face.loyalty, Some(6));
    assert_eq!(face.static_abilities.len(), 1);
    assert!(face.static_abilities.iter().any(|ability| {
        ability.ability_id.as_str() == "intrinsic_loyalty"
            && ability.definition
                == StaticAbilityDef::EntersWithCounters {
                    affected: EntersWithCountersAffected::Self_,
                    counter: CounterKind::Loyalty,
                    amount: Amount::Fixed(6),
                    cast_cost_condition: None,
                }
    }));
    assert!(face.static_abilities.iter().all(|ability| {
        ability.definition != StaticAbilityDef::SourceLoyaltyAbilitiesAtInstantTiming
    }));

    assert_eq!(face.activated_abilities.len(), 3);

    let plus_one = &face.activated_abilities[0];
    assert_eq!(plus_one.ability_id.as_str(), "activated_01");
    assert_eq!(
        plus_one.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(plus_one.costs, [AbilityCost::Loyalty(1)]);
    assert!(plus_one.program.is_none());
    assert!(plus_one.targeting.is_none());
    assert_eq!(
        plus_one.effect,
        [SpellEffectKind::LookTopCardOptionalToHand {
            filter: ZoneCardFilter {
                card_type: Some(CardTypeFilter::Nonland),
                excluded_card_types: vec![CardTypeFilter::Creature],
                ..ZoneCardFilter::default()
            }
        }]
    );

    let minus_two = &face.activated_abilities[1];
    assert_eq!(minus_two.ability_id.as_str(), "activated_02");
    assert_eq!(
        minus_two.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(minus_two.costs, [AbilityCost::Loyalty(-2)]);
    assert!(minus_two.program.is_none());
    assert!(minus_two.targeting.is_none());
    assert_eq!(
        minus_two.effect,
        [SpellEffectKind::NextHandInstantOrSorceryGainsReboundThisTurn]
    );

    let minus_nine = &face.activated_abilities[2];
    assert_eq!(minus_nine.ability_id.as_str(), "activated_03");
    assert_eq!(
        minus_nine.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(minus_nine.costs, [AbilityCost::Loyalty(-9)]);
    assert!(minus_nine.program.is_none());
    assert!(minus_nine.targeting.is_none());
    assert!(matches!(
        minus_nine.effect.as_slice(),
        [SpellEffectKind::CreateStaticEmblem {
            emblem_id,
            display_name,
            effects,
        }] if emblem_id == "narset_transcendent_emblem"
            && display_name == "Narset Transcendent Emblem"
            && effects == &[StaticEmblemEffect::OpponentsCantCastNoncreatureSpells]
    ));
}
