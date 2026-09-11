use tricerules_cards::primitives::{
    LifeAmount, PermanentTypeFilter, PlayerRecipient, SpellEffectKind,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, Amount, CardRegistry, CastTriggerPlayer,
    LibraryPartitionKind, TriggerCondition,
};

#[test]
fn issue_252_registers_all_nineteen_generated_cards_with_typed_abilities() {
    let registry = CardRegistry::global();

    for (id, oracle_line) in [
        ("a.i.m._synthoids", 1),
        ("imperious_inkmage", 2),
        ("sterling_hound", 1),
        ("wreckage_wickerfolk", 2),
    ] {
        let face = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing {id}"))
            .primary_face();
        let [ability] = face.triggered_abilities.as_slice() else {
            panic!("{id} must have one generated trigger");
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![oracle_line])
        );
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::LibraryPartition {
                count: 2,
                top_min: 0,
                top_max: None,
                kind: LibraryPartitionKind::Surveil,
            }]
        );
    }

    for (id, ability_index, oracle_line) in [
        ("buzz_bots", 0, 2),
        ("outlaw_medic", 0, 2),
        ("pelakka_wurm", 1, 3),
        ("summit_sentinel", 0, 1),
    ] {
        let face = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing {id}"))
            .primary_face();
        let ability = &face.triggered_abilities[ability_index];
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![oracle_line])
        );
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfDies);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }]
        );
    }

    for (id, oracle_line) in [
        ("glidedive_duo", 2),
        ("skirmish_rhino", 2),
        ("sneering_shadewriter", 2),
        ("vampire_spawn", 1),
    ] {
        let face = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing {id}"))
            .primary_face();
        let [ability] = face.triggered_abilities.as_slice() else {
            panic!("{id} must have one generated trigger");
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![oracle_line])
        );
        assert_eq!(
            ability.effect,
            [
                SpellEffectKind::LoseLife {
                    amount: LifeAmount::Fixed(2),
                    who: PlayerRecipient::EachOpponent,
                },
                SpellEffectKind::GainLife {
                    amount: Amount::Fixed(2),
                },
            ]
        );
    }

    for (id, oracle_line) in [
        ("dazzling_angel", 2),
        ("hinterland_sanctifier", 1),
        ("lifecreed_duo", 2),
        ("virulent_emissary", 2),
    ] {
        let face = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing {id}"))
            .primary_face();
        let [ability] = face.triggered_abilities.as_slice() else {
            panic!("{id} must have one generated trigger");
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![oracle_line])
        );
        let TriggerCondition::WheneverPermanentEntersBattlefield {
            controller,
            filter,
            creature_filter,
        } = &ability.trigger
        else {
            panic!("{id} must watch creature entries");
        };
        assert_eq!(*controller, CastTriggerPlayer::Controller);
        assert_eq!(filter.permanent_type, Some(PermanentTypeFilter::Creature));
        assert!(filter.exclude_source);
        assert!(creature_filter.is_none());
        assert_eq!(
            ability.effect,
            [SpellEffectKind::GainLife {
                amount: Amount::Fixed(1),
            }]
        );
    }

    for (id, oracle_line) in [
        ("great_forest_druid", 1),
        ("oasis_gardener", 2),
        ("three_tree_rootweaver", 1),
    ] {
        let face = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing {id}"))
            .primary_face();
        let [ability] = face.activated_abilities.as_slice() else {
            panic!("{id} must have one generated mana ability");
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![oracle_line])
        );
        assert_eq!(ability.costs, [AbilityCost::Tap]);
        let options = ability.mana_options().expect("typed mana ability");
        assert_eq!(options.len(), 5);
        assert!(options
            .iter()
            .all(|mana| { mana.c == 0 && mana.w + mana.u + mana.b + mana.r + mana.g == 1 }));
    }
}
