//! Registry conformance for the queue-v40 reuse-only cohorts.
use tricerules_cards::primitives::{
    Amount, EffectSubject, LibraryBottomOrder, LibraryPartitionKind, ObjectFilter,
    PermanentTypeFilter, SpellEffectKind, TargetKind, TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, Layout};

fn filter_permanent_types(filter: &ObjectFilter) -> Vec<PermanentTypeFilter> {
    filter.permanent_types.clone()
}

fn definition(id: &str) -> &'static tricerules_cards::CardDefinition {
    tricerules_cards::registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
}

#[test]
fn q40_look_choose_hand_maps_definitions() {
    struct Card {
        id: &'static str,
        name: &'static str,
        mana: &'static str,
        types: &'static [&'static str],
        count: u32,
        bottom: LibraryBottomOrder,
        gain: Option<u32>,
        flashback: Option<&'static str>,
    }
    let cards = [
        Card {
            id: "seek_the_wilds",
            name: "Seek the Wilds",
            mana: "{1}{G}",
            types: &["Sorcery"],
            count: 4,
            bottom: LibraryBottomOrder::Chosen,
            gain: None,
            flashback: None,
        },
        Card {
            id: "adventurous_impulse",
            name: "Adventurous Impulse",
            mana: "{G}",
            types: &["Sorcery"],
            count: 3,
            bottom: LibraryBottomOrder::Chosen,
            gain: None,
            flashback: None,
        },
        Card {
            id: "commune_with_dinosaurs",
            name: "Commune with Dinosaurs",
            mana: "{G}",
            types: &["Sorcery"],
            count: 5,
            bottom: LibraryBottomOrder::Chosen,
            gain: None,
            flashback: None,
        },
        Card {
            id: "peer_through_depths",
            name: "Peer Through Depths",
            mana: "{1}{U}",
            types: &["Instant", "Arcane"],
            count: 5,
            bottom: LibraryBottomOrder::Chosen,
            gain: None,
            flashback: None,
        },
        Card {
            id: "ancient_stirrings",
            name: "Ancient Stirrings",
            mana: "{G}",
            types: &["Sorcery"],
            count: 5,
            bottom: LibraryBottomOrder::Chosen,
            gain: None,
            flashback: None,
        },
        Card {
            id: "commune_with_spirits",
            name: "Commune with Spirits",
            mana: "{G}",
            types: &["Sorcery"],
            count: 4,
            bottom: LibraryBottomOrder::Random,
            gain: None,
            flashback: None,
        },
        Card {
            id: "commune_with_beavers",
            name: "Commune with Beavers",
            mana: "{G}",
            types: &["Sorcery"],
            count: 3,
            bottom: LibraryBottomOrder::Chosen,
            gain: None,
            flashback: None,
        },
        Card {
            id: "bond_of_flourishing",
            name: "Bond of Flourishing",
            mana: "{1}{G}",
            types: &["Sorcery"],
            count: 3,
            bottom: LibraryBottomOrder::Chosen,
            gain: Some(3),
            flashback: None,
        },
        Card {
            id: "arcane_infusion",
            name: "Arcane Infusion",
            mana: "{U}{R}",
            types: &["Instant"],
            count: 4,
            bottom: LibraryBottomOrder::Random,
            gain: None,
            flashback: Some("{3}{U}{R}"),
        },
        Card {
            id: "basic_conjuration",
            name: "Basic Conjuration",
            mana: "{1}{G}{G}",
            types: &["Sorcery", "Lesson"],
            count: 6,
            bottom: LibraryBottomOrder::Random,
            gain: Some(3),
            flashback: None,
        },
    ];
    let registry = tricerules_cards::registry::global();
    for card in cards {
        let definition = definition(card.id);
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        assert_eq!(f.face_id.as_str(), card.id);
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana);
        assert_eq!(f.types, card.types);
        assert_eq!((f.power, f.toughness), (None, None));
        assert!(f.keywords.is_empty());
        assert!(f.targeting.is_none());
        assert_eq!(
            f.flashback_cost.as_ref().map(ToString::to_string),
            card.flashback.map(str::to_owned)
        );
        let mut index = 0;
        match &f.spell_effect[index] {
            SpellEffectKind::LookChooseToHand {
                count,
                min,
                max,
                reveal,
                bottom_order,
                ..
            } => {
                assert_eq!(*count, card.count, "{}", card.id);
                assert_eq!((*min, *max), (0, 1), "{}", card.id);
                assert!(*reveal, "{}", card.id);
                assert_eq!(*bottom_order, card.bottom, "{}", card.id);
            }
            other => panic!("{}: expected LookChooseToHand, got {other:?}", card.id),
        }
        index += 1;
        if let Some(amount) = card.gain {
            assert!(
                matches!(&f.spell_effect[index], SpellEffectKind::GainLife { amount: Amount::Fixed(value), .. } if *value == amount),
                "{}: printed life gain",
                card.id
            );
            index += 1;
        }
        assert_eq!(
            index,
            f.spell_effect.len(),
            "{}: exact effect list",
            card.id
        );
    }
}

