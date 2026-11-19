//! Actual-card scenarios for the queue-v39 tap-ping cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/118.3 pay the activation cost; CR 701.26 taps
//! the source; CR 120.2/120.3 deal damage; CR 115.1a makes any target legal; CR 302.6 applies
//! summoning sickness unless haste; and CR 608.2b fizzles a stale target.
use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    ping_mana: ManaGift,
    keywords: &'static [Keyword],
}

fn cases() -> [Case; 6] {
    [
        Case {
            id: "mawcor",
            mana: ManaGift {
                u: 2,
                c: 3,
                ..Default::default()
            },
            ping_mana: ManaGift::default(),
            keywords: &[Keyword::Flying],
        },
        Case {
            id: "thornwind_faeries",
            mana: ManaGift {
                u: 2,
                c: 1,
                ..Default::default()
            },
            ping_mana: ManaGift::default(),
            keywords: &[Keyword::Flying],
        },
        Case {
            id: "vulshok_sorcerer",
            mana: ManaGift {
                r: 2,
                c: 1,
                ..Default::default()
            },
            ping_mana: ManaGift::default(),
            keywords: &[Keyword::Haste],
        },
        Case {
            id: "cunning_sparkmage",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            ping_mana: ManaGift::default(),
            keywords: &[Keyword::Haste],
        },
        Case {
            id: "anaba_shaman",
            mana: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
            ping_mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            keywords: &[],
        },
        Case {
            id: "jeska,_warrior_adept",
            mana: ManaGift {
                r: 2,
                c: 2,
                ..Default::default()
            },
            ping_mana: ManaGift::default(),
            keywords: &[Keyword::FirstStrike, Keyword::Haste],
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

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("mountain", &[]), deck_with("forest", &[])]);
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

fn cast_card(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(e, 0, gift(&case.mana));
    let slot = inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
    assert_eq!(
        e.state.objects[&slot].zone,
        Zone::Battlefield,
        "{}",
        case.id
    );
    slot
}

fn activate_ping(e: &mut GameEngine, case: &Case, source: u32, target: u32) -> Result<(), String> {
    give_mana(e, 0, gift(&case.ping_mana));
    apply_ability(e, 0, source, 0, target_object(target))
        .map(|_| ())
        .map_err(|error| format!("{error:?}"))
}

#[test]
fn q39_tap_ping_canary_simplest() {
    let case = &cases()[3];
    let mut e = engine(39_080_001);
    let source = cast_card(&mut e, case);
    activate_ping(&mut e, case, source, 1).expect("ping the opponent");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 19);
    assert!(e.state.objects[&source].tapped);
}

#[test]
fn q39_tap_ping_canary_hardest() {
    let case = &cases()[4];
    let mut e = engine(39_080_002);
    let source = cast_card(&mut e, case);
    e.state
        .objects
        .get_mut(&source)
        .expect("source")
        .summoning_sick = false;
    let victim = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 1, 1);
    activate_ping(&mut e, case, source, victim).expect("ping the creature");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&victim].zone, Zone::Graveyard);
    assert!(e.state.objects[&source].tapped);
    assert_eq!(e.state.players[1].life, 20);
}

#[test]
fn q39_tap_ping_cards_deal_exactly_one_damage_and_tap_the_source() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_081_000 + index as u64);
        let source = cast_card(&mut e, case);
        e.state
            .objects
            .get_mut(&source)
            .expect("source")
            .summoning_sick = false;
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 4, 4);
        activate_ping(&mut e, case, source, target).expect("activate ping");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&target].damage, 1, "{}", case.id);
        assert!(e.state.objects[&source].tapped, "{}", case.id);
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);
        for keyword in case.keywords {
            assert!(
                e.effective_has_keyword(source, *keyword),
                "{}: {:?}",
                case.id,
                keyword
            );
        }
    }
}

#[test]
fn q39_tap_ping_cards_can_target_a_player() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_082_000 + index as u64);
        let source = cast_card(&mut e, case);
        e.state
            .objects
            .get_mut(&source)
            .expect("source")
            .summoning_sick = false;
        give_mana(&mut e, 0, gift(&case.ping_mana));
        apply_ability(&mut e, 0, source, 0, target_player(1)).expect("ping player");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.players[1].life, 19, "{}", case.id);
    }
}

#[test]
fn q39_tap_ping_cards_require_their_printed_ping_mana() {
    let case = &cases()[4];
    let mut e = engine(39_083_000);
    let source = cast_card(&mut e, case);
    e.state
        .objects
        .get_mut(&source)
        .expect("source")
        .summoning_sick = false;
    assert!(apply_ability(&mut e, 0, source, 0, target_object(source)).is_err());
    assert!(!e.state.objects[&source].tapped);
    assert!(e.state.stack.is_empty());
}

#[test]
fn q39_tap_ping_cards_respect_summoning_sickness_without_haste() {
    let case = &cases()[0];
    let mut e = engine(39_084_000);
    let source = cast_card(&mut e, case);
    assert!(e.state.objects[&source].summoning_sick);
    assert!(apply_ability(&mut e, 0, source, 0, target_player(1)).is_err());
    assert!(!e.state.objects[&source].tapped);
}

#[test]
fn q39_tap_ping_cards_fizzle_when_the_target_left() {
    let case = &cases()[2];
    let mut e = engine(39_085_000);
    let source = cast_card(&mut e, case);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 4, 4);
    give_mana(&mut e, 0, gift(&case.ping_mana));
    apply_ability(&mut e, 0, source, 0, target_object(target)).expect("activate ping");
    e.state.players[1]
        .battlefield
        .retain(|object| *object != target);
    e.state.players[1].graveyard.push(target);
    e.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(target).or_default() += 1;
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 20);
    assert!(e.state.objects[&source].tapped);
}
