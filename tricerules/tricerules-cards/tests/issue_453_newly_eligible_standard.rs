//! Issue #453 — registry conformance for the 24 newly eligible pinned-Standard identities.
//!
//! Every cohort member is a complete-card exact match to a recipe that already shipped; the only
//! change is reviewed admission. Expectations below are the reviewed printed Oracle behavior and
//! the shipped typed vocabulary, not a copy of generator output. Exact Scryfall records and
//! `rulings_uri` responses were fetched 2026-09-19 against pinned snapshot
//! `27bf3214-1271-490b-bdfe-c0be6c23d02e`; none changed the emitted mechanics.
//!
//! CR surfaces: 701.8 (destroy), 120.2b (damage source), 702.51 (Convoke), 702.21 (Ward),
//! 701.42 (surveil), 702.29 (cycling/basic landcycling), 614.1d (enters tapped), 603.6c/700.4
//! (dies triggers), 702.108 (prowess), 715 (Adventure), 303.4 (Aura attach), 113.6 (graveyard
//! abilities), and 111.10b (Food).

mod common;

use common::FaceExpectation;
use tricerules_cards::primitives::{
    Amount, CardTypeFilter, CombatRole, DiscardQuantity, EffectSubject, EntersTappedAffected,
    EventZone, LibraryPartitionKind, PermanentEventFilter, PermanentTypeFilter, PlayerRecipient,
    ResolutionCost, SearchDestination, SearchZoneSelection, SpellCastFilter, SpellCostModifier,
    SpellEffectKind, StaticAbilityDef, TargetController, TargetFilter, TargetKind,
    TargetMatchFilter, TargetingSourceFilter, TriggerCondition, ZoneCardFilter,
    ZoneEventCardinality, ZoneEventDestination,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, CardRegistry, CastTriggerPlayer, Keyword, Layout, ManaAmount,
    ManaCost,
};

struct CohortEntry {
    id: &'static str,
    name: &'static str,
    face_id: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    keywords: &'static [Keyword],
    power_toughness: Option<(u32, u32)>,
    layout: Layout,
    face_count: usize,
}

