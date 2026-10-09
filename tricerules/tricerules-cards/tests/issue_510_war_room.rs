use tricerules_cards::primitives::{AbilityCost, PlayerRecipient, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Amount, CardRegistry};

#[test]
fn war_room_registers_the_complete_exact_card_and_two_mapped_abilities() {
    let registry = tricerules_cards::registry::global();
    assert_eq!(registry.id_for_name("War Room"), Some("war_room"));
    let card = registry.get("war_room").unwrap();
    assert_eq!(card.name, "War Room");
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "war_room");
    assert_eq!(face.types, ["Land"]);
    assert_eq!(face.mana_cost.to_string(), "");
    let [mana, draw] = face.activated_abilities.as_slice() else {
        panic!("two complete abilities")
    };
    assert_eq!(mana.ability_id.as_str(), "activated_01");
    assert_eq!(mana.presentation, AbilityPresentation::OracleLines(vec![1]));
    assert_eq!(mana.costs, [AbilityCost::Tap]);
    assert!(mana.is_mana_ability());
    let [SpellEffectKind::ProduceMana {
        options,
        commander_color_identity: false,
        restriction: None,
        conditional: None,
    }] = mana.effect.as_slice()
    else {
        panic!("ordinary C")
    };
    assert_eq!(
        options,
        &[tricerules_cards::ManaAmount {
            c: 1,
            ..Default::default()
        }]
    );
    assert_eq!(draw.ability_id.as_str(), "activated_02");
    assert_eq!(draw.presentation, AbilityPresentation::OracleLines(vec![2]));
    assert!(matches!(draw.costs.as_slice(),
        [AbilityCost::Mana(cost), AbilityCost::Tap, AbilityCost::PayCommanderColorIdentityLife]
            if cost.to_string() == "{3}"));
    assert!(!draw.is_mana_ability());
    assert_eq!(
        draw.effect,
        [SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1)
        }]
    );
    assert!(draw.targeting.is_none());
    assert!(draw.conditions.is_empty());
    assert!(draw.activation_limit.is_none());
}

#[test]
fn dynamic_commander_life_cost_does_not_weaken_fixed_zero_life_validation() {
    let source = include_str!("../data/war_room.ron");
    assert!(CardRegistry::from_chunks_and_tokens(&[source], &[]).is_ok());
    let zero = source.replace("PayCommanderColorIdentityLife", "PayLife(amount: 0)");
    let result = CardRegistry::from_chunks_and_tokens(&[&zero], &[]);
    assert!(matches!(result,
        Err(tricerules_cards::registry::RegistryError::InvalidCard { reason, .. })
            if reason.contains("positive amount")));
    let positive = source.replace("PayCommanderColorIdentityLife", "PayLife(amount: 1)");
    assert!(CardRegistry::from_chunks_and_tokens(&[&positive], &[]).is_ok());
}
