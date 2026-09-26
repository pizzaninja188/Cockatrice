use tricerules_cards::primitives::{AbilityCost, PlayerRecipient, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, AbilitySourceZone, Amount, CardRegistry};

#[test]
fn mind_stone_registers_its_colorless_mana_and_sacrifice_draw_abilities() {
    let registry = CardRegistry::global();
    let card = registry
        .get("mind_stone")
        .expect("Mind Stone registry definition");

    assert_eq!(card.name, "Mind Stone");
    assert_eq!(registry.id_for_name("Mind Stone"), Some("mind_stone"));
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "mind_stone");
    assert_eq!(face.mana_cost.to_string(), "{2}");
    assert_eq!(face.types, ["Artifact"]);
    assert!(face.colors().is_empty());
    assert!(face.spell_effect.is_empty());
    assert_eq!(face.activated_abilities.len(), 2);

    let [mana_ability, draw_ability] = face.activated_abilities.as_slice() else {
        panic!("Mind Stone has two activated abilities");
    };
    assert_eq!(mana_ability.ability_id.as_str(), "activated_01");
    assert_eq!(mana_ability.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(
        mana_ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(mana_ability.costs, [AbilityCost::Tap]);
    let [SpellEffectKind::ProduceMana {
        options,
        restriction: None,
        conditional: None,
    }] = mana_ability.effect.as_slice()
    else {
        panic!("Mind Stone's first ability produces mana");
    };
    assert_eq!(options.len(), 1);
    assert_eq!(
        (
            options[0].w,
            options[0].u,
            options[0].b,
            options[0].r,
            options[0].g,
            options[0].c
        ),
        (0, 0, 0, 0, 0, 1)
    );

    assert_eq!(draw_ability.ability_id.as_str(), "activated_02");
    assert_eq!(draw_ability.source_zone, AbilitySourceZone::Battlefield);
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
