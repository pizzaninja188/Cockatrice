use tricerules_cards::primitives::{PlayerRecipient, SpellEffectKind};
use tricerules_cards::{AbilityCost, AbilityPresentation, Amount, CardRegistry};

#[test]
fn issue_255_registers_all_nine_lands_with_the_exact_draw_ability() {
    let registry = CardRegistry::global();
    for id in [
        "airship_engine_room",
        "north_pole_gates",
        "meditation_pools",
        "boiling_rock_prison",
        "omashu_city",
        "foggy_bottom_swamp",
        "kyoshi_village",
        "misty_palms_oasis",
        "serpents_pass",
    ] {
        let face = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing generated card {id}"))
            .primary_face();
        assert!(
            face.types.iter().any(|card_type| card_type == "Land"),
            "{id}"
        );

        let [_, ability] = face.activated_abilities.as_slice() else {
            panic!("{id} must have its mana and draw abilities");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_02", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![3]),
            "{id}"
        );
        assert!(
            matches!(
                ability.costs.as_slice(),
                [AbilityCost::Mana(cost), AbilityCost::Tap, AbilityCost::SacrificeSelf]
                    if cost.to_string() == "{4}"
            ),
            "{id}"
        );
        assert_eq!(
            ability.effect,
            [SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }],
            "{id}"
        );
        assert!(ability.targeting.is_none(), "{id}");
    }
}