const COHORT: [CohortEntry; 24] = [
    CohortEntry {
        id: "raucous_theater",
        name: "Raucous Theater",
        face_id: "raucous_theater",
        mana: "",
        types: &["Land", "Swamp", "Mountain"],
        keywords: &[],
        power_toughness: None,
        layout: Layout::Normal,
        face_count: 1,
    },
    CohortEntry {
        id: "huatlis_final_strike",
        name: "Huatli's Final Strike",
        face_id: "huatli_s_final_strike",
        mana: "{2}{G}",
        types: &["Instant"],
        keywords: &[],
        power_toughness: None,
        layout: Layout::Normal,
        face_count: 1,
    },
    CohortEntry {
        id: "capital_city",
        name: "Capital City",
        face_id: "capital_city",
        mana: "",
        types: &["Land", "Town"],
        keywords: &[],
        power_toughness: None,
        layout: Layout::Normal,
        face_count: 1,
    },
    CohortEntry {
        id: "kree_sentinel",
        name: "Kree Sentinel",
        face_id: "kree_sentinel",
        mana: "{4}{R}",
        types: &["Artifact", "Creature", "Kree", "Robot", "Villain"],
        keywords: &[Keyword::Reach],
        power_toughness: Some((5, 5)),
        layout: Layout::Normal,
        face_count: 1,
    },
    CohortEntry {
        id: "al_bhed_salvagers",
        name: "Al Bhed Salvagers",
        face_id: "al_bhed_salvagers",
        mana: "{2}{B}",
        types: &["Creature", "Human", "Artificer", "Warrior"],
        keywords: &[],
        power_toughness: Some((2, 3)),
        layout: Layout::Normal,
        face_count: 1,
    },
    CohortEntry {
        id: "bilbos_deadly_slice",
        name: "Bilbo's Deadly Slice",
        face_id: "bilbo_s_deadly_slice",
        mana: "{1}{B}{B}",
        types: &["Instant"],
        keywords: &[],
        power_toughness: None,
        layout: Layout::Normal,
        face_count: 1,
    },
    CohortEntry {
        id: "bloodfell_caves",
        name: "Bloodfell Caves",
        face_id: "bloodfell_caves",
        mana: "",
        types: &["Land"],
        keywords: &[],
        power_toughness: None,
        layout: Layout::Normal,
        face_count: 1,
    },
    CohortEntry {
        id: "savage_land_dinosaur",
        name: "Savage Land Dinosaur",
        face_id: "savage_land_dinosaur",
        mana: "{4}{G}{G}",
        types: &["Creature", "Dinosaur"],
        keywords: &[Keyword::Trample],
        power_toughness: Some((7, 6)),
        layout: Layout::Normal,
        face_count: 1,
    },
    CohortEntry {
        id: "large_bear",
        name: "Large Bear",
        face_id: "large_bear",
        mana: "{3}{B/G}{B/G}",
        types: &["Creature", "Bear"],
        keywords: &[Keyword::Reach, Keyword::Trample, Keyword::Haste],
        power_toughness: Some((5, 5)),
        layout: Layout::Normal,
        face_count: 1,
    },
    CohortEntry {
        id: "vote_out",
        name: "Vote Out",
        face_id: "vote_out",
        mana: "{3}{B}",
        types: &["Sorcery"],
        keywords: &[Keyword::Convoke],
        power_toughness: None,
        layout: Layout::Normal,
        face_count: 1,
    },
    CohortEntry {
        id: "magnificent_end",
        name: "Magnificent End",
        face_id: "magnificent_end",
        mana: "{4}{W}",
        types: &["Instant"],
        keywords: &[],
        power_toughness: None,
        layout: Layout::Normal,
        face_count: 1,
    },
    CohortEntry {
        id: "ordinary_bear",
        name: "Ordinary Bear",
        face_id: "ordinary_bear",
        mana: "{3}{G}",
        types: &["Creature", "Bear"],
        keywords: &[],
        power_toughness: Some((4, 5)),
        layout: Layout::Normal,
        face_count: 1,
    },
    CohortEntry {
        id: "impact_tremors",
        name: "Impact Tremors",
        face_id: "impact_tremors",
        mana: "{1}{R}",
        types: &["Enchantment"],
        keywords: &[],
        power_toughness: None,
        layout: Layout::Normal,
        face_count: 1,
    },
    CohortEntry {
        id: "bestial_bloodline",
        name: "Bestial Bloodline",
        face_id: "bestial_bloodline",
        mana: "{1}{G}",
        types: &["Enchantment", "Aura"],
        keywords: &[],
        power_toughness: None,
        layout: Layout::Normal,
        face_count: 1,
    },
    CohortEntry {
        id: "rabanastre,_royal_city",
        name: "Rabanastre, Royal City",
        face_id: "rabanastre_royal_city",
        mana: "",
        types: &["Land", "Town"],
        keywords: &[],
        power_toughness: None,
        layout: Layout::Normal,
        face_count: 1,
    },
    CohortEntry {
        id: "protective_response",
        name: "Protective Response",
        face_id: "protective_response",
        mana: "{2}{W}",
        types: &["Instant"],
        keywords: &[Keyword::Convoke],
        power_toughness: None,
        layout: Layout::Normal,
        face_count: 1,
    },
    CohortEntry {
        id: "archive_dragon",
        name: "Archive Dragon",
        face_id: "archive_dragon",
        mana: "{4}{U}{U}",
        types: &["Creature", "Dragon", "Wizard"],
        keywords: &[Keyword::Flying],
        power_toughness: Some((4, 6)),
        layout: Layout::Normal,
        face_count: 1,
    },
    CohortEntry {
        id: "stony-voiced_goblins",
        name: "Stony-Voiced Goblins",
        face_id: "stony_voiced_goblins",
        mana: "{1}{B}",
        types: &["Creature", "Goblin", "Bard"],
        keywords: &[],
        power_toughness: Some((1, 1)),
        layout: Layout::Normal,
        face_count: 1,
    },
    CohortEntry {
        id: "thundering_falls",
        name: "Thundering Falls",
        face_id: "thundering_falls",
        mana: "",
        types: &["Land", "Island", "Mountain"],
        keywords: &[],
        power_toughness: None,
        layout: Layout::Normal,
        face_count: 1,
    },
    CohortEntry {
        id: "minecart_daredevil_ride_the_rails",
        name: "Minecart Daredevil // Ride the Rails",
        face_id: "minecart_daredevil",
        mana: "{2}{R}",
        types: &["Creature", "Dwarf", "Knight"],
        keywords: &[],
        power_toughness: Some((4, 2)),
        layout: Layout::Adventure,
        face_count: 2,
    },
    CohortEntry {
        id: "lightshell_duo",
        name: "Lightshell Duo",
        face_id: "lightshell_duo",
        mana: "{3}{U}",
        types: &["Creature", "Rat", "Otter"],
        keywords: &[],
        power_toughness: Some((3, 4)),
        layout: Layout::Normal,
        face_count: 1,
    },
    CohortEntry {
        id: "topiary_panther",
        name: "Topiary Panther",
        face_id: "topiary_panther",
        mana: "{4}{G}{G}",
        types: &["Creature", "Plant", "Cat"],
        keywords: &[Keyword::Trample],
        power_toughness: Some((6, 5)),
        layout: Layout::Normal,
        face_count: 1,
    },
    CohortEntry {
        id: "gingerbread_hunter_puny_snack",
        name: "Gingerbread Hunter // Puny Snack",
        face_id: "gingerbread_hunter",
        mana: "{4}{G}",
        types: &["Creature", "Giant"],
        keywords: &[],
        power_toughness: Some((5, 5)),
        layout: Layout::Adventure,
        face_count: 2,
    },
    CohortEntry {
        id: "smaug,_the_great_calamity_spew_flame",
        name: "Smaug, the Great Calamity // Spew Flame",
        face_id: "smaug_the_great_calamity",
        mana: "{5}{R}{R}",
        types: &["Creature", "Dragon"],
        keywords: &[Keyword::Flying],
        power_toughness: Some((5, 5)),
        layout: Layout::Adventure,
        face_count: 2,
    },
];

