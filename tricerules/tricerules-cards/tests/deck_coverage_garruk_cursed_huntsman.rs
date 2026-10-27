//! Exact Garruk identity and the ordered -3 loyalty ability.
use tricerules_cards::primitives::{
    AbilityCost, Amount, Color, CounterKind, CreatureScopeController, CreatureScopeFilter,
    EffectSubject, Keyword, PlaneswalkerCounterSubtype, PlayerRecipient, SpellEffectKind,
    StaticEmblemEffect, TargetKind, TriggerCondition,
};
use tricerules_cards::AbilityPresentation;

#[test]
fn garruk_exact_identity_and_destroy_then_draw() {
    let registry = tricerules_cards::registry::global();
    let id = registry
        .id_for_name("Garruk, Cursed Huntsman")
        .expect("Garruk, Cursed Huntsman must be registered");
    assert_eq!(id, "garruk,_cursed_huntsman");
    let card = registry.get(id).unwrap();
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.name, "Garruk, Cursed Huntsman");
    assert_eq!(face.face_id.as_str(), "garruk_cursed_huntsman");
    assert_eq!(face.mana_cost.to_string(), "{4}{B}{G}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Planeswalker", "Garruk"]);
    assert_eq!(face.loyalty, Some(5));

    // Stable identity keeps this assertion valid as the preceding 0 ability is added later.
    let ability = face
        .activated_abilities
        .iter()
        .find(|ability| ability.ability_id.as_str() == "activated_02")
        .expect("Garruk has the -3 loyalty ability");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(ability.costs, [AbilityCost::Loyalty(-3)]);
    assert!(ability.program.is_none());
    let [group] = ability
        .targeting
        .as_ref()
        .expect("required creature target")
        .groups
        .as_slice()
    else {
        panic!("one target group")
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.effect_indices, [0]);
    assert!(matches!(ability.effect.as_slice(), [
        SpellEffectKind::Destroy { subject: EffectSubject::Chosen(target) },
        SpellEffectKind::Draw { who: PlayerRecipient::Controller, count: Amount::Fixed(1) },
    ] if target.kind == TargetKind::Creature));
}

#[test]
fn garruk_zero_creates_two_exact_wolves_with_a_death_trigger() {
    let registry = tricerules_cards::registry::global();
    let card = registry
        .get("garruk,_cursed_huntsman")
        .expect("Garruk, Cursed Huntsman must be registered");
    let ability = card
        .primary_face()
        .activated_abilities
        .iter()
        .find(|ability| ability.ability_id.as_str() == "activated_01")
        .expect("Garruk has the 0 loyalty ability");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(ability.costs, [AbilityCost::Loyalty(0)]);
    assert!(ability.program.is_none() && ability.targeting.is_none());
    assert!(
        matches!(ability.effect.as_slice(), [SpellEffectKind::CreateTokens {
        token, count: Amount::Fixed(2), who: PlayerRecipient::Controller,
        tapped: false, sacrifice_timing: None,
    }] if token == "wolf_bg_2_2_garruk_loyalty")
    );

    let token_id = "wolf_bg_2_2_garruk_loyalty";
    assert!(registry.is_token(token_id));
    let wolf = registry
        .get(token_id)
        .expect("Garruk's Wolf token definition")
        .primary_face();
    assert_eq!(wolf.name, "Wolf");
    assert_eq!(wolf.face_id.as_str(), "wolf");
    assert_eq!(wolf.types, ["Creature", "Wolf"]);
    assert_eq!(wolf.colors(), [Color::Black, Color::Green]);
    assert_eq!((wolf.power, wolf.toughness), (Some(2), Some(2)));
    let [death] = wolf.triggered_abilities.as_slice() else {
        panic!("one Wolf death trigger")
    };
    assert_eq!(death.ability_id.as_str(), "triggered_01");
    assert_eq!(death.presentation, AbilityPresentation::Fallback);
    assert_eq!(death.trigger, TriggerCondition::WhenSelfDies);
    assert!(death.program.is_none() && death.targeting.is_none());
    assert!(matches!(
        death.effect.as_slice(),
        [SpellEffectKind::PutCountersAllPlaneswalkers {
            counter: CounterKind::Loyalty,
            count: Amount::Fixed(1),
            exclude_self: false,
            subtype: Some(PlaneswalkerCounterSubtype::Garruk),
        }]
    ));
}

#[test]
fn garruk_minus_six_has_the_printed_emblem_ability_shape() {
    let registry = tricerules_cards::registry::global();
    let card = registry
        .get("garruk,_cursed_huntsman")
        .expect("Garruk must be registered");
    let ability = card
        .primary_face()
        .activated_abilities
        .iter()
        .find(|ability| ability.ability_id.as_str() == "activated_03")
        .expect("Garruk has the -6 loyalty ability");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(ability.costs, [AbilityCost::Loyalty(-6)]);
    assert!(ability.program.is_none() && ability.targeting.is_none());
    let [SpellEffectKind::CreateStaticEmblem {
        emblem_id,
        display_name,
        effects,
    }] = ability.effect.as_slice()
    else {
        panic!("the ultimate creates one static emblem")
    };
    assert_eq!(emblem_id, "garruk_cursed_huntsman_emblem");
    assert_eq!(display_name, "Garruk, Cursed Huntsman Emblem");
    let scope = CreatureScopeFilter {
        controller: Some(CreatureScopeController::YouControl),
        ..Default::default()
    };
    assert_eq!(
        effects,
        &[
            StaticEmblemEffect::CreaturePt {
                filter: scope.clone(),
                power: 3,
                toughness: 3
            },
            StaticEmblemEffect::CreatureKeyword {
                filter: scope,
                keyword: Keyword::Trample
            },
        ]
    );
}
