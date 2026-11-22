//! Actual-card scenarios for the queue-v41 Rebel and Mercenary subtype tutors.
//!
//! Expectations are printed Oracle values. CR 602.2 governs activation, CR 107.5/118.3 the
//! tap cost, CR 701.23 a filtered library search (which may fail to find), CR 701.24 the
//! shuffle and CR 608.2b the resolution-time legality of the chosen card.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

struct Case {
    id: &'static str,
    mana: ManaGift,
    subtype: &'static str,
    max_mana_value: u32,
}

fn cases() -> [Case; 7] {
    [
        Case {
            id: "ramosian_sergeant",
            mana: ManaGift {
                c: 3,
                ..Default::default()
            },
            subtype: "Rebel",
            max_mana_value: 2,
        },
        Case {
            id: "ramosian_lieutenant",
            mana: ManaGift {
                c: 4,
                ..Default::default()
            },
            subtype: "Rebel",
            max_mana_value: 3,
        },
        Case {
            id: "amrou_scout",
            mana: ManaGift {
                c: 4,
                ..Default::default()
            },
            subtype: "Rebel",
            max_mana_value: 3,
        },
        Case {
            id: "ramosian_commander",
            mana: ManaGift {
                c: 6,
                ..Default::default()
            },
            subtype: "Rebel",
            max_mana_value: 5,
        },
        Case {
            id: "cateran_persuader",
            mana: ManaGift {
                c: 1,
                ..Default::default()
            },
            subtype: "Mercenary",
            max_mana_value: 1,
        },
        Case {
            id: "cateran_brute",
            mana: ManaGift {
                c: 2,
                ..Default::default()
            },
            subtype: "Mercenary",
            max_mana_value: 2,
        },
        Case {
            id: "cateran_kidnappers",
            mana: ManaGift {
                c: 3,
                ..Default::default()
            },
            subtype: "Mercenary",
            max_mana_value: 3,
        },
    ]
}

fn gift(m: &ManaGift) -> ManaGift {
    ManaGift {
        w: m.w,
        u: m.u,
        b: m.b,
        r: m.r,
        g: m.g,
        c: m.c,
    }
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[card]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    e
}

fn reach_search(e: &mut GameEngine, source: u32, case: &Case) -> Vec<u32> {
    give_mana(e, 0, gift(&case.mana));
    apply_ability(e, 0, source, 0, vec![]).expect("activate the tutor");
    let batch = pass_both_players_with_batch(e);
    let choice = find_resolution_choice(&batch).expect("library search choice");
    assert_eq!(
        choice.choice_kind(),
        ChoiceKind::LibrarySearch,
        "{}",
        case.id
    );
    assert_eq!(choice.deciding_player_id, 0, "{}", case.id);
    assert!(
        choice.public_reveal.is_none(),
        "{}: the library search is private",
        case.id
    );
    choice.candidate_object_ids
}

#[test]
fn q41_tutor_canary_simplest_finds_a_legal_rebel() {
    let case = cases()
        .into_iter()
        .find(|case| case.id == "ramosian_sergeant")
        .expect("Ramosian Sergeant case");
    let mut e = engine(41_010_001, case.id);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    let rebel = inject_library_card(&mut e, 0, "rathi_trapper");
    let nonrebel = inject_library_card(&mut e, 0, "grizzly_bears");
    let candidates = reach_search(&mut e, source, &case);
    assert!(candidates.contains(&rebel), "the Rebel is offered");
    assert!(
        !candidates.contains(&nonrebel),
        "a non-Rebel is not offered"
    );
    e.apply_scenario_command(0, &submit_resolution_choice(vec![rebel]))
        .expect("find the Rebel");
    assert_eq!(e.state.objects[&rebel].zone, Zone::Battlefield);
    assert!(
        !e.state.objects[&rebel].tapped,
        "the found card enters untapped"
    );
    assert_eq!(e.state.objects[&source].zone, Zone::Battlefield);
    assert!(e.state.objects[&source].tapped, "the tap cost was paid");
    assert_eq!(e.effective_power(rebel), Some(1));
    assert_eq!(e.effective_toughness(rebel), Some(2));
    assert_eq!(e.state.players[0].life, 20);
}

#[test]
fn q41_tutor_canary_hardest_respects_the_printed_mana_value_cap() {
    let case = cases()
        .into_iter()
        .find(|case| case.id == "ramosian_commander")
        .expect("Ramosian Commander case");
    let mut e = engine(41_010_002, case.id);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    let at_cap = inject_library_card(&mut e, 0, "resistance_skywarden");
    let above_cap = inject_library_card(&mut e, 0, "jhovall_queen");
    let candidates = reach_search(&mut e, source, &case);
    assert!(candidates.contains(&at_cap), "mana value 5 is within 5");
    assert!(
        !candidates.contains(&above_cap),
        "mana value 6 is above the printed cap"
    );
    e.apply_scenario_command(0, &submit_resolution_choice(vec![at_cap]))
        .expect("find the capped Rebel");
    assert_eq!(e.state.objects[&at_cap].zone, Zone::Battlefield);
}

