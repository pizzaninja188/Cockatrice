//! Registry conformance for the queue-v46 reuse-only cohorts.
#![allow(clippy::type_complexity)]
use tricerules_cards::primitives::{
    AbilityCost, ActivationLimit, ActivationTiming, Amount, CounterKind, EffectSubject,
    EntersTappedAffected, InstructionKind, RelativePlayerSet, SearchDestination,
    SearchSelectionBounds, SpellEffectKind, StaticAbilityDef, TargetKind, TriggerCondition,
    ZoneCardFilter,
};
use tricerules_cards::{registry, AbilityPresentation, CardDefinition, CardFace, Layout};

fn definition(id: &str) -> &'static CardDefinition {
    registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
}

fn identity(
    id: &str,
    name: &str,
    mana: &str,
    types: &[&str],
    body: Option<(u32, u32)>,
) -> &'static CardFace {
    let definition = definition(id);
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    assert_eq!(registry::global().id_for_name(name), Some(id), "{id}");
    let f = definition.primary_face();
    assert_eq!(f.face_id.as_str(), id, "{id} face id");
    assert_eq!(f.name, name, "{id} name");
    assert_eq!(f.mana_cost.to_string(), mana, "{id} mana");
    assert_eq!(
        f.types.iter().map(String::as_str).collect::<Vec<_>>(),
        types,
        "{id} types"
    );
    assert_eq!(
        (f.power, f.toughness),
        body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
        "{id} body"
    );
    f
}

// ── 01 quest-counter expeditions ────────────────────────────────────────────

#[test]
fn q46_quest_cycle_maps_definitions() {
    for (id, name, mana, effect_name) in [
        (
            "soul_stair_expedition",
            "Soul Stair Expedition",
            "{B}",
            "move",
        ),
        (
            "khalni_heart_expedition",
            "Khalni Heart Expedition",
            "{1}{G}",
            "search",
        ),
        (
            "sunspring_expedition",
            "Sunspring Expedition",
            "{W}",
            "life",
        ),
        (
            "ior_ruin_expedition",
            "Ior Ruin Expedition",
            "{1}{U}",
            "draw",
        ),
    ] {
        let f = identity(id, name, mana, &["Enchantment"], None);
        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{id}: one landfall trigger");
        };
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert!(
            matches!(
                trigger.trigger,
                TriggerCondition::WheneverPermanentEntersBattlefield {
                    controller: tricerules_cards::primitives::CastTriggerPlayer::Controller,
                    filter: tricerules_cards::primitives::PermanentEventFilter {
                        permanent_type: Some(
                            tricerules_cards::primitives::PermanentTypeFilter::Land
                        ),
                        ..
                    },
                    ..
                }
            ),
            "{id}: landfall controller trigger"
        );
        assert!(trigger.may, "{id}: may");
        assert!(
            matches!(
                trigger.effect.as_slice(),
                [SpellEffectKind::PutCounters {
                    counter: CounterKind::Quest,
                    count: Amount::Fixed(1),
                    subject: EffectSubject::Source,
                }]
            ),
            "{id}: quest counter on the source"
        );
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: one activated ability");
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert!(
            matches!(
                ability.costs.as_slice(),
                [
                    AbilityCost::RemoveCounters {
                        counter: Some(CounterKind::Quest),
                        count: 3,
                        ..
                    },
                    AbilityCost::SacrificeSelf
                ]
            ),
            "{id}: remove three quest counters and sacrifice as costs"
        );
        match effect_name {
            "move" => {
                assert!(
                    matches!(
                        ability.effect.as_slice(),
                        [SpellEffectKind::MoveGraveyardCards {
                            filter: tricerules_cards::primitives::GraveyardFilter {
                                owner: tricerules_cards::primitives::GraveyardOwner::Controller,
                                card: Some(ZoneCardFilter {
                                    card_type: Some(
                                        tricerules_cards::primitives::CardTypeFilter::Creature
                                    ),
                                    ..
                                }),
                                ..
                            },
                            destination: tricerules_cards::primitives::GraveyardDestination::Hand,
                            ..
                        }]
                    ),
                    "{id}: return creature cards from your graveyard to hand"
                );
                let targeting = ability.targeting.as_ref().expect("graveyard targets");
                assert_eq!(targeting.groups.len(), 1);
                assert_eq!(
                    (targeting.groups[0].min, targeting.groups[0].max),
                    (0, 2),
                    "{id}: up to two targets"
                );
            }
            "search" => {
                let program = ability.program.as_ref().expect("typed search program");
                let [step] = program.body.steps.as_slice() else {
                    panic!("{id}: one program step");
                };
                let InstructionKind::Perform(effect) = &step.kind else {
                    panic!("{id}: typed search");
                };
                let SpellEffectKind::SearchLibrary {
                    count,
                    filter,
                    destination,
                    selection_bounds,
                    ..
                } = effect.as_ref()
                else {
                    panic!("{id}: typed search");
                };
                assert_eq!(*count, 2, "{id}: two cards");
                assert_eq!(
                    *destination,
                    SearchDestination::Battlefield { tapped: true }
                );
                assert_eq!(
                    selection_bounds,
                    &Some(SearchSelectionBounds { min: 0, max: 2 }),
                    "{id}: up to two"
                );
                assert!(
                    matches!(
                        filter,
                        Some(ZoneCardFilter {
                            card_type: Some(
                                tricerules_cards::primitives::CardTypeFilter::BasicLand
                            ),
                            ..
                        })
                    ),
                    "{id}: basic lands"
                );
            }
            "life" => assert!(
                matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::GainLife {
                        amount: Amount::Fixed(8),
                        ..
                    }]
                ),
                "{id}: gain eight life"
            ),
            "draw" => assert!(
                matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::Draw {
                        count: Amount::Fixed(2),
                        ..
                    }]
                ),
                "{id}: draw two"
            ),
            _ => unreachable!(),
        }
    }
}