#[test]
fn q40_etb_look_choose_maps_definitions() {
    struct Card {
        id: &'static str,
        name: &'static str,
        mana: &'static str,
        types: &'static [&'static str],
        power: u32,
        toughness: u32,
        count: u32,
        bottom: LibraryBottomOrder,
    }
    let cards = [
        Card {
            id: "courageous_outrider",
            name: "Courageous Outrider",
            mana: "{3}{W}",
            types: &["Creature", "Human", "Scout"],
            power: 3,
            toughness: 4,
            count: 4,
            bottom: LibraryBottomOrder::Chosen,
        },
        Card {
            id: "eclipsed_flamekin",
            name: "Eclipsed Flamekin",
            mana: "{1}{U/R}{U/R}",
            types: &["Creature", "Elemental", "Scout"],
            power: 1,
            toughness: 4,
            count: 4,
            bottom: LibraryBottomOrder::Random,
        },
        Card {
            id: "sumala_woodshaper",
            name: "Sumala Woodshaper",
            mana: "{2}{G}{W}",
            types: &["Creature", "Elf", "Druid"],
            power: 2,
            toughness: 1,
            count: 4,
            bottom: LibraryBottomOrder::Random,
        },
        Card {
            id: "augur_of_bolas",
            name: "Augur of Bolas",
            mana: "{1}{U}",
            types: &["Creature", "Merfolk", "Wizard"],
            power: 1,
            toughness: 3,
            count: 3,
            bottom: LibraryBottomOrder::Chosen,
        },
        Card {
            id: "eclipsed_kithkin",
            name: "Eclipsed Kithkin",
            mana: "{G/W}{G/W}",
            types: &["Creature", "Kithkin", "Scout"],
            power: 2,
            toughness: 1,
            count: 4,
            bottom: LibraryBottomOrder::Random,
        },
    ];
    for card in cards {
        let f = definition(card.id).primary_face();
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana);
        assert_eq!(f.types, card.types);
        assert_eq!(
            (f.power, f.toughness),
            (Some(card.power), Some(card.toughness))
        );
        assert!(f.keywords.is_empty());
        assert_eq!(f.triggered_abilities.len(), 1);
        let trigger = &f.triggered_abilities[0];
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert!(matches!(
            trigger.effect.as_slice(),
            [SpellEffectKind::LookChooseToHand { count, min: 0, max: 1, reveal: true, bottom_order, .. }]
                if *count == card.count && *bottom_order == card.bottom
        ));
    }
}

