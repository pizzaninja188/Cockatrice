//! Actual-card scenarios for the reviewed pump-and-trample spell family.
//!
//! Expectations are printed Oracle values. CR 115.1 and 608.2b govern the required creature
//! target and stale-target fizzle; CR 611.2a gives the duration, CR 613.4/613.1f apply the layer-7c
//! pump and layer-6 trample, and CR 514.2 ends both during cleanup.
use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    power: u32,
    toughness: u32,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "tread_upon",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
        },
        Case {
            id: "confidence_from_strength",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            power: 4,
            toughness: 4,
        },
        Case {
            id: "awaken_the_bear",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            power: 3,
            toughness: 3,
        },
        Case {
            id: "brute_strength",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            power: 3,
            toughness: 1,
        },
        Case {
            id: "colossal_might",
            mana: ManaGift {
                r: 1,
                g: 1,
                ..Default::default()
            },
            power: 4,
            toughness: 2,
        },
        Case {
            id: "crash_the_ramparts",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            power: 3,
            toughness: 3,
        },
        Case {
            id: "fanatical_fever",
            mana: ManaGift {
                g: 2,
                c: 2,
                ..Default::default()
            },
            power: 3,
            toughness: 0,
        },
        Case {
            id: "larger_than_life",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 4,
            toughness: 4,
        },
        Case {
            id: "massive_might",
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
        },
        Case {
            id: "predators_strike",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 3,
            toughness: 3,
        },
        Case {
            id: "rush_of_adrenaline",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 1,
        },
        Case {
            id: "unnatural_predation",
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let pool = &e.state.players[0].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
}

fn assert_pool_empty(e: &GameEngine, id: &str) {
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{id}: exact cost consumed");
}

fn begin_cast_at(e: &mut GameEngine, id: &str, mana: ManaGift, target: u32) -> u32 {
    give_mana(e, 0, mana);
    let slot = inject_card_into_hand(e, 0, id);
    let hand_slot = hand_index_for_card(e, 0, id);
    semantic::accepted(e, 0, &cast_spell(hand_slot, target_object(target)));
    assert_pool_empty(e, id);
    slot
}

fn cast_at(e: &mut GameEngine, id: &str, mana: ManaGift, target: u32) -> u32 {
    let slot = begin_cast_at(e, id, mana, target);
    assert_eq!(
        e.state.stack.len(),
        1,
        "{id}: exactly the spell is on the stack"
    );
    assert_eq!(
        e.effective_power(target),
        Some(2),
        "{id}: the pump is not applied at cast time"
    );
    assert_eq!(
        e.effective_toughness(target),
        Some(2),
        "{id}: the toughness pump is not applied at cast time"
    );
    assert!(
        !e.effective_has_keyword(target, Keyword::Trample),
        "{id}: trample is not granted at cast time"
    );
    resolve_entire_stack_two_player(e);
    assert_eq!(
        e.state.objects[&slot].zone,
        Zone::Graveyard,
        "{id}: the spell goes to its owner's graveyard"
    );
    assert!(
        e.state.players[0].graveyard.contains(&slot),
        "{id}: the spell is in its owner's graveyard"
    );
    assert!(
        !e.state.players[1].graveyard.contains(&slot),
        "{id}: the spell is not in the opponent's graveyard"
    );
    slot
}

#[test]
fn pump_trample_batch_cards_cast_with_exact_printed_effects() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(740_000 + case_index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        let opponent_hand = e.state.players[1].hand.len();
        let opponent_library = e.state.players[1].library.len();
        let slot = cast_at(&mut e, case.id, case.mana, target);
        assert_eq!(
            e.effective_power(target),
            Some(2 + case.power),
            "{}: exact power pump",
            case.id
        );
        assert_eq!(
            e.effective_toughness(target),
            Some(2 + case.toughness),
            "{}: exact toughness pump",
            case.id
        );
        assert!(
            e.effective_has_keyword(target, Keyword::Trample),
            "{}: trample granted",
            case.id
        );
        assert_eq!(
            e.state.objects[&target].controller, e.state.players[1].id,
            "{}: the targeted creature remains under its controller",
            case.id
        );
        assert_eq!(
            e.state.objects[&slot].zone,
            Zone::Graveyard,
            "{}: the spell is in its owner's graveyard",
            case.id
        );
        assert_eq!(
            e.state.players[1].hand.len(),
            opponent_hand,
            "{}: opponent hand is unchanged",
            case.id
        );
        assert_eq!(
            e.state.players[1].library.len(),
            opponent_library,
            "{}: opponent library is unchanged",
            case.id
        );
    }
}

#[test]
fn pump_trample_batch_cards_expire_at_cleanup() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(741_000 + case_index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        cast_at(&mut e, case.id, case.mana, target);
        assert_eq!(e.effective_power(target), Some(2 + case.power));
        assert!(e.effective_has_keyword(target, Keyword::Trample));
        end_active_turn(&mut e, 0);
        assert_eq!(
            e.effective_power(target),
            Some(2),
            "{}: CR 514.2 restores power",
            case.id
        );
        assert_eq!(
            e.effective_toughness(target),
            Some(2),
            "{}: CR 514.2 restores toughness",
            case.id
        );
        assert!(
            !e.effective_has_keyword(target, Keyword::Trample),
            "{}: trample expires",
            case.id
        );
    }
}

#[test]
fn pump_trample_batch_cards_reject_a_noncreature_target() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(742_000 + case_index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        let before_pool = pool(&e);
        let before_command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(land)))
                .is_err(),
            "{}: a land is not a legal creature target",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand[hand_slot], slot,
            "{}: the rejected spell stays in hand",
            case.id
        );
        assert_eq!(pool(&e), before_pool, "{}: no mana is spent", case.id);
        assert!(
            e.state.stack.is_empty(),
            "{}: nothing is on the stack",
            case.id
        );
        assert_eq!(
            e.state.command_index, before_command_index,
            "{}: the rejected cast changes no command position",
            case.id
        );
    }
}

#[test]
fn pump_trample_batch_cards_fizzle_when_the_target_left() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(743_000 + case_index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        let bystander = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        let slot = begin_cast_at(&mut e, case.id, case.mana, target);
        assert_eq!(e.state.stack.len(), 1);
        assert_eq!(e.effective_power(target), Some(2));
        assert_eq!(e.effective_toughness(target), Some(2));
        assert!(!e.effective_has_keyword(target, Keyword::Trample));
        e.state.players[1]
            .battlefield
            .retain(|object| *object != target);
        e.state.players[1].graveyard.push(target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
        *e.state.zone_change_generation.entry(target).or_default() += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{}: the stale target remains in the graveyard",
            case.id
        );
        assert_eq!(
            e.state.objects[&slot].zone,
            Zone::Graveyard,
            "{}: the spell fizzles into its owner's graveyard",
            case.id
        );
        assert!(
            e.state.players[0].graveyard.contains(&slot),
            "{}: the fizzled spell is in its owner's graveyard",
            case.id
        );
        assert!(
            !e.state.players[1].graveyard.contains(&slot),
            "{}: the fizzled spell is not in the opponent's graveyard",
            case.id
        );
        assert_eq!(
            e.effective_power(bystander),
            Some(2),
            "{}: a bystander is unaffected",
            case.id
        );
        assert_eq!(
            e.effective_toughness(bystander),
            Some(2),
            "{}: a bystander's toughness is unaffected",
            case.id
        );
        assert!(
            !e.effective_has_keyword(bystander, Keyword::Trample),
            "{}: a bystander gains no trample",
            case.id
        );
    }
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