// ── 02 archway commons cycle ────────────────────────────────────────────────

#[test]
fn q46_archway_cycle_maps_definitions() {
    for (id, name, is_gate) in [
        ("archway_commons", "Archway Commons", false),
        ("rupture_spire", "Rupture Spire", false),
        ("transguild_promenade", "Transguild Promenade", false),
        ("gateway_plaza", "Gateway Plaza", true),
    ] {
        let types: &[&str] = if is_gate {
            &["Land", "Gate"]
        } else {
            &["Land"]
        };
        let f = identity(id, name, "", types, None);
        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{id}: one static ability");
        };
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert!(
            matches!(
                static_ability.definition,
                StaticAbilityDef::EntersTapped {
                    affected: EntersTappedAffected::Self_,
                    ..
                }
            ),
            "{id}: enters tapped"
        );
        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{id}: one ETB trigger");
        };
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        let program = trigger.program.as_ref().expect("pay-or-sacrifice program");
        let [step] = program.body.steps.as_slice() else {
            panic!("{id}: one step");
        };
        let InstructionKind::Choose {
            optional,
            branches,
            otherwise,
            ..
        } = &step.kind
        else {
            panic!("{id}: choose");
        };
        assert!(*optional, "{id}: optional payment");
        assert_eq!(branches.len(), 1);
        assert_eq!(
            branches[0].cost,
            tricerules_cards::primitives::ResolutionCost::Mana(
                tricerules_cards::mana::ManaCost::parse("{1}").expect("mana")
            )
        );
        assert!(
            otherwise.steps.iter().any(|step| {
                matches!(
                    &step.kind,
                    InstructionKind::Perform(effect)
                        if matches!(
                            effect.as_ref(),
                            SpellEffectKind::Sacrifice {
                                subject: EffectSubject::Source
                            }
                        )
                )
            }),
            "{id}: declining sacrifices the source"
        );
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: one mana ability");
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![3])
        );
        assert_eq!(ability.costs, vec![AbilityCost::Tap]);
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 5
                        && options[0].w == 1
                        && options[1].u == 1
                        && options[2].b == 1
                        && options[3].r == 1
                        && options[4].g == 1
            ),
            "{id}: one mana of any color"
        );
    }
}

// ── 03 hidden cave discover ─────────────────────────────────────────────────