fn face(id: &str) -> &'static tricerules_cards::CardFace {
    CardRegistry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing reviewed card {id}"))
        .primary_face()
}

fn adventure_face(id: &str, index: usize) -> &'static tricerules_cards::CardFace {
    CardRegistry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing reviewed card {id}"))
        .face(index)
        .unwrap_or_else(|| panic!("{id} face {index}"))
}

fn default_creature() -> TargetFilter {
    TargetFilter::default_creature()
}

fn mana_options(
    ability: &tricerules_cards::ActivatedAbilityDef,
) -> Vec<(u32, u32, u32, u32, u32, u32)> {
    ability
        .mana_options()
        .expect("generated mana ability publishes typed options")
        .iter()
        .map(|option: &ManaAmount| (option.w, option.u, option.b, option.r, option.g, option.c))
        .collect()
}

#[test]
fn issue_453_registers_exactly_the_reviewed_twenty_four() {
    let registry = CardRegistry::global();
    for card in &COHORT {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing reviewed card {}", card.id));
        assert_eq!(definition.name, card.name, "{}", card.id);
        assert_eq!(
            registry.id_for_name(card.name),
            Some(card.id),
            "{}",
            card.id
        );
        assert_eq!(definition.layout, card.layout, "{}", card.id);
        assert_eq!(definition.face_count(), card.face_count, "{}", card.id);
        FaceExpectation {
            id: card.id,
            name: card.name,
            face_id: card.face_id,
            mana_cost: card.mana,
            types: card.types,
            keywords: card.keywords,
            power_toughness: card.power_toughness,
        }
        .check();
    }
}

