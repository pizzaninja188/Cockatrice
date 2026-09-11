use tricerules_cards::primitives::{TargetKind, TargetingDef};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, Amount, CardRegistry, LibraryPartitionKind, ManaCost,
    SpellEffectKind, TriggerCondition,
};

fn generated_face(id: &str) -> &'static tricerules_cards::CardFace {
    CardRegistry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing generated card {id}"))
        .primary_face()
}

fn assert_etb(id: &str, line: u16) -> &'static tricerules_cards::TriggeredAbilityDef {
    let face = generated_face(id);
    assert!(
        face.types.iter().any(|card_type| card_type == "Land"),
        "{id}"
    );
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("{id} must have exactly one triggered ability");
    };
    assert_eq!(ability.ability_id.as_str(), "triggered_01", "{id}");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![line]),
        "{id}"
    );
    assert_eq!(
        ability.trigger,
        TriggerCondition::WhenSelfEntersBattlefield,
        "{id}"
    );
    ability
}

#[test]
fn issue_254_registers_all_forty_eight_lands_with_exact_etb_effects() {
    for id in [
        "illegitimate_business",
        "foot_headquarters",
        "avengers_hangar",
        "hells_kitchen",
        "tcri_building",
        "los_diablos_missile_base",
        "stark_industries",
        "fisk_tower",
        "pym_technologies",
        "a.i.m._labs",
        "birnin_zana_plaza",
        "scoured_barrens",
        "subterranean_cavern",
        "asgardian_citadel",
        "mutant_town",
    ] {
        assert_eq!(
            assert_etb(id, 2).effect,
            [SpellEffectKind::GainLife {
                amount: Amount::Fixed(1)
            }],
            "{id}"
        );
    }

    for (id, line) in [
        ("temple_of_deceit", 2),
        ("temple_of_abandon", 2),
        ("temple_of_triumph", 2),
        ("temple_of_epiphany", 2),
        ("temple_of_malice", 2),
        ("temple_of_mystery", 2),
        ("rumble_arena", 2),
        ("temple_of_enlightenment", 2),
        ("temple_of_malady", 2),
        ("temple_of_plenty", 2),
        ("temple_of_silence", 2),
        ("crystal_grotto", 1),
    ] {
        assert_eq!(
            assert_etb(id, line).effect,
            [SpellEffectKind::Scry {
                count: Amount::Fixed(1)
            }],
            "{id}"
        );
    }

    for (id, line) in [
        ("undercity_sewers", 3),
        ("shadowy_backstreet", 3),
        ("conduit_pylons", 1),
        ("hidden_grotto", 1),
        ("underground_mortuary", 3),
        ("elegant_parlor", 3),
        ("surveillance_room", 1),
        ("commercial_district", 3),
        ("hedge_maze", 3),
        ("meticulous_archive", 3),
        ("lush_portico", 3),
    ] {
        assert_eq!(
            assert_etb(id, line).effect,
            [SpellEffectKind::LibraryPartition {
                count: 1,
                top_min: 0,
                top_max: None,
                kind: LibraryPartitionKind::Surveil,
            }],
            "{id}"
        );
    }

    for id in [
        "lonely_arroyo",
        "jagged_barrens",
        "eroded_canyon",
        "bristling_backwoods",
        "festering_gulch",
        "lush_oasis",
        "creosote_heath",
        "abraded_bluffs",
        "soured_springs",
        "forlorn_flats",
    ] {
        let ability = assert_etb(id, 2);
        let [SpellEffectKind::DamageTarget { amount, target }] = ability.effect.as_slice() else {
            panic!("{id} must deal targeted damage");
        };
        assert_eq!(*amount, Amount::Fixed(1), "{id}");
        assert_eq!(target.kind, TargetKind::OpponentPlayer, "{id}");
        let Some(TargetingDef { groups }) = &ability.targeting else {
            panic!("{id} must publish targeting");
        };
        assert_eq!(groups.len(), 1, "{id}");
        assert_eq!((groups[0].min, groups[0].max), (1, 1), "{id}");
        assert_eq!(groups[0].effect_indices, [0], "{id}");
    }
}

#[test]
fn issue_254_paid_five_color_lands_preserve_cost_identity_and_option_order() {
    for (id, line) in [
        ("rumble_arena", 4),
        ("crystal_grotto", 3),
        ("conduit_pylons", 3),
        ("hidden_grotto", 3),
        ("surveillance_room", 3),
    ] {
        let face = generated_face(id);
        let ability = face
            .activated_abilities
            .iter()
            .find(|ability| ability.ability_id.as_str() == "activated_02")
            .unwrap_or_else(|| panic!("{id} missing activated_02"));
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![line]),
            "{id}"
        );
        assert_eq!(
            ability.costs,
            [
                AbilityCost::Mana(ManaCost::parse("{1}").unwrap()),
                AbilityCost::Tap,
            ],
            "{id}"
        );
        let options = ability.mana_options().expect("typed mana options");
        assert_eq!(options.len(), 5, "{id}");
        assert_eq!(
            options
                .iter()
                .map(|mana| (mana.w, mana.u, mana.b, mana.r, mana.g, mana.c))
                .collect::<Vec<_>>(),
            [
                (1, 0, 0, 0, 0, 0),
                (0, 1, 0, 0, 0, 0),
                (0, 0, 1, 0, 0, 0),
                (0, 0, 0, 1, 0, 0),
                (0, 0, 0, 0, 1, 0),
            ],
            "{id}"
        );
    }
}