#[test]
fn q41_tutor_cards_apply_their_printed_subtype_and_cap() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(41_011_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let legal = match (case.subtype, case.max_mana_value) {
            ("Rebel", 2) => Some(inject_library_card(&mut e, 0, "rathi_trapper")),
            ("Rebel", _) => Some(inject_library_card(&mut e, 0, "saltfield_recluse")),
            ("Mercenary", 1) => None,
            ("Mercenary", 2) => Some(inject_library_card(&mut e, 0, "spineless_thug")),
            _ => Some(inject_library_card(&mut e, 0, "bog_smugglers")),
        };
        let wrong_subtype = inject_library_card(&mut e, 0, "grizzly_bears");
        let candidates = reach_search(&mut e, source, &case);
        if let Some(legal) = legal {
            assert!(candidates.contains(&legal), "{}: legal candidate", case.id);
        } else {
            assert!(
                candidates.is_empty(),
                "{}: no registered Mercenary satisfies mana value 1",
                case.id
            );
        }
        assert!(
            !candidates.contains(&wrong_subtype),
            "{}: wrong subtype",
            case.id
        );
        assert!(
            candidates.iter().all(|id| {
                let card = &e.state.objects[id];
                let face = tricerules_cards::registry::global()
                    .get(&card.card_id)
                    .expect("registered candidate")
                    .primary_face();
                face.types.iter().any(|t| t == case.subtype)
                    && face.mana_cost.mana_value() <= case.max_mana_value
            }),
            "{}: every candidate satisfies the printed filter",
            case.id
        );
        e.apply_scenario_command(0, &submit_resolution_choice(legal.into_iter().collect()))
            .expect("complete the printed search");
        if let Some(legal) = legal {
            assert_eq!(
                e.state.objects[&legal].zone,
                Zone::Battlefield,
                "{}",
                case.id
            );
        }
        assert!(e.state.objects[&source].tapped, "{}: tap cost", case.id);
        let mana = &e.state.players[0].mana_pool;
        assert_eq!(
            (
                mana.white,
                mana.blue,
                mana.black,
                mana.red,
                mana.green,
                mana.colorless
            ),
            (0, 0, 0, 0, 0, 0),
            "{}: exact mana cost",
            case.id
        );
    }
}

#[test]
fn q41_tutor_cards_offer_noncreature_rebel_permanents() {
    for (index, case) in cases().into_iter().enumerate() {
        if case.subtype != "Rebel" {
            continue;
        }
        let mut e = engine(41_015_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let aura = inject_library_card(&mut e, 0, "bound_in_silence");
        let candidates = reach_search(&mut e, source, &case);
        if case.max_mana_value >= 3 {
            assert!(
                candidates.contains(&aura),
                "{}: a Rebel permanent card need not be a creature",
                case.id
            );
        } else {
            assert!(
                !candidates.contains(&aura),
                "{}: mana value 3 is above the printed cap",
                case.id
            );
        }
    }
}

#[test]
fn q41_tutor_cateran_canary_simplest_finds_a_legal_mercenary() {
    let case = cases()
        .into_iter()
        .find(|case| case.id == "cateran_brute")
        .expect("Cateran Brute case");
    let mut e = engine(41_014_001, case.id);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    let mercenary = inject_library_card(&mut e, 0, "spineless_thug");
    let nonmercenary = inject_library_card(&mut e, 0, "grizzly_bears");
    let candidates = reach_search(&mut e, source, &case);
    assert!(candidates.contains(&mercenary));
    assert!(!candidates.contains(&nonmercenary));
    e.apply_scenario_command(0, &submit_resolution_choice(vec![mercenary]))
        .expect("find the Mercenary");
    assert_eq!(e.state.objects[&mercenary].zone, Zone::Battlefield);
    assert!(e.state.objects[&source].tapped);
}

#[test]
fn q41_tutor_cateran_canary_hardest_respects_the_printed_cap() {
    let case = cases()
        .into_iter()
        .find(|case| case.id == "cateran_kidnappers")
        .expect("Cateran Kidnappers case");
    let mut e = engine(41_014_002, case.id);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    let at_cap = inject_library_card(&mut e, 0, "bog_smugglers");
    let above_cap = inject_library_card(&mut e, 0, "highway_robber");
    let candidates = reach_search(&mut e, source, &case);
    assert!(candidates.contains(&at_cap), "mana value 3 is within 3");
    assert!(
        !candidates.contains(&above_cap),
        "mana value 4 is above the printed cap"
    );
    e.apply_scenario_command(0, &submit_resolution_choice(vec![at_cap]))
        .expect("find the capped Mercenary");
    assert_eq!(e.state.objects[&at_cap].zone, Zone::Battlefield);
}

#[test]
fn q41_tutor_rejects_activation_without_the_printed_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(41_012_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let before = format!("{:?}", e.state);
        assert!(
            apply_ability(&mut e, 0, source, 0, vec![]).is_err(),
            "{}: the printed mana is required",
            case.id
        );
        assert_eq!(format!("{:?}", e.state), before, "{}", case.id);
    }
}

#[test]
fn q41_tutor_can_fail_to_find_and_still_shuffles() {
    let case = cases()
        .into_iter()
        .find(|case| case.id == "ramosian_sergeant")
        .expect("Ramosian Sergeant case");
    let mut e = engine(41_013_001, case.id);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    let wrong_subtype = inject_library_card(&mut e, 0, "grizzly_bears");
    let candidates = reach_search(&mut e, source, &case);
    assert!(
        !candidates.contains(&wrong_subtype),
        "no non-Rebel candidate is offered"
    );
    let batch = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("decline to find");
    assert_eq!(
        e.state.objects[&wrong_subtype].zone,
        Zone::Library,
        "the non-Rebel stays in the library"
    );
    assert!(e.state.objects[&source].tapped);
    for log in ["P0 finds no card.", "P0 shuffles their library."] {
        assert_eq!(
            batch
                .events
                .iter()
                .filter(|event| matches!(
                    &event.ev,
                    Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(message))
                        if message.text == log
                ))
                .count(),
            1,
            "the failed search emits '{log}' exactly once"
        );
    }
}