#[test]
fn issue_453_destroy_spells_keep_exact_target_classes() {
    let registry = CardRegistry::global();

    for id in ["bilbos_deadly_slice", "vote_out"] {
        let face = face(id);
        assert_eq!(
            face.spell_effect,
            [SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(Box::new(default_creature())),
            }],
            "{id}"
        );
        assert!(face.targeting.is_none(), "{id} targets inline");
        assert!(face.triggered_abilities.is_empty(), "{id}");
        assert!(face.activated_abilities.is_empty(), "{id}");
        assert!(face.static_abilities.is_empty(), "{id}");
    }

    let protective = face("protective_response");
    assert_eq!(
        protective.spell_effect,
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                combat_role: Some(CombatRole::AttackingOrBlocking),
                ..TargetFilter::default()
            })),
        }],
        "CR 509 / 701.8: attacking or blocking creatures only"
    );
    for id in ["bilbos_deadly_slice", "vote_out", "protective_response"] {
        assert!(registry.get(id).is_some(), "{id}");
    }
}

#[test]
fn issue_453_magnificent_end_reduces_only_for_a_tapped_creature_target() {
    let face = face("magnificent_end");
    let [SpellCostModifier::TargetMatchGenericReduction { amount, filter }] =
        face.cost_modifiers.as_slice()
    else {
        panic!("Magnificent End must own one target-matching reduction");
    };
    assert_eq!(*amount, 3);
    let TargetMatchFilter::Battlefield(filter) = filter else {
        panic!("the reduction must inspect the battlefield target");
    };
    assert_eq!(filter.kind, TargetKind::Creature);
    assert_eq!(filter.tapped, Some(true));
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::DamageTarget {
            amount: Amount::Fixed(5),
            target: default_creature(),
        }]
    );
    assert!(face.targeting.is_none());
}

#[test]
fn issue_453_huatli_final_strike_pumps_then_deals_the_pumped_power() {
    let face = face("huatlis_final_strike");
    assert_eq!(
        face.spell_effect,
        [
            SpellEffectKind::PumpTarget {
                power: 1,
                toughness: 0,
                scale: None,
                subject: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::You,
                    ..TargetFilter::default()
                })),
            },
            SpellEffectKind::CreatureDealsDamageEqualToPower {
                source: TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::You,
                    ..TargetFilter::default()
                },
                target: TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::Opponent,
                    ..TargetFilter::default()
                },
            },
        ],
        "printed order: pump, then power-based damage"
    );
    let targeting = face.targeting.as_ref().expect("two target groups");
    assert_eq!(targeting.groups.len(), 2);
    assert_eq!(
        (
            targeting.groups[0].min,
            targeting.groups[0].max,
            targeting.groups[0].effect_indices.as_slice(),
        ),
        (1, 1, [0, 1].as_slice())
    );
    assert_eq!(
        (
            targeting.groups[1].min,
            targeting.groups[1].max,
            targeting.groups[1].effect_indices.as_slice(),
        ),
        (1, 1, [1].as_slice())
    );
}