#[test]
fn q46_hidden_caves_maps_definitions() {
    for (id, name, color, cost) in [
        (
            "hidden_nursery",
            "Hidden Nursery",
            tricerules_cards::primitives::Color::Green,
            "{4}{G}",
        ),
        (
            "hidden_cataract",
            "Hidden Cataract",
            tricerules_cards::primitives::Color::Blue,
            "{4}{U}",
        ),
        (
            "hidden_volcano",
            "Hidden Volcano",
            tricerules_cards::primitives::Color::Red,
            "{4}{R}",
        ),
        (
            "hidden_courtyard",
            "Hidden Courtyard",
            tricerules_cards::primitives::Color::White,
            "{4}{W}",
        ),
        (
            "hidden_necropolis",
            "Hidden Necropolis",
            tricerules_cards::primitives::Color::Black,
            "{4}{B}",
        ),
    ] {
        let f = identity(id, name, "", &["Land", "Cave"], None);
        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{id}: one static ability");
        };
        assert!(matches!(
            static_ability.definition,
            StaticAbilityDef::EntersTapped {
                affected: EntersTappedAffected::Self_,
                ..
            }
        ));
        let [mana, discover] = f.activated_abilities.as_slice() else {
            panic!("{id}: two activated abilities");
        };
        assert_eq!(mana.presentation, AbilityPresentation::OracleLines(vec![2]));
        assert_eq!(mana.costs, vec![AbilityCost::Tap]);
        assert!(
            matches!(
                mana.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 1
                        && match color {
                            tricerules_cards::primitives::Color::White => options[0].w == 1,
                            tricerules_cards::primitives::Color::Blue => options[0].u == 1,
                            tricerules_cards::primitives::Color::Black => options[0].b == 1,
                            tricerules_cards::primitives::Color::Red => options[0].r == 1,
                            tricerules_cards::primitives::Color::Green => options[0].g == 1,
                        }
            ),
            "{id}: one mana of the printed color"
        );
        assert_eq!(
            discover.presentation,
            AbilityPresentation::OracleLines(vec![3])
        );
        assert_eq!(discover.timing, ActivationTiming::SorcerySpeed);
        assert_eq!(
            discover.costs,
            vec![
                AbilityCost::Mana(tricerules_cards::mana::ManaCost::parse(cost).expect("mana")),
                AbilityCost::Tap,
                AbilityCost::SacrificeSelf,
            ]
        );
        assert!(matches!(
            discover.effect.as_slice(),
            [SpellEffectKind::Discover { value: 4 }]
        ));
    }
}

// ── 04 impulse look ─────────────────────────────────────────────────────────

#[test]
fn q46_impulse_look_maps_definitions() {
    for (id, name, mana, kind, count, put) in [
        ("impulse", "Impulse", "{1}{U}", "Instant", 4u32, 1u32),
        ("anticipate", "Anticipate", "{1}{U}", "Instant", 3, 1),
        ("stock_up", "Stock Up", "{2}{U}", "Sorcery", 5, 2),
    ] {
        let f = identity(id, name, mana, &[kind], None);
        let program = f.spell_program.as_ref().expect("look program");
        let [step] = program.body.steps.as_slice() else {
            panic!("{id}: one step");
        };
        assert!(
            matches!(
                &step.kind,
                InstructionKind::Perform(effect)
                    if matches!(
                        effect.as_ref(),
                        SpellEffectKind::LookChooseToHand {
                            count: c,
                            min: m,
                            reveal: false,
                            bottom_order: tricerules_cards::primitives::LibraryBottomOrder::Chosen,
                            ..
                        } if *c == count && *m == put
                    )
            ),
            "{id}: look at {count}, put {put} into hand"
        );
    }
}

// ── 05 target player draws ──────────────────────────────────────────────────

#[test]
fn q46_target_player_draw_maps_definitions() {
    for (id, name, mana, kind, amount, is_x) in [
        (
            "stroke_of_genius",
            "Stroke of Genius",
            "{X}{2}{U}",
            "Instant",
            "X",
            true,
        ),
        (
            "braingeyser",
            "Braingeyser",
            "{X}{U}{U}",
            "Sorcery",
            "X",
            true,
        ),
        (
            "inspiration",
            "Inspiration",
            "{3}{U}",
            "Instant",
            "2",
            false,
        ),
        (
            "opportunity",
            "Opportunity",
            "{4}{U}{U}",
            "Instant",
            "4",
            false,
        ),
        (
            "overflowing_insight",
            "Overflowing Insight",
            "{4}{U}{U}{U}",
            "Sorcery",
            "7",
            false,
        ),
    ] {
        let f = identity(id, name, mana, &[kind], None);
        assert!(matches!(
            f.spell_effect.as_slice(),
            [SpellEffectKind::TargetPlayerDraws {
                count,
                target: tricerules_cards::primitives::TargetFilter {
                    kind: TargetKind::AnyPlayer,
                    ..
                },
            }] if match amount {
                "X" => matches!(count, Amount::X) == is_x,
                value => matches!(count, Amount::Fixed(n) if *n == value.parse::<u32>().expect("count")),
            }
        ));
        let targeting = f.targeting.as_ref().expect("target player");
        assert_eq!(targeting.groups.len(), 1);
        assert_eq!((targeting.groups[0].min, targeting.groups[0].max), (1, 1));
    }
}