#[test]
fn q40_etb_surveil_maps_definitions() {
    struct Card {
        id: &'static str,
        name: &'static str,
        mana: &'static str,
        types: &'static [&'static str],
        power: u32,
        toughness: u32,
        count: u32,
        keywords: &'static [tricerules_cards::primitives::Keyword],
        line: u16,
    }
    use tricerules_cards::primitives::Keyword::{Defender, Flash, Flying};
    let cards = [
        Card {
            id: "naga_oracle",
            name: "Naga Oracle",
            mana: "{3}{U}",
            types: &["Creature", "Snake", "Cleric"],
            power: 2,
            toughness: 4,
            count: 3,
            keywords: &[],
            line: 1,
        },
        Card {
            id: "whisper_agent",
            name: "Whisper Agent",
            mana: "{1}{U/B}{U/B}",
            types: &["Creature", "Human", "Rogue"],
            power: 3,
            toughness: 2,
            count: 1,
            keywords: &[Flash],
            line: 2,
        },
        Card {
            id: "barrier_of_bones",
            name: "Barrier of Bones",
            mana: "{B}",
            types: &["Creature", "Skeleton", "Wall"],
            power: 0,
            toughness: 3,
            count: 1,
            keywords: &[Defender],
            line: 2,
        },
        Card {
            id: "watcher_in_the_mist",
            name: "Watcher in the Mist",
            mana: "{3}{U}{U}",
            types: &["Creature", "Spirit"],
            power: 3,
            toughness: 4,
            count: 2,
            keywords: &[Flying],
            line: 2,
        },
        Card {
            id: "dimir_informant",
            name: "Dimir Informant",
            mana: "{2}{U}",
            types: &["Creature", "Human", "Rogue"],
            power: 1,
            toughness: 4,
            count: 2,
            keywords: &[],
            line: 1,
        },
    ];
    for card in cards {
        let f = definition(card.id).primary_face();
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana);
        assert_eq!(f.types, card.types);
        assert_eq!(
            (f.power, f.toughness),
            (Some(card.power), Some(card.toughness))
        );
        assert_eq!(f.keywords, card.keywords);
        assert_eq!(f.triggered_abilities.len(), 1);
        let trigger = &f.triggered_abilities[0];
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![card.line])
        );
        assert!(matches!(
            trigger.effect.as_slice(),
            [SpellEffectKind::LibraryPartition { count, top_min: 0, top_max: None, kind: LibraryPartitionKind::Surveil }]
                if *count == card.count
        ));
    }
}

#[test]
#[allow(clippy::type_complexity)]
fn q40_etb_shrink_maps_definitions() {
    use tricerules_cards::primitives::Keyword::Flying;
    let cases: [(
        &str,
        &str,
        &str,
        u32,
        u32,
        i32,
        i32,
        &[tricerules_cards::primitives::Keyword],
    ); 3] = [
        (
            "farbog_boneflinger",
            "Farbog Boneflinger",
            "{4}{B}",
            2,
            2,
            -2,
            -2,
            &[],
        ),
        (
            "keening_banshee",
            "Keening Banshee",
            "{2}{B}{B}",
            2,
            2,
            -2,
            -2,
            &[Flying],
        ),
        (
            "blister_beetle",
            "Blister Beetle",
            "{1}{B}",
            1,
            1,
            -1,
            -1,
            &[],
        ),
    ];
    for (id, name, mana, power, toughness, dp, dt, keywords) in cases {
        let f = definition(id).primary_face();
        assert_eq!(f.name, name);
        assert_eq!(f.mana_cost.to_string(), mana);
        assert_eq!((f.power, f.toughness), (Some(power), Some(toughness)));
        assert_eq!(f.keywords, keywords);
        assert_eq!(f.triggered_abilities.len(), 1);
        let trigger = &f.triggered_abilities[0];
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert!(matches!(
            trigger.effect.as_slice(),
            [SpellEffectKind::PumpTarget { power, toughness, subject: EffectSubject::Chosen(filter), .. }]
                if *power == dp
                    && *toughness == dt
                    && matches!(**filter, ObjectFilter { kind: TargetKind::Creature, .. })
        ));
        assert_eq!(trigger.targeting.as_ref().map(|t| t.groups.len()), Some(1));
    }
}

