//! Actual-card scenarios for the queue-v20 life-gain-plus-draw cohort.
//!
//! Expectations are printed Oracle values. CR 119.3 applies the life gain,
//! CR 121.1 the draw, CR 601.2 the exact mana cost, and CR 304.1/307.1 plus
//! CR 117.1a govern Sorcery and Instant timing.
use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    life: u32,
    sorcery: bool,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "dosans_oldest_chant",
            mana: ManaGift {
                g: 1,
                c: 4,
                ..Default::default()
            },
            life: 6,
            sorcery: true,
        },
        Case {
            id: "resupply",
            mana: ManaGift {
                w: 1,
                c: 5,
                ..Default::default()
            },
            life: 6,
            sorcery: false,
        },
        Case {
            id: "revitalize",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            life: 3,
            sorcery: false,
        },
        Case {
            id: "reviving_dose",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            life: 3,
            sorcery: false,
        },
        Case {
            id: "ritual_of_rejuvenation",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            life: 4,
            sorcery: false,
        },
    ]
}

fn engine(seed: u64, _card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[]), deck_with("forest", &[])]);
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn put_on_top(e: &mut GameEngine, player: usize, card: &str) -> u32 {
    let object = inject_library_card(e, player, card);
    e.state.players[player]
        .library
        .retain(|candidate| *candidate != object);
    e.state.players[player].library.push_front(object);
    object
}

fn cast_spell_for(e: &mut GameEngine, case: &Case) -> u32 {
    let slot = inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact mana", case.id);
    slot
}

#[test]
fn gain_life_draw_batch_cards_resolve_for_exact_life_and_one_card() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_100_000 + index as u64, case.id);
        let drawn = put_on_top(&mut e, 0, "grizzly_bears");
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();
        let spell = cast_spell_for(&mut e, &case);
        assert_eq!(
            e.state.players[0].life, 20,
            "{}: no life before resolution",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].life,
            20 + case.life as i32,
            "{}: exact printed life gain",
            case.id
        );
        assert_eq!(
            e.state.objects[&drawn].zone,
            Zone::Hand,
            "{}: exactly one card is drawn",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before + 1,
            "{}: the injected spell left hand and one card was drawn",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.len(),
            library_before - 1,
            "{}: one card leaves the library",
            case.id
        );
        assert_eq!(
            e.state.objects[&spell].zone,
            Zone::Graveyard,
            "{}: the spell resolves to the graveyard",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn gain_life_draw_batch_cards_reject_cast_without_exact_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_101_000 + index as u64, case.id);
        let slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
                .is_err(),
            "{}: the spell needs its exact mana",
            case.id
        );
        assert_eq!(
            e.state.objects[&slot].zone,
            Zone::Hand,
            "{}: the card stays in hand",
            case.id
        );
        assert_eq!(e.state.players[0].life, 20, "{}: no life gained", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn gain_life_draw_batch_instants_resolve_on_the_opponents_turn() {
    for (index, case) in cases().into_iter().filter(|case| !case.sorcery).enumerate() {
        let mut e = engine(10_102_000 + index as u64, case.id);
        end_active_turn(&mut e, 0);
        assert_eq!(e.state.active_player_idx, 1, "{}: opponent's turn", case.id);
        e.apply_scenario_command(1, &pass())
            .expect("the active player passes priority in upkeep");
        let slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
            .unwrap_or_else(|error| panic!("{}: instant on opponent turn: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].life,
            20 + case.life as i32,
            "{}: the instant resolved",
            case.id
        );
        assert_eq!(e.state.objects[&slot].zone, Zone::Graveyard, "{}", case.id);
    }
}

#[test]
fn gain_life_draw_batch_sorcery_is_rejected_outside_the_controllers_main_phase() {
    let case = cases()
        .into_iter()
        .find(|case| case.sorcery)
        .expect("sorcery");
    let mut e = engine(10_103_000, case.id);
    end_active_turn(&mut e, 0);
    assert_eq!(e.state.active_player_idx, 1);
    e.apply_scenario_command(1, &pass())
        .expect("the active player passes priority in upkeep");
    let slot = inject_card_into_hand(&mut e, 0, case.id);
    let hand_slot = hand_index_for_card(&e, 0, case.id);
    give_mana(&mut e, 0, gift(&case.mana));
    assert!(
        e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
            .is_err(),
        "{}: a sorcery cannot be cast on the opponent's turn",
        case.id
    );
    assert_eq!(e.state.objects[&slot].zone, Zone::Hand, "{}", case.id);
    assert_eq!(e.state.players[0].life, 20, "{}", case.id);
    assert!(e.state.stack.is_empty(), "{}", case.id);
}
