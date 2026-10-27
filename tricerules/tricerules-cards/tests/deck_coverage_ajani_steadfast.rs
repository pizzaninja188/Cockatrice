//! Exact Ajani identity and his first loyalty ability.
use tricerules_cards::primitives::{
    AbilityCost, EffectSubject, Keyword, SpellEffectKind, StaticEmblemEffect, TargetKind,
};
use tricerules_cards::AbilityPresentation;

#[test]
fn ajani_exact_identity_and_optional_plus_one() {
    let registry = tricerules_cards::registry::global();
    let id = registry
        .id_for_name("Ajani Steadfast")
        .expect("Ajani Steadfast must be registered");
    assert_eq!(id, "ajani_steadfast");
    let card = registry.get(id).unwrap();
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.name, "Ajani Steadfast");
    assert_eq!(face.face_id.as_str(), "ajani_steadfast");
    assert_eq!(face.mana_cost.to_string(), "{3}{W}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Planeswalker", "Ajani"]);
    assert_eq!(face.loyalty, Some(4));

    let plus = face
        .activated_abilities
        .first()
        .expect("Ajani has a +1 ability");
    assert_eq!(plus.ability_id.as_str(), "activated_01");
    assert_eq!(plus.presentation, AbilityPresentation::OracleLines(vec![1]));
    assert_eq!(plus.costs, [AbilityCost::Loyalty(1)]);
    assert!(plus.program.is_none());
    let [group] = plus
        .targeting
        .as_ref()
        .expect("shared optional creature target")
        .groups
        .as_slice()
    else {
        panic!("one shared target group");
    };
    assert_eq!((group.min, group.max), (0, 1));
    assert_eq!(group.effect_indices, [0, 1]);
    assert!(matches!(plus.effect.as_slice(), [
        SpellEffectKind::PumpTarget { power: 1, toughness: 1, scale: None, subject: EffectSubject::Chosen(pump) },
        SpellEffectKind::GrantKeywords { subject: EffectSubject::Chosen(grant), keywords },
    ] if pump.kind == TargetKind::Creature && grant.kind == TargetKind::Creature
        && pump == grant && keywords == &[Keyword::FirstStrike, Keyword::Vigilance, Keyword::Lifelink]));
}

#[test]
fn ajani_minus_two_has_the_printed_cost_and_one_untargeted_instruction() {
    let card = tricerules_cards::registry::global()
        .get("ajani_steadfast")
        .expect("Ajani Steadfast must be registered");
    let minus = card
        .primary_face()
        .activated_abilities
        .get(1)
        .expect("Ajani has a -2 ability");
    assert_eq!(minus.ability_id.as_str(), "activated_02");
    assert_eq!(
        minus.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(minus.costs, [AbilityCost::Loyalty(-2)]);
    assert!(minus.program.is_none());
    assert!(minus.targeting.is_none());
    assert_eq!(
        minus.effect,
        [SpellEffectKind::PutCountersOnCreaturesAndOtherPlaneswalkers],
        "both kinds of counters are one instruction"
    );
}

#[test]
fn ajani_minus_seven_has_the_printed_cost_and_creates_one_emblem() {
    let card = tricerules_cards::registry::global()
        .get("ajani_steadfast")
        .expect("Ajani Steadfast must be registered");
    let ultimate = card
        .primary_face()
        .activated_abilities
        .get(2)
        .expect("Ajani has a -7 ability");
    assert_eq!(ultimate.ability_id.as_str(), "activated_03");
    assert_eq!(
        ultimate.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(ultimate.costs, [AbilityCost::Loyalty(-7)]);
    assert!(ultimate.program.is_none());
    assert!(ultimate.targeting.is_none());
    assert!(
        matches!(ultimate.effect.as_slice(), [SpellEffectKind::CreateStaticEmblem {
        emblem_id, display_name, effects,
    }] if emblem_id == "ajani_steadfast_emblem" && display_name == "Ajani Steadfast Emblem"
        && effects == &[StaticEmblemEffect::PreventAllButOneDamageToYouAndYourPlaneswalkers])
    );
}