#[test]
fn q40_etb_drain_maps_definitions() {
    let cases: [(&str, &str, &str, u32, u32, u32, bool); 3] = [
        (
            "vampire_sovereign",
            "Vampire Sovereign",
            "{3}{B}{B}",
            3,
            4,
            3,
            true,
        ),
        (
            "highway_robber",
            "Highway Robber",
            "{2}{B}{B}",
            2,
            2,
            2,
            false,
        ),
        ("dakmor_ghoul", "Dakmor Ghoul", "{2}{B}{B}", 2, 2, 2, false),
    ];
    for (id, name, mana, power, toughness, amount, flying) in cases {
        let f = definition(id).primary_face();
        assert_eq!(f.name, name);
        assert_eq!(f.mana_cost.to_string(), mana);
        assert_eq!((f.power, f.toughness), (Some(power), Some(toughness)));
        assert_eq!(
            f.keywords
                .contains(&tricerules_cards::primitives::Keyword::Flying),
            flying
        );
        assert_eq!(f.triggered_abilities.len(), 1);
        let trigger = &f.triggered_abilities[0];
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert!(matches!(
            trigger.effect.as_slice(),
            [SpellEffectKind::DrainTarget { amount: value, target: ObjectFilter { kind: TargetKind::OpponentPlayer, .. } }]
                if *value == amount
        ));
    }
}

#[test]
fn q40_etb_destroy_enchantment_maps_definitions() {
    use tricerules_cards::primitives::Keyword::Flying;
    let cases: [(&str, &str, &str, &[tricerules_cards::primitives::Keyword]); 3] = [
        ("aven_cloudchaser", "Aven Cloudchaser", "{3}{W}", &[Flying]),
        ("monk_realist", "Monk Realist", "{1}{W}", &[]),
        (
            "cloudchaser_eagle",
            "Cloudchaser Eagle",
            "{3}{W}",
            &[Flying],
        ),
    ];
    for (id, name, mana, keywords) in cases {
        let f = definition(id).primary_face();
        assert_eq!(f.name, name);
        assert_eq!(f.mana_cost.to_string(), mana);
        assert_eq!(f.keywords, keywords);
        assert_eq!(f.triggered_abilities.len(), 1);
        let trigger = &f.triggered_abilities[0];
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert!(matches!(
            trigger.effect.as_slice(),
            [SpellEffectKind::Destroy { subject: EffectSubject::Chosen(filter) }]
                if matches!(**filter, ObjectFilter { kind: TargetKind::AnyPermanent, .. })
                    && filter_permanent_types(filter) == [PermanentTypeFilter::Enchantment]
        ));
        assert_eq!(trigger.targeting.as_ref().map(|t| t.groups.len()), Some(1));
    }
}

#[test]
#[allow(clippy::type_complexity)]
fn q40_attack_self_pump_maps_definitions() {
    use tricerules_cards::primitives::Keyword::{Flying, Trample};
    let cases: [(
        &str,
        &str,
        &str,
        u32,
        u32,
        i32,
        i32,
        &[tricerules_cards::primitives::Keyword],
    ); 3] = [
        (
            "charging_griffin",
            "Charging Griffin",
            "{3}{W}",
            2,
            2,
            1,
            1,
            &[Flying],
        ),
        (
            "ravenous_skirge",
            "Ravenous Skirge",
            "{2}{B}",
            1,
            1,
            2,
            0,
            &[Flying],
        ),
        (
            "hungry_spriggan",
            "Hungry Spriggan",
            "{2}{G}",
            1,
            1,
            3,
            3,
            &[Trample],
        ),
    ];
    for (id, name, mana, power, toughness, dp, dt, keywords) in cases {
        let f = definition(id).primary_face();
        assert_eq!(f.name, name);
        assert_eq!(f.mana_cost.to_string(), mana);
        assert_eq!((f.power, f.toughness), (Some(power), Some(toughness)));
        assert_eq!(f.keywords, keywords);
        assert_eq!(f.triggered_abilities.len(), 1);
        let trigger = &f.triggered_abilities[0];
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert!(matches!(
            trigger.trigger,
            TriggerCondition::WheneverSelfAttacks {
                minimum_other_attackers: 0
            }
        ));
        assert!(matches!(
            trigger.effect.as_slice(),
            [SpellEffectKind::PumpTarget { power, toughness, subject: EffectSubject::Source, .. }]
                if *power == dp && *toughness == dt
        ));
    }
}

