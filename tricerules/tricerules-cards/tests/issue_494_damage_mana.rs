use tricerules_cards::primitives::{
    AbilityCost, AbilitySourceZone, ActivatedAbilityDef, ActivationTiming, Amount, PlayerRecipient,
    SpellEffectKind,
};
use tricerules_cards::{AbilityPresentation, Layout, ManaAmount};

fn mana_option(c: u32, r: u32, g: u32, u: u32) -> ManaAmount {
    ManaAmount {
        c,
        r,
        g,
        u,
        ..Default::default()
    }
}

fn assert_mana_ability(
    ability: &ActivatedAbilityDef,
    index: usize,
    options: &[ManaAmount],
    damage: bool,
) {
    assert_eq!(
        ability.ability_id.as_str(),
        format!("activated_{:02}", index + 1)
    );
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![(index + 1) as u16])
    );
    assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(ability.costs, [AbilityCost::Tap]);
    assert!(ability.cost_modifiers.is_empty());
    assert_eq!(ability.timing, ActivationTiming::Normal);
    assert!(ability.conditions.is_empty());
    assert!(ability.activation_limit.is_none());
    assert!(ability.targeting.is_none());

    let actual_options = match ability.effect.as_slice() {
        [SpellEffectKind::ProduceMana {
            commander_color_identity: false,
            options,
            restriction: None,
            conditional: None,
        }] if !damage => options,
        [SpellEffectKind::ProduceMana {
            commander_color_identity: false,
            options,
            restriction: None,
            conditional: None,
        }, SpellEffectKind::DamagePlayer {
            amount: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
        }] if damage => options,
        _ => panic!("incorrect typed effect list for activated ability {index}"),
    };
    assert_eq!(actual_options, options);
}

#[test]
fn issue_494_registers_exact_complete_definitions() {
    let registry = tricerules_cards::registry::global();
    let cases = [
        (
            "talisman_of_impulse",
            "Talisman of Impulse",
            "talisman_of_impulse",
            "{2}",
            ["Artifact"],
            vec![mana_option(1, 0, 0, 0)],
            vec![mana_option(0, 1, 0, 0), mana_option(0, 0, 1, 0)],
        ),
        (
            "karplusan_forest",
            "Karplusan Forest",
            "karplusan_forest",
            "",
            ["Land"],
            vec![mana_option(1, 0, 0, 0)],
            vec![mana_option(0, 1, 0, 0), mana_option(0, 0, 1, 0)],
        ),
        (
            "yavimaya_coast",
            "Yavimaya Coast",
            "yavimaya_coast",
            "",
            ["Land"],
            vec![mana_option(1, 0, 0, 0)],
            vec![mana_option(0, 0, 1, 0), mana_option(0, 0, 0, 1)],
        ),
    ];

    for (id, name, face_id, mana_cost, types, colorless, colored) in cases {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {name}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        assert_eq!(definition.layout, Layout::Normal);
        assert_eq!(definition.face_count(), 1);

        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), face_id);
        assert_eq!(face.mana_cost.to_string(), mana_cost);
        assert_eq!(face.types, types);
        assert!(face.supertypes.is_empty());
        assert!(face.colors().is_empty());
        assert_eq!(face.power, None);
        assert_eq!(face.toughness, None);
        assert!(face.keywords.is_empty());
        assert!(face.spell_effect.is_empty());
        assert!(face.targeting.is_none());
        assert!(face.modal_spell.is_none());
        assert!(face.custom_effect.is_none());
        assert!(face.triggered_abilities.is_empty());
        assert!(face.static_abilities.is_empty());
        assert_eq!(face.activated_abilities.len(), 2);
        assert_mana_ability(&face.activated_abilities[0], 0, &colorless, false);
        assert_mana_ability(&face.activated_abilities[1], 1, &colored, true);
    }
}