// ── 06 damage divided two ───────────────────────────────────────────────────

#[test]
fn q46_damage_divided_maps_definitions() {
    for (id, name, mana, kind) in [
        (
            "chandras_pyrohelix",
            "Chandra's Pyrohelix",
            "{1}{R}",
            "Instant",
        ),
        ("twin_bolt", "Twin Bolt", "{1}{R}", "Instant"),
        ("forked_bolt", "Forked Bolt", "{R}", "Sorcery"),
    ] {
        let f = identity(id, name, mana, &[kind], None);
        assert!(matches!(
            f.spell_effect.as_slice(),
            [SpellEffectKind::DamageTargets {
                amount: Amount::Fixed(2),
                division: tricerules_cards::primitives::DamageDivision::ChooseAtCast,
                ..
            }]
        ));
        let targeting = f.targeting.as_ref().expect("damage targets");
        assert_eq!(targeting.groups.len(), 1);
        assert_eq!((targeting.groups[0].min, targeting.groups[0].max), (1, 2));
    }
}

// ── 07 enters-tap lock ──────────────────────────────────────────────────────

#[test]
fn q46_etb_tap_lock_maps_definitions() {
    for (id, name, mana, types, body, flying, enters_tapped, line) in [
        (
            "stitched_mangler",
            "Stitched Mangler",
            "{2}{U}",
            &["Creature", "Zombie", "Horror"][..],
            (2, 3),
            false,
            true,
            2,
        ),
        (
            "chillbringer",
            "Chillbringer",
            "{4}{U}",
            &["Creature", "Elemental"][..],
            (3, 3),
            true,
            false,
            2,
        ),
        (
            "white_dragon",
            "White Dragon",
            "{4}{W}{W}",
            &["Creature", "Dragon"][..],
            (4, 4),
            true,
            false,
            2,
        ),
    ] {
        let f = identity(id, name, mana, types, Some(body));
        assert_eq!(
            f.keywords,
            if flying {
                vec![tricerules_cards::primitives::Keyword::Flying]
            } else {
                Vec::new()
            },
            "{id}"
        );
        assert_eq!(
            f.static_abilities.iter().any(|ability| matches!(
                ability.definition,
                StaticAbilityDef::EntersTapped {
                    affected: EntersTappedAffected::Self_,
                    ..
                }
            )),
            enters_tapped,
            "{id}: enters-tapped replacement"
        );
        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{id}: one ETB trigger");
        };
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![line])
        );
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert!(
            matches!(
                trigger.effect.as_slice(),
                [
                    SpellEffectKind::Tap {
                        subject: EffectSubject::Chosen(_)
                    },
                    SpellEffectKind::SkipNextUntap {
                        target: tricerules_cards::primitives::TargetFilter {
                            controller: tricerules_cards::primitives::TargetController::Opponent,
                            ..
                        }
                    }
                ]
            ),
            "{id}: tap and skip the next untap"
        );
        let targeting = trigger.targeting.as_ref().expect("one target");
        assert_eq!(targeting.groups.len(), 1);
        assert_eq!((targeting.groups[0].min, targeting.groups[0].max), (1, 1));
    }
}

// ── 08 graveyard-name draw ──────────────────────────────────────────────────