#[test]
#[allow(clippy::type_complexity)]
fn q40_dies_return_self_maps_definitions() {
    use tricerules_cards::primitives::Keyword::{Flying, Trample};
    let cases: [(
        &str,
        &str,
        &str,
        u32,
        u32,
        &[tricerules_cards::primitives::Keyword],
        u16,
    ); 5] = [
        (
            "mortus_strider",
            "Mortus Strider",
            "{1}{U}{B}",
            1,
            1,
            &[],
            1,
        ),
        (
            "immortal_phoenix",
            "Immortal Phoenix",
            "{4}{R}{R}",
            5,
            3,
            &[Flying],
            2,
        ),
        (
            "shivan_phoenix",
            "Shivan Phoenix",
            "{4}{R}{R}",
            3,
            4,
            &[Flying],
            2,
        ),
        (
            "weatherseed_treefolk",
            "Weatherseed Treefolk",
            "{2}{G}{G}{G}",
            5,
            3,
            &[Trample],
            2,
        ),
        (
            "endless_cockroaches",
            "Endless Cockroaches",
            "{1}{B}{B}",
            1,
            1,
            &[],
            1,
        ),
    ];
    for (id, name, mana, power, toughness, keywords, line) in cases {
        let f = definition(id).primary_face();
        assert_eq!(f.name, name);
        assert_eq!(f.mana_cost.to_string(), mana);
        assert_eq!((f.power, f.toughness), (Some(power), Some(toughness)));
        assert_eq!(f.keywords, keywords);
        assert_eq!(f.triggered_abilities.len(), 1);
        let trigger = &f.triggered_abilities[0];
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![line])
        );
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfDies);
        assert!(matches!(
            trigger.effect.as_slice(),
            [SpellEffectKind::ReturnTriggeredCard { .. }]
        ));
    }
}

#[test]
#[allow(clippy::type_complexity)]
fn q40_dies_draw_maps_definitions() {
    use tricerules_cards::primitives::Keyword::{Flying, Trample, Vigilance};
    let cases: [(
        &str,
        &str,
        &str,
        u32,
        u32,
        &[tricerules_cards::primitives::Keyword],
    ); 3] = [
        (
            "riptide_crab",
            "Riptide Crab",
            "{1}{W}{U}",
            1,
            3,
            &[Vigilance],
        ),
        (
            "malcators_watcher",
            "Malcator's Watcher",
            "{1}{U}",
            1,
            1,
            &[Flying, Vigilance],
        ),
        (
            "silverback_shaman",
            "Silverback Shaman",
            "{3}{G}{G}",
            5,
            4,
            &[Trample],
        ),
    ];
    for (id, name, mana, power, toughness, keywords) in cases {
        let f = definition(id).primary_face();
        assert_eq!(f.name, name);
        assert_eq!(f.mana_cost.to_string(), mana);
        assert_eq!((f.power, f.toughness), (Some(power), Some(toughness)));
        assert_eq!(f.keywords, keywords);
        assert_eq!(f.triggered_abilities.len(), 1);
        let trigger = &f.triggered_abilities[0];
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfDies);
        assert!(matches!(
            trigger.effect.as_slice(),
            [SpellEffectKind::Draw {
                count: Amount::Fixed(1),
                ..
            }]
        ));
    }
}
