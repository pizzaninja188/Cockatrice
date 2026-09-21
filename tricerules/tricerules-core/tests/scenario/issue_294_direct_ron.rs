//! Issue #294 — reviewed direct-RON scenarios for Whoosh! and Into the Roil.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-20 against the pinned snapshot
//! identities `855353e2-b686-482a-b987-c6ae456f088c` (Whoosh!) and
//! `c2898bbd-82a4-4d26-b6ee-169b0ebe71b4` (Into the Roil). Both print `Kicker {1}{U}` and then
//! return target nonland permanent to its owner's hand, drawing a card only if kicked.
//!
//! CR 702.33a-e (kicker), 601.2b/f-h (additional costs paid during casting), 607.2i and 707.10
//! (the announced receipt and copy decisions), 608.2b (target revalidation), 400.3 (owner's
//! zone), and 121.1-121.2 (draw) govern. The 2020-09-25 ruling confirms an illegal sole target
//! prevents the kicked draw; the 2024-11-08 ruling confirms a copy of a kicked spell is kicked.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};

const CARDS: [(&str, u64); 2] = [("whoosh!", 294_101), ("into_the_roil", 294_102)];

fn kicker_selection() -> CastCostGroupSelection {
    CastCostGroupSelection {
        group_index: 0,
        option_index: 0,
        ..Default::default()
    }
}

#[test]
fn issue_294_base_and_kicked_casts_bounce_then_optionally_draw() {
    for (card, seed) in CARDS {
        for kicked in [false, true] {
            let mut engine = semantic::main_phase(seed + u64::from(kicked));
            // Whoosh! proves the creature case; Into the Roil additionally exercises a
            // noncreature nonland permanent against the shared AnyPermanent-minus-Land filter.
            let target = if card == "into_the_roil" {
                inject_permanent_on_battlefield(&mut engine, 1, "explosive_apparatus")
            } else {
                inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears")
            };
            let source = inject_card_into_hand(&mut engine, 0, card);
            let top = engine.state.players[0].library.front().copied().unwrap();
            grant_pool(&mut engine, 0);
            let slot = hand_index_for_card(&engine, 0, card);
            let hand_before = engine.state.players[0].hand.len();
            let selections = if kicked {
                vec![kicker_selection()]
            } else {
                Vec::new()
            };

            semantic::accepted(
                &mut engine,
                0,
                &cast_spell_with_cast_cost_groups(slot, target_object(target), selections),
            );

            // CR 702.33a / 601.2f-h: the additional {1}{U} is paid during casting, before any
            // player passes, and the stable receipt rides on the stack item.
            let item = engine.state.stack.last().expect("spell on the stack");
            assert_eq!(item.card_id, card);
            assert_eq!(
                item.cast_cost_receipts.len(),
                usize::from(kicked),
                "{card} receipt"
            );
            assert_eq!(
                engine.state.players[0].mana_pool.blue,
                if kicked { 7 } else { 8 },
                "{card} blue paid at cast time"
            );
            assert_eq!(
                engine.state.players[0].mana_pool.colorless,
                if kicked { 7 } else { 8 },
                "{card} generic paid at cast time"
            );

            semantic::complete(&mut engine, 8, |_| None).require_exercised();

            assert_eq!(
                engine.state.objects[&target].zone,
                Zone::Hand,
                "{card} permanent returned to its owner's hand"
            );
            assert!(engine.state.players[1].hand.contains(&target), "{card}");
            assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
            assert_eq!(
                engine.state.players[0].hand.len(),
                hand_before - 1 + usize::from(kicked),
                "{card} draws exactly one card only when kicked"
            );
            assert_eq!(
                engine.state.objects[&top].zone,
                if kicked { Zone::Hand } else { Zone::Library },
                "{card} top card drawn only when kicked"
            );
        }
    }
}

#[test]
fn issue_294_return_uses_the_owner_not_the_controller() {
    for (card, seed) in CARDS {
        let mut engine = semantic::main_phase(seed + 10);
        // Owned by P0, controlled by P1 (CR 108.3 / 110.2).
        let reanimated = inject_creature_under_foreign_control(&mut engine, 0, 1, "grizzly_bears");
        inject_card_into_hand(&mut engine, 0, card);
        grant_pool(&mut engine, 0);
        let slot = hand_index_for_card(&engine, 0, card);

        semantic::accepted(&mut engine, 0, &cast_spell(slot, target_object(reanimated)));
        semantic::complete(&mut engine, 8, |_| None).require_exercised();

        // CR 400.3: the hand it reaches is the owner's, not the controller's.
        assert!(engine.state.players[0].hand.contains(&reanimated), "{card}");
        assert!(
            !engine.state.players[1].hand.contains(&reanimated),
            "{card}"
        );
        assert_eq!(engine.state.objects[&reanimated].owner, 0);
    }
}