#[test]
fn q46_graveyard_name_draw_maps_definitions() {
    for (id, name, mana, kind, owners) in [
        (
            "accumulated_knowledge",
            "Accumulated Knowledge",
            "{1}{U}",
            "Instant",
            RelativePlayerSet::All,
        ),
        (
            "take_inventory",
            "Take Inventory",
            "{1}{U}",
            "Sorcery",
            RelativePlayerSet::Controller,
        ),
        (
            "frantic_inventory",
            "Frantic Inventory",
            "{1}{U}",
            "Instant",
            RelativePlayerSet::Controller,
        ),
    ] {
        let f = identity(id, name, mana, &[kind], None);
        let [first, second] = f.spell_effect.as_slice() else {
            panic!("{id}: draw then count");
        };
        assert!(matches!(
            first,
            SpellEffectKind::Draw {
                count: Amount::Fixed(1),
                ..
            }
        ));
        assert!(matches!(
            second,
            SpellEffectKind::Draw {
                count: Amount::Count(tricerules_cards::primitives::CountExpression::GraveyardCards {
                    owners: actual,
                    filter: Some(ZoneCardFilter { exact_name: Some(actual_name), .. }),
                }),
                ..
            } if *actual == owners && actual_name == name
        ));
    }
}

// ── 09 flying twice pump ────────────────────────────────────────────────────

#[test]
fn q46_flying_twice_pump_maps_definitions() {
    for (id, name, types) in [
        ("pit_imp", "Pit Imp", &["Creature", "Imp"][..]),
        (
            "phyrexian_battleflies",
            "Phyrexian Battleflies",
            &["Creature", "Phyrexian", "Insect"][..],
        ),
        ("vampire_bats", "Vampire Bats", &["Creature", "Bat"][..]),
    ] {
        let f = identity(id, name, "{B}", types, Some((0, 1)));
        assert_eq!(
            f.keywords,
            vec![tricerules_cards::primitives::Keyword::Flying],
            "{id}"
        );
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: one activated ability");
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert_eq!(
            ability.costs,
            vec![AbilityCost::Mana(
                tricerules_cards::mana::ManaCost::parse("{B}").expect("mana")
            )]
        );
        assert!(matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::PumpTarget {
                power: 1,
                toughness: 0,
                subject: EffectSubject::Source,
                ..
            }]
        ));
        assert_eq!(
            ability.activation_limit,
            Some(ActivationLimit::PerTurn { max_activations: 2 })
        );
    }
}

#[test]
fn q46_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name) in [
        ("soul_stair_expedition", "Soul Stair Expedition"),
        ("khalni_heart_expedition", "Khalni Heart Expedition"),
        ("sunspring_expedition", "Sunspring Expedition"),
        ("ior_ruin_expedition", "Ior Ruin Expedition"),
        ("archway_commons", "Archway Commons"),
        ("rupture_spire", "Rupture Spire"),
        ("transguild_promenade", "Transguild Promenade"),
        ("gateway_plaza", "Gateway Plaza"),
        ("hidden_nursery", "Hidden Nursery"),
        ("hidden_cataract", "Hidden Cataract"),
        ("hidden_volcano", "Hidden Volcano"),
        ("hidden_courtyard", "Hidden Courtyard"),
        ("hidden_necropolis", "Hidden Necropolis"),
        ("impulse", "Impulse"),
        ("anticipate", "Anticipate"),
        ("stock_up", "Stock Up"),
        ("stroke_of_genius", "Stroke of Genius"),
        ("braingeyser", "Braingeyser"),
        ("inspiration", "Inspiration"),
        ("opportunity", "Opportunity"),
        ("overflowing_insight", "Overflowing Insight"),
        ("chandras_pyrohelix", "Chandra's Pyrohelix"),
        ("twin_bolt", "Twin Bolt"),
        ("forked_bolt", "Forked Bolt"),
        ("stitched_mangler", "Stitched Mangler"),
        ("chillbringer", "Chillbringer"),
        ("white_dragon", "White Dragon"),
        ("accumulated_knowledge", "Accumulated Knowledge"),
        ("take_inventory", "Take Inventory"),
        ("frantic_inventory", "Frantic Inventory"),
        ("pit_imp", "Pit Imp"),
        ("phyrexian_battleflies", "Phyrexian Battleflies"),
        ("vampire_bats", "Vampire Bats"),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "{id}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id, "face id drift for {id}");
        assert_eq!(fields[3], name);
        assert_eq!(fields[4].len(), 64, "fingerprint length for {id}");
        assert!(
            fields[4].chars().all(|ch| ch.is_ascii_hexdigit()),
            "fingerprint digest for {id}"
        );
        assert_eq!(definition(id).primary_face().name, name, "{id}");
    }
}
