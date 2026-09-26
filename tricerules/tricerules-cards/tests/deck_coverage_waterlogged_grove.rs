use tricerules_cards::primitives::{AbilityCost, PlayerRecipient, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Amount, CardRegistry};

#[test]
fn waterlogged_grove_registers_both_complete_abilities() {
    let registry = CardRegistry::global();
    assert_eq!(
        registry.id_for_name("Waterlogged Grove"),
        Some("waterlogged_grove")
    );
    let card = registry
        .get("waterlogged_grove")
        .expect("Waterlogged Grove");
    assert_eq!(card.name, "Waterlogged Grove");
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "waterlogged_grove");
    assert_eq!(face.types, ["Land"]);
    assert_eq!(face.mana_cost.to_string(), "");
    let [mana_ability, draw_ability] = face.activated_abilities.as_slice() else {
        panic!("Waterlogged Grove has two activated abilities");
    };

    assert_eq!(mana_ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        mana_ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(matches!(
        mana_ability.costs.as_slice(),
        [AbilityCost::Tap, AbilityCost::PayLife { amount: 1 }]
    ));
    let [SpellEffectKind::ProduceMana {
        options,
        restriction: None,
        conditional: None,
    }] = mana_ability.effect.as_slice()
    else {
        panic!("the first ability produces one of two colors of mana");
    };
    assert_eq!(options.len(), 2);
    assert_eq!(
        (
            options[0].w,
            options[0].u,
            options[0].b,
            options[0].r,
            options[0].g,
            options[0].c
        ),
        (0, 0, 0, 0, 1, 0)
    );
    assert_eq!(
        (
            options[1].w,
            options[1].u,
            options[1].b,
            options[1].r,
            options[1].g,
            options[1].c
        ),
        (0, 1, 0, 0, 0, 0)
    );

    assert_eq!(draw_ability.ability_id.as_str(), "activated_02");
    assert_eq!(
        draw_ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(matches!(
        draw_ability.costs.as_slice(),
        [AbilityCost::Mana(cost), AbilityCost::Tap, AbilityCost::SacrificeSelf]
            if cost.to_string() == "{1}"
    ));
    assert_eq!(
        draw_ability.effect,
        [SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        }]
    );
}