#[test]
fn issue_294_rejects_land_targets_and_unfunded_kicker_atomically() {
    for (card, seed) in CARDS {
        let mut engine = semantic::main_phase(seed + 20);
        let land = inject_permanent_on_battlefield(&mut engine, 1, "island");
        let source = inject_card_into_hand(&mut engine, 0, card);
        grant_pool(&mut engine, 0);
        let slot = hand_index_for_card(&engine, 0, card);

        let commands_before = engine.state.command_index;
        engine
            .apply_command(0, &cast_spell(slot, target_object(land)))
            .expect_err("a land is not a legal nonland-permanent target");
        assert_eq!(engine.state.command_index, commands_before);
        assert_eq!(engine.state.objects[&source].zone, Zone::Hand);

        // Fund only the printed base {1}{U}; the announced additional {1}{U} is unpayable.
        let creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        {
            let pool = &mut engine.state.players[0].mana_pool;
            pool.white = 0;
            pool.blue = 1;
            pool.black = 0;
            pool.red = 0;
            pool.green = 0;
            pool.colorless = 1;
        }
        let commands_before = engine.state.command_index;
        engine
            .apply_command(
                0,
                &cast_spell_with_cast_cost_groups(
                    slot,
                    target_object(creature),
                    vec![kicker_selection()],
                ),
            )
            .expect_err("the announced kicker requires its additional mana at cast time");
        assert_eq!(engine.state.command_index, commands_before);
        assert_eq!(engine.state.players[0].mana_pool.blue, 1);
        assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
        assert_eq!(engine.state.objects[&creature].zone, Zone::Battlefield);
        assert!(engine.state.stack.is_empty());

        // Positive control: the same cast is accepted once the kicker is not announced, so the
        // two rejections above are target/payment semantics, not a missing or broken definition.
        grant_pool(&mut engine, 0);
        semantic::accepted(&mut engine, 0, &cast_spell(slot, target_object(creature)));
        semantic::complete(&mut engine, 8, |_| None).require_exercised();
        assert_eq!(engine.state.objects[&creature].zone, Zone::Hand);
    }
}

