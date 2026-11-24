//! Actual-card scenarios for the queue-v75 03-surveil-cantrips cohort.
//!
//! Expectations are printed Oracle values. CR 701.25 governs surveil (look at the top N, put any
//! number into the graveyard, the rest back on top in any order), CR 121 draws, CR 611.2c fixes
//! the affected set at resolution, CR 613.4c applies the -1/-1 modification and CR 514.2 expires
//! it at cleanup. Printed order follows CR 608.2c.
use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

struct Case {
    id: &'static str,
    mana: ManaGift,
    surveil: u32,
}

fn cases() -> [Case; 2] {
    [
        Case {
            id: "consider",
            mana: ManaGift {
                u: 1,
                ..Default::default()
            },
            surveil: 1,
        },
        Case {
            id: "curate",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            surveil: 2,
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
    let decks = Some(vec![deck_with("island", &[]), deck_with("swamp", &[])]);
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

fn pool(e: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[player].mana_pool;
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

fn surveil_prompt(e: &GameEngine, cards: u32) -> Vec<u32> {
    let pending = e
        .state
        .pending_resolution
        .as_ref()
        .expect("the surveil choice is parked");
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::LibraryLook,
        "surveil parks a library-look choice"
    );
    assert_eq!(
        (pending.presentation.min, pending.presentation.max),
        (0, cards)
    );
    pending.presentation.candidates.clone()
}

fn assert_rejected(e: &mut GameEngine, card: &str, mana: &ManaGift) {
    let object = inject_card_into_hand(e, 0, card);
    give_mana(e, 0, gift(mana));
    let before = pool(e, 0);
    let hand_before = e.state.players[0].hand.clone();
    let slot = hand_index_for_card(e, 0, card);
    let error = e
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect_err(&format!("{card}: cast must be rejected"));
    assert!(
        matches!(error, tricerules_core::EngineError::Illegal(_)),
        "{card}: expected an illegal-input error, got {error:?}"
    );
    assert_eq!(pool(e, 0), before, "{card}: rejected cast spends nothing");
    assert_eq!(
        e.state.players[0].hand, hand_before,
        "{card}: rejected cast keeps the card in hand"
    );
    assert_eq!(e.state.objects[&object].zone, Zone::Hand, "{card}");
    assert!(e.state.stack.is_empty(), "{card}: no stack object");
}

#[test]
fn q75_surveil_canary_simplest_consider_surveils_one_then_draws() {
    let case = &cases()[0];
    let mut e = engine(75_300_001);
    let bottom = put_on_top(&mut e, 0, "grizzly_bears");
    let seen = put_on_top(&mut e, 0, "storm_crow");
    let hand_before = e.state.players[0].hand.len();
    inject_card_into_hand(&mut e, 0, case.id);
    give_mana(&mut e, 0, gift(&case.mana));
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Consider");
    pass_both_players(&mut e);
    let candidates = surveil_prompt(&e, case.surveil);
    assert_eq!(candidates, vec![seen], "only the top card is looked at");
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "the draw is still pending while surveilling"
    );
    e.apply_scenario_command(0, &submit_resolution_choice(vec![seen]))
        .expect("put the looked-at card into the graveyard");
    assert_eq!(e.state.objects[&seen].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&bottom].zone, Zone::Hand, "then draw");
    assert_eq!(e.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed cost");
    assert!(e.state.stack.is_empty());
}

#[test]
fn q75_surveil_canary_hardest_mephitic_vapors_shrinks_then_surveils() {
    let mut e = engine(75_300_002);
    let own_doomed = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 1, 1);
    let opposing_doomed = inject_creature_with_stats(&mut e, 1, "storm_crow", 1, 1);
    let opposing_survivor = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 3);
    let own_survivor = inject_creature_with_stats(&mut e, 0, "hill_giant", 3, 3);
    let first = put_on_top(&mut e, 0, "grizzly_bears");
    let second = put_on_top(&mut e, 0, "storm_crow");
    inject_card_into_hand(&mut e, 0, "mephitic_vapors");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "mephitic_vapors");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Mephitic Vapors");
    pass_both_players(&mut e);
    let candidates = surveil_prompt(&e, 2);
    assert_eq!(candidates, vec![second, first]);
    for doomed in [own_doomed, opposing_doomed] {
        assert_eq!(
            e.state.objects[&doomed].zone,
            Zone::Battlefield,
            "a creature that will die to the shrink stays on the battlefield while its controller surveils"
        );
        assert_eq!(e.effective_power(doomed), Some(0), "it is already shrunk");
        assert_eq!(e.effective_toughness(doomed), Some(0));
    }
    e.apply_scenario_command(0, &submit_resolution_choice(vec![second]))
        .expect("put the top card into the graveyard");
    assert_eq!(e.state.objects[&second].zone, Zone::Graveyard);
    assert_eq!(
        e.state.objects[&first].zone,
        Zone::Library,
        "the other stays on top"
    );
    assert_eq!(
        e.state.players[0].library.front().copied(),
        Some(first),
        "the kept card is the top card"
    );
    assert_eq!(e.state.objects[&own_doomed].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&opposing_doomed].zone, Zone::Graveyard);
    assert_eq!(e.effective_power(opposing_survivor), Some(1));
    assert_eq!(e.effective_toughness(opposing_survivor), Some(2));
    assert_eq!(e.effective_power(own_survivor), Some(2));
    assert_eq!(e.effective_toughness(own_survivor), Some(2));
    assert!(e.state.stack.is_empty());
}