#[test]
fn issue_453_lands_keep_entry_replacements_and_mana_abilities() {
    let registry = CardRegistry::global();

    for (id, line) in [("bloodfell_caves", 1), ("rabanastre,_royal_city", 1)] {
        let face = face(id);
        let [entry] = face.static_abilities.as_slice() else {
            panic!("{id} must have one entry replacement");
        };
        assert_eq!(entry.ability_id.as_str(), "static_01");
        assert_eq!(
            entry.presentation,
            AbilityPresentation::OracleLines(vec![line])
        );
        assert_eq!(
            entry.definition,
            StaticAbilityDef::EntersTapped {
                affected: EntersTappedAffected::Self_,
                condition: None,
                unless_cost: None,
            },
            "{id}"
        );
    }

    let caves = face("bloodfell_caves");
    let [gain] = caves.triggered_abilities.as_slice() else {
        panic!("Bloodfell Caves must have one ETB trigger");
    };
    assert_eq!(gain.ability_id.as_str(), "triggered_01");
    assert_eq!(gain.presentation, AbilityPresentation::OracleLines(vec![2]));
    assert_eq!(gain.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        gain.effect,
        [SpellEffectKind::GainLife {
            amount: Amount::Fixed(1)
        }]
    );
    let [tap] = caves.activated_abilities.as_slice() else {
        panic!("Bloodfell Caves must have one mana ability");
    };
    assert_eq!(tap.ability_id.as_str(), "activated_01");
    assert_eq!(tap.presentation, AbilityPresentation::OracleLines(vec![3]));
    assert_eq!(tap.costs, [AbilityCost::Tap]);
    assert_eq!(
        mana_options(tap),
        [(0, 0, 1, 0, 0, 0), (0, 0, 0, 1, 0, 0)],
        "printed order: {{B}} then {{R}}"
    );

    let rabanastre = face("rabanastre,_royal_city");
    let [tap] = rabanastre.activated_abilities.as_slice() else {
        panic!("Rabanastre must have one mana ability");
    };
    assert_eq!(tap.presentation, AbilityPresentation::OracleLines(vec![2]));
    assert_eq!(tap.costs, [AbilityCost::Tap]);
    assert_eq!(
        mana_options(tap),
        [(0, 0, 0, 1, 0, 0), (1, 0, 0, 0, 0, 0)],
        "printed order: {{R}} then {{W}}"
    );
    assert!(
        rabanastre.triggered_abilities.is_empty(),
        "Rabanastre has no ETB trigger"
    );

    for id in ["raucous_theater", "thundering_falls"] {
        let face = face(id);
        assert_eq!(face.types.len(), 3, "{id} keeps intrinsic basic land types");
        let [entry] = face.static_abilities.as_slice() else {
            panic!("{id} must have one entry replacement");
        };
        assert_eq!(
            entry.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert_eq!(
            entry.definition,
            StaticAbilityDef::EntersTapped {
                affected: EntersTappedAffected::Self_,
                condition: None,
                unless_cost: None,
            },
            "{id}"
        );
        let [surveil] = face.triggered_abilities.as_slice() else {
            panic!("{id} must have one ETB surveil");
        };
        assert_eq!(surveil.ability_id.as_str(), "triggered_01");
        assert_eq!(
            surveil.presentation,
            AbilityPresentation::OracleLines(vec![3])
        );
        assert_eq!(surveil.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert_eq!(
            surveil.effect,
            [SpellEffectKind::LibraryPartition {
                count: 1,
                top_min: 0,
                top_max: None,
                kind: LibraryPartitionKind::Surveil,
            }],
            "{id}"
        );
        assert!(
            face.activated_abilities.is_empty(),
            "{id} relies on intrinsic basic land types, not an explicit mana ability"
        );
    }
    assert!(registry.get("raucous_theater").is_some());
}

#[test]
fn issue_453_capital_city_keeps_its_three_printed_abilities() {
    let face = face("capital_city");
    assert_eq!(face.types, ["Land", "Town"]);
    let [colorless, any_color, cycling] = face.activated_abilities.as_slice() else {
        panic!("Capital City must have three activated abilities");
    };
    assert_eq!(colorless.ability_id.as_str(), "activated_01");
    assert_eq!(
        colorless.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        colorless.source_zone,
        tricerules_cards::AbilitySourceZone::Battlefield
    );
    assert_eq!(colorless.costs, [AbilityCost::Tap]);
    assert_eq!(mana_options(colorless), [(0, 0, 0, 0, 0, 1)]);

    assert_eq!(any_color.ability_id.as_str(), "activated_02");
    assert_eq!(
        any_color.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        any_color.costs,
        [
            AbilityCost::Mana(ManaCost::parse("{1}").unwrap()),
            AbilityCost::Tap
        ]
    );
    assert_eq!(
        mana_options(any_color),
        [
            (1, 0, 0, 0, 0, 0),
            (0, 1, 0, 0, 0, 0),
            (0, 0, 1, 0, 0, 0),
            (0, 0, 0, 1, 0, 0),
            (0, 0, 0, 0, 1, 0),
        ],
        "WUBRG option order"
    );

    assert_eq!(cycling.ability_id.as_str(), "activated_03");
    assert_eq!(
        cycling.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(
        cycling.source_zone,
        tricerules_cards::AbilitySourceZone::Hand
    );
    assert_eq!(
        cycling.costs,
        [
            AbilityCost::Mana(ManaCost::parse("{2}").unwrap()),
            AbilityCost::DiscardSelf
        ]
    );
    assert_eq!(
        cycling.effect,
        [SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        }]
    );
}

#[test]
fn issue_453_landcycling_abilities_search_a_revealed_basic_land_to_hand() {
    for (id, cost, line) in [
        ("kree_sentinel", "{2}", 2),
        ("savage_land_dinosaur", "{2}", 2),
        ("topiary_panther", "{1}{G}", 2),
    ] {
        let face = face(id);
        let [ability] = face.activated_abilities.as_slice() else {
            panic!("{id} must have one landcycling ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![line])
        );
        assert_eq!(
            ability.source_zone,
            tricerules_cards::AbilitySourceZone::Hand
        );
        assert_eq!(
            ability.costs,
            [
                AbilityCost::Mana(ManaCost::parse(cost).unwrap()),
                AbilityCost::DiscardSelf
            ],
            "{id}"
        );
        assert_eq!(
            ability.effect,
            [SpellEffectKind::SearchLibrary {
                who: PlayerRecipient::Controller,
                optional: false,
                count: 1,
                count_by_cast_cost: None,
                filter: Some(ZoneCardFilter {
                    card_type: Some(CardTypeFilter::BasicLand),
                    ..ZoneCardFilter::default()
                }),
                slots: Vec::new(),
                zones: SearchZoneSelection::default(),
                destination: SearchDestination::Hand,
                conditional_destination: None,
                shuffle: true,
                reveal: true,
                result_id: None,
            }],
            "{id}"
        );
    }
}

#[test]
fn issue_453_etb_triggers_keep_exact_shapes() {
    let registry = CardRegistry::global();

    let salvagers = face("al_bhed_salvagers");
    let [drain] = salvagers.triggered_abilities.as_slice() else {
        panic!("Al Bhed Salvagers must have one dies trigger");
    };
    assert_eq!(drain.ability_id.as_str(), "triggered_01");
    assert_eq!(
        drain.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(!drain.may);
    assert_eq!(
        drain.trigger,
        TriggerCondition::WheneverPermanentLeavesBattlefield {
            controller: CastTriggerPlayer::Controller,
            filter: PermanentEventFilter {
                any_of: Some(vec![
                    PermanentEventFilter {
                        permanent_type: Some(PermanentTypeFilter::Creature),
                        ..PermanentEventFilter::default()
                    },
                    PermanentEventFilter {
                        permanent_type: Some(PermanentTypeFilter::Artifact),
                        ..PermanentEventFilter::default()
                    },
                ]),
                ..PermanentEventFilter::default()
            },
            destination: ZoneEventDestination::OneOf(vec![EventZone::Graveyard]),
            cardinality: ZoneEventCardinality::EachObject,
        }
    );
    assert_eq!(
        drain.effect,
        [
            SpellEffectKind::TargetPlayerLosesLife {
                amount: 1,
                target: TargetFilter {
                    kind: TargetKind::OpponentPlayer,
                    ..TargetFilter::default()
                },
            },
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(1)
            },
        ]
    );
    let targeting = drain.targeting.as_ref().expect("one target group");
    assert_eq!(targeting.groups.len(), 1);
    assert_eq!((targeting.groups[0].min, targeting.groups[0].max), (1, 1));
    assert_eq!(targeting.groups[0].effect_indices, [0]);
    assert_eq!(targeting.groups[0].prompt, "Choose target opponent");

    let goblins = face("stony-voiced_goblins");
    let [discard] = goblins.triggered_abilities.as_slice() else {
        panic!("Stony-Voiced Goblins must have one ETB trigger");
    };
    assert_eq!(
        discard.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(discard.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        discard.effect,
        [SpellEffectKind::Discard {
            who: PlayerRecipient::EachOpponent,
            quantity: DiscardQuantity::Exact(1),
        }]
    );

    let gingerbread = face("gingerbread_hunter_puny_snack");
    let [food] = gingerbread.triggered_abilities.as_slice() else {
        panic!("Gingerbread Hunter must have one ETB trigger");
    };
    assert_eq!(food.presentation, AbilityPresentation::OracleLines(vec![1]));
    assert_eq!(food.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        food.effect,
        [SpellEffectKind::CreateTokens {
            token: "food".into(),
            count: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        }]
    );
    assert!(registry.is_token("food"));

    let dragon = face("archive_dragon");
    let [ward, scry] = dragon.triggered_abilities.as_slice() else {
        panic!("Archive Dragon must have Ward then an ETB scry");
    };
    assert_eq!(ward.ability_id.as_str(), "triggered_01");
    assert_eq!(ward.presentation, AbilityPresentation::OracleLines(vec![2]));
    assert_eq!(
        ward.trigger,
        TriggerCondition::WheneverSelfBecomesTarget {
            source: TargetingSourceFilter::SpellOrAbility,
            source_controller: CastTriggerPlayer::Opponent,
        }
    );
    assert_eq!(
        ward.effect,
        [SpellEffectKind::CounterTriggeringStackObjectUnlessPays {
            cost: ResolutionCost::Mana(ManaCost::parse("{2}").unwrap()),
        }]
    );
    assert_eq!(scry.ability_id.as_str(), "triggered_02");
    assert_eq!(scry.presentation, AbilityPresentation::OracleLines(vec![3]));
    assert_eq!(scry.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        scry.effect,
        [SpellEffectKind::Scry {
            count: Amount::Fixed(2)
        }]
    );
}

#[test]
fn issue_453_impact_tremors_and_lightshell_duo_keep_their_trigger_shapes() {
    let tremors = face("impact_tremors");
    let [ping] = tremors.triggered_abilities.as_slice() else {
        panic!("Impact Tremors must have one entry trigger");
    };
    assert_eq!(ping.ability_id.as_str(), "triggered_01");
    assert_eq!(ping.presentation, AbilityPresentation::OracleLines(vec![1]));
    assert_eq!(
        ping.trigger,
        TriggerCondition::WheneverPermanentEntersBattlefield {
            controller: CastTriggerPlayer::Controller,
            filter: PermanentEventFilter {
                permanent_type: Some(PermanentTypeFilter::Creature),
                ..PermanentEventFilter::default()
            },
            creature_filter: None,
        }
    );
    assert_eq!(
        ping.effect,
        [SpellEffectKind::DamagePlayer {
            amount: Amount::Fixed(1),
            who: PlayerRecipient::EachOpponent,
        }]
    );
    assert!(ping.targeting.is_none());

    let duo = face("lightshell_duo");
    let [prowess, surveil] = duo.triggered_abilities.as_slice() else {
        panic!("Lightshell Duo must have prowess then an ETB surveil");
    };
    assert_eq!(prowess.ability_id.as_str(), "triggered_01");
    assert_eq!(
        prowess.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        prowess.trigger,
        TriggerCondition::WheneverPlayerCastsSpell {
            caster: CastTriggerPlayer::Controller,
            filter: SpellCastFilter {
                card_type: Some(CardTypeFilter::Noncreature),
                ..SpellCastFilter::default()
            },
            ordinal: None,
            ordinal_scope: Default::default(),
        }
    );
    assert_eq!(
        prowess.effect,
        [SpellEffectKind::PumpTarget {
            power: 1,
            toughness: 1,
            scale: None,
            subject: EffectSubject::Source,
        }]
    );
    assert_eq!(surveil.ability_id.as_str(), "triggered_02");
    assert_eq!(
        surveil.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(surveil.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        surveil.effect,
        [SpellEffectKind::LibraryPartition {
            count: 2,
            top_min: 0,
            top_max: None,
            kind: LibraryPartitionKind::Surveil,
        }]
    );
}

#[test]
fn issue_453_bestial_bloodline_keeps_aura_modifier_and_graveyard_return() {
    let face = face("bestial_bloodline");
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::AuraAttach {
            target: default_creature(),
        }]
    );
    let [modifier] = face.static_abilities.as_slice() else {
        panic!("Bestial Bloodline must have one attached modifier");
    };
    assert_eq!(modifier.ability_id.as_str(), "static_01");
    assert_eq!(
        modifier.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(matches!(
        &modifier.definition,
        StaticAbilityDef::AttachedModifier {
            delta_power: 2,
            delta_toughness: 2,
            ..
        }
    ));
    let [return_ability] = face.activated_abilities.as_slice() else {
        panic!("Bestial Bloodline must have one graveyard activation");
    };
    assert_eq!(return_ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        return_ability.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(
        return_ability.source_zone,
        tricerules_cards::AbilitySourceZone::Graveyard
    );
    assert_eq!(
        return_ability.costs,
        [AbilityCost::Mana(ManaCost::parse("{4}{G}").unwrap())]
    );
    assert_eq!(
        return_ability.effect,
        [SpellEffectKind::ReturnToOwnersHand {
            subject: EffectSubject::Source,
        }]
    );
}

#[test]
fn issue_453_adventure_faces_keep_printed_characteristics_and_effects() {
    let registry = CardRegistry::global();

    let minecart = adventure_face("minecart_daredevil_ride_the_rails", 1);
    assert_eq!(minecart.name, "Ride the Rails");
    assert_eq!(minecart.face_id.as_str(), "ride_the_rails");
    assert_eq!(minecart.mana_cost.to_string(), "{1}{R}");
    assert_eq!(minecart.types, ["Instant", "Adventure"]);
    assert_eq!(
        minecart.spell_effect,
        [SpellEffectKind::PumpTarget {
            power: 2,
            toughness: 1,
            scale: None,
            subject: EffectSubject::Chosen(Box::new(default_creature())),
        }]
    );

    let puny_snack = adventure_face("gingerbread_hunter_puny_snack", 1);
    assert_eq!(puny_snack.name, "Puny Snack");
    assert_eq!(puny_snack.face_id.as_str(), "puny_snack");
    assert_eq!(puny_snack.mana_cost.to_string(), "{2}{B}");
    assert_eq!(puny_snack.types, ["Instant", "Adventure"]);
    assert_eq!(
        puny_snack.spell_effect,
        [SpellEffectKind::PumpTarget {
            power: -2,
            toughness: -2,
            scale: None,
            subject: EffectSubject::Chosen(Box::new(default_creature())),
        }]
    );

    let smaug = registry
        .get("smaug,_the_great_calamity_spew_flame")
        .expect("Smaug");
    assert_eq!(smaug.primary_face().supertypes, ["Legendary"]);
    let spew_flame = adventure_face("smaug,_the_great_calamity_spew_flame", 1);
    assert_eq!(spew_flame.name, "Spew Flame");
    assert_eq!(spew_flame.face_id.as_str(), "spew_flame");
    assert_eq!(spew_flame.mana_cost.to_string(), "{4}{R}");
    assert_eq!(spew_flame.types, ["Sorcery", "Adventure"]);
    assert_eq!(
        spew_flame.spell_effect,
        [SpellEffectKind::DamageTarget {
            amount: Amount::Fixed(5),
            target: default_creature(),
        }]
    );
}

#[test]
fn issue_453_vanilla_bears_keep_only_printed_keywords() {
    let large = face("large_bear");
    assert_eq!(
        large.keywords,
        [Keyword::Reach, Keyword::Trample, Keyword::Haste]
    );
    let ordinary = face("ordinary_bear");
    assert!(ordinary.keywords.is_empty());

    for id in ["large_bear", "ordinary_bear"] {
        let face = face(id);
        assert!(face.spell_effect.is_empty(), "{id}");
        assert!(face.triggered_abilities.is_empty(), "{id}");
        assert!(face.activated_abilities.is_empty(), "{id}");
        assert!(face.static_abilities.is_empty(), "{id}");
        assert!(face.targeting.is_none(), "{id}");
    }
}

#[test]
fn issue_453_unreviewed_damage_amounts_stay_unregistered() {
    let registry = CardRegistry::global();
    for excluded in ["breath_of_fire", "fiery_finish"] {
        assert!(
            registry.get(excluded).is_none(),
            "{excluded} is an unreviewed damage-amount singleton and must stay unregistered"
        );
        assert!(
            registry
                .id_for_name(match excluded {
                    "breath_of_fire" => "Breath of Fire",
                    _ => "Fiery Finish",
                })
                .is_none(),
            "{excluded} must not resolve by name"
        );
    }
}
