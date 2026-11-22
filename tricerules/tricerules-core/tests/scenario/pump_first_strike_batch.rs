//! Actual-card scenarios for the reviewed pump-and-first-strike spell family.
//!
//! Expectations are printed Oracle values. CR 115.1 and 608.2b govern the required
//! creature target and stale-target fizzle; CR 611.2a gives the duration, CR
//! 613.4c applies the layer-7c pump and CR 613.1f the layer-6 first-strike grant;
//! CR 514.2 ends both during cleanup.
use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 6] {
    [
        Case {
            id: "fit_of_rage",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            power: 3,
            toughness: 3,
        },
        Case {
            id: "precise_strike",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 0,
        },
        Case {
            id: "rise_to_the_challenge",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 0,
        },
        Case {
            id: "seize_the_initiative",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        Case {
            id: "skillful_lunge",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 0,
        },
        Case {
            id: "zealous_strike",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
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
    let hand_slot = e.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == slot)
        .expect("the injected card is in hand");
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
        "{id}: pump is not applied at cast time"
    );
    assert_eq!(
        e.effective_toughness(target),
        Some(2),
        "{id}: toughness pump is not applied at cast time"
    );
    assert!(
        !e.effective_has_keyword(target, Keyword::FirstStrike),
        "{id}: first strike is not granted at cast time"
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
fn pump_first_strike_batch_cards_cast_with_exact_printed_effects() {
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
            e.effective_has_keyword(target, Keyword::FirstStrike),
            "{}: first strike granted",
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
fn pump_first_strike_batch_cards_expire_at_cleanup() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(741_000 + case_index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        cast_at(&mut e, case.id, case.mana, target);
        assert_eq!(e.effective_power(target), Some(2 + case.power));
        assert!(e.effective_has_keyword(target, Keyword::FirstStrike));
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
            !e.effective_has_keyword(target, Keyword::FirstStrike),
            "{}: first strike expires",
            case.id
        );
    }
}

#[test]
fn pump_first_strike_batch_cards_reject_a_noncreature_target() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(742_000 + case_index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        give_mana(
            &mut e,
            0,
            ManaGift {
                w: case.mana.w,
                u: case.mana.u,
                b: case.mana.b,
                r: case.mana.r,
                g: case.mana.g,
                c: case.mana.c,
            },
        );
        let slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = e.state.players[0]
            .hand
            .iter()
            .position(|oid| *oid == slot)
            .expect("the injected card is in hand");
        let before_pool = pool(&e);
        let before_command_index = e.state.command_index;
        for (illegal, label) in [(target_object(land), "land"), (target_player(1), "player")] {
            assert!(
                e.apply_scenario_command(0, &cast_spell(hand_slot, illegal))
                    .is_err(),
                "{}: a {label} is not a legal creature target",
                case.id
            );
            assert_eq!(
                e.state.players[0].hand[hand_slot], slot,
                "{}: the rejected {label} cast stays in hand",
                case.id
            );
            assert_eq!(
                pool(&e),
                before_pool,
                "{}: no mana is spent on the {label} cast",
                case.id
            );
            assert!(
                e.state.stack.is_empty(),
                "{}: nothing is on the stack after the {label} cast",
                case.id
            );
            assert_eq!(
                e.state.command_index, before_command_index,
                "{}: the rejected {label} cast changes no command position",
                case.id
            );
        }
    }
}

#[test]
fn pump_first_strike_batch_cards_reject_a_cast_without_the_printed_mana() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(742_500 + case_index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        let slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = e.state.players[0]
            .hand
            .iter()
            .position(|oid| *oid == slot)
            .expect("the injected card is in hand");
        let before = e.state.command_index;
        e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(target)))
            .expect_err("the printed mana cost is required");
        assert_eq!(e.state.command_index, before, "{}", case.id);
        assert_eq!(e.state.players[0].hand[hand_slot], slot, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}", case.id);
        assert_eq!(e.effective_power(target), Some(2), "{}", case.id);
    }
}

#[test]
fn pump_first_strike_batch_cards_fizzle_when_the_target_left() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(743_000 + case_index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        let bystander = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        let slot = begin_cast_at(&mut e, case.id, case.mana, target);
        assert_eq!(e.state.stack.len(), 1);
        assert_eq!(e.effective_power(target), Some(2));
        assert_eq!(e.effective_toughness(target), Some(2));
        assert!(!e.effective_has_keyword(target, Keyword::FirstStrike));
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
            "{}: a bystander is unaffected",
            case.id
        );
        assert!(
            !e.effective_has_keyword(bystander, Keyword::FirstStrike),
            "{}: a bystander gains nothing",
            case.id
        );
    }
}