#[test]
fn q75_surveil_curate_surveils_two_keeps_the_rest_and_draws() {
    let case = &cases()[1];
    let mut e = engine(75_301_001);
    let deepest = put_on_top(&mut e, 0, "hill_giant");
    let kept = put_on_top(&mut e, 0, "grizzly_bears");
    let discarded = put_on_top(&mut e, 0, "storm_crow");
    inject_card_into_hand(&mut e, 0, case.id);
    give_mana(&mut e, 0, gift(&case.mana));
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Curate");
    pass_both_players(&mut e);
    let candidates = surveil_prompt(&e, case.surveil);
    assert_eq!(candidates, vec![discarded, kept]);
    e.apply_scenario_command(0, &submit_resolution_choice(vec![discarded]))
        .expect("put one looked-at card into the graveyard");
    assert_eq!(e.state.objects[&discarded].zone, Zone::Graveyard);
    assert_eq!(
        e.state.objects[&kept].zone,
        Zone::Hand,
        "the draw takes the kept card"
    );
    assert_eq!(e.state.objects[&deepest].zone, Zone::Library);
    assert_eq!(
        e.state.players[0].library.front().copied(),
        Some(deepest),
        "the untouched card stays beneath the drawn one"
    );
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0));
}

#[test]
fn q75_surveil_cantrips_reject_underfunded_casts() {
    for (index, case) in cases().into_iter().enumerate() {
        let seed = 75_302_000 + index as u64;
        let mut e = engine(seed);
        let underfunded = ManaGift {
            w: case.mana.w.saturating_sub(1),
            u: case.mana.u.saturating_sub(1),
            b: case.mana.b.saturating_sub(1),
            r: case.mana.r.saturating_sub(1),
            g: case.mana.g.saturating_sub(1),
            c: case.mana.c.saturating_sub(1),
        };
        assert_rejected(&mut e, case.id, &underfunded);
    }
    let mut e = engine(75_302_100);
    assert_rejected(
        &mut e,
        "mephitic_vapors",
        &ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
}

#[test]
fn q75_surveil_mephitic_vapors_affects_only_creatures_present_at_resolution() {
    let mut e = engine(75_303_001);
    put_on_top(&mut e, 0, "forest");
    put_on_top(&mut e, 0, "island");
    inject_card_into_hand(&mut e, 0, "mephitic_vapors");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "mephitic_vapors");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Mephitic Vapors");
    pass_both_players(&mut e);
    let candidates = surveil_prompt(&e, 2);
    e.apply_scenario_command(0, &submit_resolution_choice(candidates))
        .expect("surveil the top cards");
    let late = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 3, 3);
    assert_eq!(
        e.effective_power(late),
        Some(3),
        "a creature that enters later is not affected"
    );
    assert_eq!(e.effective_toughness(late), Some(3));
    assert!(e.state.stack.is_empty());
}