#[test]
fn issue_294_an_illegal_sole_target_prevents_bounce_and_kicked_draw() {
    for (card, seed) in CARDS {
        let mut engine = semantic::main_phase(seed + 30);
        let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        inject_card_into_hand(&mut engine, 0, card);
        inject_card_into_hand(&mut engine, 0, "unsummon");
        grant_pool(&mut engine, 0);
        let slot = hand_index_for_card(&engine, 0, card);
        let hand_before = engine.state.players[0].hand.len();
        let source = engine.state.players[0].hand[slot];
        let top = engine.state.players[0].library.front().copied().unwrap();

        semantic::accepted(
            &mut engine,
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                target_object(target),
                vec![kicker_selection()],
            ),
        );
        let unsummon = hand_index_for_card(&engine, 0, "unsummon");
        semantic::accepted(&mut engine, 0, &cast_spell(unsummon, target_object(target)));
        resolve_entire_stack_two_player(&mut engine);

        // 2020-09-25 ruling and CR 608.2b: the only target is illegal, so the spell doesn't
        // resolve and the kicked draw never happens.
        assert_eq!(engine.state.objects[&target].zone, Zone::Hand, "{card}");
        assert!(engine.state.players[1].hand.contains(&target));
        assert_eq!(
            engine.state.objects[&source].zone,
            Zone::Graveyard,
            "{card}"
        );
        assert_eq!(
            engine.state.players[0].hand.len(),
            hand_before - 2,
            "{card}: spell and Unsummon left, no draw"
        );
        assert_eq!(
            engine.state.objects[&top].zone,
            Zone::Library,
            "{card} library untouched"
        );
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn issue_294_a_countered_kicked_cast_never_bounces_or_draws() {
    for (card, seed) in CARDS {
        let mut engine = semantic::main_phase(seed + 40);
        let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        inject_card_into_hand(&mut engine, 0, card);
        inject_card_into_hand(&mut engine, 1, "counterspell");
        grant_pool(&mut engine, 0);
        grant_pool(&mut engine, 1);
        let slot = hand_index_for_card(&engine, 0, card);
        let hand_before = engine.state.players[0].hand.len();
        let source = engine.state.players[0].hand[slot];
        let top = engine.state.players[0].library.front().copied().unwrap();

        semantic::accepted(
            &mut engine,
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                target_object(target),
                vec![kicker_selection()],
            ),
        );
        let spell = engine.state.stack.last().unwrap().id;
        let mut spell_target = target_object(spell);
        spell_target[0].kind = TargetRefKind::Stack as i32;
        // P0 passes so the opponent receives priority and can respond.
        semantic::accepted(&mut engine, 0, &pass());
        let counterspell = hand_index_for_card(&engine, 1, "counterspell");
        semantic::accepted(&mut engine, 1, &cast_spell(counterspell, spell_target));
        resolve_entire_stack_two_player(&mut engine);

        // CR 701.6 and 608.2b: the countered spell never resolves, but its cast-time kicker
        // payment is still spent (CR 601.2h).
        assert_eq!(
            engine.state.objects[&target].zone,
            Zone::Battlefield,
            "{card} target untouched"
        );
        assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
        assert_eq!(
            engine.state.players[0].hand.len(),
            hand_before - 1,
            "{card} no draw"
        );
        assert_eq!(engine.state.objects[&top].zone, Zone::Library);
        assert_eq!(
            engine.state.players[0].mana_pool.blue, 7,
            "{card} kicker paid at cast time"
        );
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn issue_294_a_copy_of_a_kicked_spell_is_kicked_without_repaying() {
    for (card, seed) in CARDS {
        let decks = Some(vec![
            deck_with("island", &[card]),
            deck_with("island", &["twincast"]),
        ]);
        let mut engine = GameEngine::new(seed + 50, &[0, 1], 20, decks, true).expect("engine");
        advance_to_main1_from_game_start(&mut engine);
        ensure_card_in_hand(&mut engine, 0, card);
        ensure_card_in_hand(&mut engine, 1, "twincast");
        let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        let opposing = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        grant_pool(&mut engine, 0);
        grant_pool(&mut engine, 1);
        let slot = hand_index_for_card(&engine, 0, card);
        let p0_hand_before = engine.state.players[0].hand.len();
        let p1_hand_before = engine.state.players[1].hand.len();
        let p1_blue_before = engine.state.players[1].mana_pool.blue;

        semantic::accepted(
            &mut engine,
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                target_object(opposing),
                vec![kicker_selection()],
            ),
        );
        let original = engine.state.stack[0].clone();
        assert_eq!(original.cast_cost_receipts.len(), 1);

        semantic::accepted(&mut engine, 0, &pass());
        let mut twincast_target = target_object(original.id);
        twincast_target[0].kind = TargetRefKind::Stack as i32;
        let twincast = hand_index_for_card(&engine, 1, "twincast");
        semantic::accepted(&mut engine, 1, &cast_spell(twincast, twincast_target));
        semantic::accepted(&mut engine, 1, &pass());
        semantic::accepted(&mut engine, 0, &pass());
        assert!(
            engine.state.pending_resolution.is_some(),
            "the copy awaits its target choice"
        );

        // CR 707.10: the copy copies the additional-cost decision, so it is kicked too.
        semantic::accepted(&mut engine, 1, &submit_resolution_choice(vec![own]));
        let copy = engine
            .state
            .stack
            .iter()
            .find(|item| item.is_copy)
            .expect("copy")
            .clone();
        assert_eq!(copy.cast_cost_receipts, original.cast_cost_receipts);

        resolve_entire_stack_two_player(&mut engine);

        assert_eq!(engine.state.objects[&own].zone, Zone::Hand);
        assert_eq!(engine.state.objects[&opposing].zone, Zone::Hand);
        assert_eq!(
            engine.state.players[0].hand.len(),
            p0_hand_before + 1,
            "{card}: bounce plus the original spell's draw"
        );
        assert_eq!(
            engine.state.players[1].hand.len(),
            p1_hand_before + 1,
            "{card}: the copy is kicked and draws for its controller"
        );
        assert_eq!(
            engine.state.players[1].mana_pool.blue,
            p1_blue_before - 2,
            "{card}: the copy never repays the kicker"
        );
        assert_eq!(engine.state.players[1].mana_pool.colorless, 9);
    }
}
