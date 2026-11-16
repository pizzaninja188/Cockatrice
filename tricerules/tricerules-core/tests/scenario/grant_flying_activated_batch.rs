//! Actual-card scenarios for the queue-v20 activated flying-grant cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 define the activated
//! ability and printed cost, CR 702.9a-b flying as evasion, CR 611.2a the
//! until-end-of-turn grant and CR 514.2 its cleanup expiry.
use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    cast: ManaGift,
    activation: ManaGift,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "goblin_balloon_brigade",
            cast: ManaGift {
                r: 1,
                ..Default::default()
            },
            activation: ManaGift {
                r: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        Case {
            id: "patagia_golem",
            cast: ManaGift {
                c: 4,
                ..Default::default()
            },
            activation: ManaGift {
                c: 3,
                ..Default::default()
            },
            power: 2,
            toughness: 3,
        },
        Case {
            id: "stream_hopper",
            cast: ManaGift {
                u: 1,
                ..Default::default()
            },
            activation: ManaGift {
                u: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
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

fn cast_creature(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(e, 0, gift(&case.cast));
    let slot = inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact full cost", case.id);
    resolve_entire_stack_two_player(e);
    assert_eq!(
        e.state.objects[&slot].zone,
        Zone::Battlefield,
        "{}: resolves onto the battlefield",
        case.id
    );
    slot
}

#[test]
fn grant_flying_activated_batch_cards_cast_with_exact_printed_characteristics() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_980_000 + index as u64);
        let creature = cast_creature(&mut e, &case);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
            case.id
        );
        assert!(
            !e.effective_has_keyword(creature, Keyword::Flying),
            "{}: no printed flying",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn grant_flying_activated_batch_cards_gain_flying_only_after_activation() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_981_000 + index as u64);
        let creature = cast_creature(&mut e, &case);
        give_mana(&mut e, 0, gift(&case.activation));
        apply_ability(&mut e, 0, creature, 0, vec![]).expect("activate flying");
        assert!(
            !e.effective_has_keyword(creature, Keyword::Flying),
            "{}: the grant is on the stack",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert!(
            e.effective_has_keyword(creature, Keyword::Flying),
            "{}: flying is granted on resolution",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
        assert!(
            !e.state.objects[&creature].tapped,
            "{}: a mana-only activation does not tap",
            case.id
        );
    }
}

#[test]
fn grant_flying_activated_batch_cards_lose_flying_at_cleanup() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_982_000 + index as u64);
        let creature = cast_creature(&mut e, &case);
        give_mana(&mut e, 0, gift(&case.activation));
        apply_ability(&mut e, 0, creature, 0, vec![]).expect("activate flying");
        resolve_entire_stack_two_player(&mut e);
        assert!(
            e.effective_has_keyword(creature, Keyword::Flying),
            "{}",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert!(
            !e.effective_has_keyword(creature, Keyword::Flying),
            "{}: CR 514.2 ends the grant",
            case.id
        );
    }
}

#[test]
fn grant_flying_activated_batch_cards_reject_activation_when_the_cost_cannot_be_paid() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_983_000 + index as u64);
        let creature = cast_creature(&mut e, &case);
        let before = pool(&e);
        assert!(
            apply_ability(&mut e, 0, creature, 0, vec![]).is_err(),
            "{}: flying needs its exact mana",
            case.id
        );
        assert_eq!(pool(&e), before, "{}: no mana is spent", case.id);
        assert!(
            !e.effective_has_keyword(creature, Keyword::Flying),
            "{}: the rejected activation grants nothing",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
