//! Actual-card scenarios for the queue-v27 "counter target instant or sorcery
//! spell" cohort.
//!
//! Expectations are printed Oracle values. CR 701.6 counters a spell on the stack;
//! CR 115.1a/601.2c make that spell the target; a spell of the other card type is
//! not a legal target and a rejected cast spends no mana. A stale target is handled
//! through the engine's ordinary CR 608.2b fizzle path.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    counters: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "dispel",
            counters: "lightning_bolt",
            mana: ManaGift {
                u: 1,
                ..Default::default()
            },
        },
        Case {
            id: "flash_counter",
            counters: "lightning_bolt",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "envelop",
            counters: "divination",
            mana: ManaGift {
                u: 1,
                ..Default::default()
            },
        },
        Case {
            id: "extinguish",
            counters: "divination",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
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

fn engine(seed: u64, counter: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &["lightning_bolt", "divination", "grizzly_bears"]),
        deck_with("island", &[counter]),
    ]);
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

fn cast_from_hand(e: &mut GameEngine, player: i32, card: &str, mana: ManaGift) -> u32 {
    ensure_in_hand(e, player as usize, card);
    give_mana(e, player, mana);
    let slot = hand_index_for_card(e, player as usize, card);
    let targets = match card {
        "lightning_bolt" => target_player(1),
        _ => vec![],
    };
    e.apply_scenario_command(player, &cast_spell(slot, targets))
        .unwrap_or_else(|error| panic!("cast {card}: {error:?}"));
    e.state.stack.last().expect("spell on stack").id
}

fn counter_spell(e: &mut GameEngine, case: &Case, target: u32) -> Result<(), String> {
    ensure_in_hand(e, 1, case.id);
    give_mana(e, 1, gift(&case.mana));
    let slot = hand_index_for_card(e, 1, case.id);
    e.apply_scenario_command(1, &cast_spell(slot, target_object(target)))
        .map(|_| ())
        .map_err(|error| format!("{error:?}"))
}

#[test]
fn counter_typed_spells_batch_cards_counter_their_printed_spell_type() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_030_000 + index as u64, case.id);
        let target_mana = if case.counters == "lightning_bolt" {
            ManaGift {
                r: 1,
                ..Default::default()
            }
        } else {
            ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            }
        };
        let spell = cast_from_hand(&mut e, 0, case.counters, target_mana);
        assert_eq!(e.state.stack.len(), 1, "{}", case.id);
        e.apply_scenario_command(0, &pass()).expect("pass priority");
        counter_spell(&mut e, &case, spell).unwrap_or_else(|error| panic!("{}: {error}", case.id));
        assert_eq!(e.state.stack.len(), 2, "{}", case.id);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&spell].zone,
            Zone::Graveyard,
            "{}: the countered spell is in its owner's graveyard",
            case.id
        );
        assert!(
            e.state.players[0].graveyard.contains(&spell),
            "{}: owner's graveyard",
            case.id
        );
        assert!(
            e.state.players[1]
                .graveyard
                .iter()
                .any(|oid| e.state.objects[oid].card_id == case.id),
            "{}: the counter resolves to its owner's graveyard",
            case.id
        );
        assert_eq!(
            e.state.players[1].life, 20,
            "{}: no spell effect resolved",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn counter_typed_spells_batch_cards_reject_the_other_spell_type() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_031_000 + index as u64, case.id);
        let wrong = if case.counters == "lightning_bolt" {
            cast_from_hand(
                &mut e,
                0,
                "grizzly_bears",
                ManaGift {
                    g: 1,
                    c: 1,
                    ..Default::default()
                },
            )
        } else {
            cast_from_hand(
                &mut e,
                0,
                "lightning_bolt",
                ManaGift {
                    r: 1,
                    c: 1,
                    ..Default::default()
                },
            )
        };
        e.apply_scenario_command(0, &pass()).expect("pass priority");
        assert!(
            counter_spell(&mut e, &case, wrong).is_err(),
            "{}: the other spell type is not a legal target",
            case.id
        );
        assert_eq!(e.state.stack.len(), 1, "{}", case.id);
        resolve_entire_stack_two_player(&mut e);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn counter_typed_spells_batch_cards_fizzle_when_the_target_spell_leaves_the_stack() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_032_000 + index as u64, case.id);
        let target_mana = if case.counters == "lightning_bolt" {
            ManaGift {
                r: 1,
                ..Default::default()
            }
        } else {
            ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            }
        };
        let spell = cast_from_hand(&mut e, 0, case.counters, target_mana);
        e.apply_scenario_command(0, &pass()).expect("pass priority");
        counter_spell(&mut e, &case, spell).expect("counter the spell");
        e.state.stack.retain(|entry| entry.id != spell);
        e.state.objects.get_mut(&spell).expect("spell").zone = Zone::Graveyard;
        e.state.players[0].graveyard.push(spell);
        *e.state.zone_change_generation.entry(spell).or_insert(0) += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&spell].zone,
            Zone::Graveyard,
            "{}: stale spell stays where it moved",
            case.id
        );
        assert!(
            e.state.players[1]
                .graveyard
                .iter()
                .any(|oid| e.state.objects[oid].card_id == case.id),
            "{}: the counter fizzles to its owner's graveyard",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
