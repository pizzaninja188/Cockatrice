//! Actual-card scenarios for the queue-v79 03-counter-riders cohort.
//!
//! Expectations are printed Oracle values. CR 701.6 counters the spell; CR 608.2b removes a
//! spell only when every target is illegal, so a stale land target still allows the other
//! effects; CR 121.1 draws, CR 701.13 mills, CR 701.26 untaps and CR 701.16a creates Clues.
//! The pinned Confirm Suspicions ruling confirms a spell with no legal target does not resolve
//! and does not investigate. Basic landcycling is the CR 702.29a-b hand activated ability.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ruled_event::Ev, AbilitySourceZone, ActivateAbility, RuledCommand,
    TargetRef,
};

fn seat_on_top(e: &mut GameEngine, player: usize, ids: &[u32]) {
    for id in ids.iter().rev() {
        let library = &mut e.state.players[player].library;
        library.retain(|candidate| candidate != id);
        library.push_front(*id);
    }
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("island", &[])]);
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

fn hand_ability(e: &GameEngine, source: u32, ability_index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: e
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index,
            ..Default::default()
        })),
    }
}

fn two_targets(first: u32, second: u32) -> Vec<TargetRef> {
    let mut targets = target_object(first);
    let mut second_group = target_object(second);
    for target in &mut second_group {
        target.group_index = 1;
    }
    targets.extend(second_group);
    targets
}

fn cast_bolt(e: &mut GameEngine) -> u32 {
    inject_card_into_hand(e, 0, "lightning_bolt");
    give_mana(
        e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, 0, "lightning_bolt");
    e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
        .expect("cast Lightning Bolt");
    e.state.stack.last().expect("Lightning Bolt").id
}

fn cast_counter(e: &mut GameEngine, id: &str, mana: ManaGift, targets: Vec<TargetRef>) -> u32 {
    inject_card_into_hand(e, 1, id);
    give_mana(e, 1, mana);
    let slot = hand_index_for_card(e, 1, id);
    e.apply_scenario_command(1, &cast_spell(slot, targets))
        .unwrap_or_else(|error| panic!("cast {id}: {error:?}"));
    e.state.stack.last().expect("counter spell").id
}

#[test]
fn q79_counter_riders_canary_simplest_dismissive_denial_counters() {
    let mut e = engine(79_300_001);
    let bolt = cast_bolt(&mut e);
    e.apply_scenario_command(0, &pass()).expect("active passes");
    let denial = cast_counter(
        &mut e,
        "dismissive_denial",
        ManaGift {
            u: 2,
            c: 2,
            ..Default::default()
        },
        target_object(bolt),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bolt].zone, Zone::Graveyard);
    assert_eq!(
        e.state.players[1].life, 20,
        "the countered Bolt deals no damage"
    );
    assert_eq!(e.state.objects[&denial].zone, Zone::Graveyard);
    assert!(e.state.stack.is_empty());
}

#[test]
fn q79_counter_riders_canary_hardest_sokkas_haiku_draws_mills_and_untaps() {
    let mut e = engine(79_300_002);
    let bolt = cast_bolt(&mut e);
    let land = inject_permanent_on_battlefield(&mut e, 1, "island");
    let other_land = inject_permanent_on_battlefield(&mut e, 1, "island");
    e.state.objects.get_mut(&land).expect("target land").tapped = true;
    e.state
        .objects
        .get_mut(&other_land)
        .expect("other land")
        .tapped = true;
    let drawn = inject_library_card(&mut e, 1, "forest");
    let milled_one = inject_library_card(&mut e, 1, "mountain");
    let milled_two = inject_library_card(&mut e, 1, "plains");
    let milled_three = inject_library_card(&mut e, 1, "swamp");
    seat_on_top(&mut e, 1, &[drawn, milled_one, milled_two, milled_three]);
    let library_before = e.state.players[1].library.len();
    let hand_before = e.state.players[1].hand.len();
    e.apply_scenario_command(0, &pass()).expect("active passes");
    cast_counter(
        &mut e,
        "sokkas_haiku",
        ManaGift {
            u: 2,
            c: 3,
            ..Default::default()
        },
        two_targets(bolt, land),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bolt].zone, Zone::Graveyard);
    assert_eq!(e.state.players[1].life, 20);
    assert!(
        !e.state.objects[&land].tapped,
        "the targeted land is untapped"
    );
    assert!(
        e.state.objects[&other_land].tapped,
        "only the targeted land is untapped"
    );
    assert_eq!(
        e.state.players[1].hand.len(),
        hand_before + 1,
        "Sokka's Haiku draws one"
    );
    assert_eq!(
        e.state.players[1].library.len(),
        library_before - 4,
        "one draw plus three milled cards"
    );
    assert_eq!(
        e.state.objects[&drawn].zone,
        Zone::Hand,
        "the first card is drawn before milling"
    );
    for milled in [milled_one, milled_two, milled_three] {
        assert_eq!(
            e.state.objects[&milled].zone,
            Zone::Graveyard,
            "the next three cards are milled"
        );
    }
    assert!(e.state.stack.is_empty());
}

#[test]
fn q79_counter_riders_confirm_suspicions_creates_three_clues() {
    let mut e = engine(79_301_001);
    let bolt = cast_bolt(&mut e);
    e.apply_scenario_command(0, &pass()).expect("active passes");
    cast_counter(
        &mut e,
        "confirm_suspicions",
        ManaGift {
            u: 2,
            c: 3,
            ..Default::default()
        },
        target_object(bolt),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bolt].zone, Zone::Graveyard);
    let clues = battlefield_token_oids(&e, 1, "clue");
    assert_eq!(
        clues.len(),
        3,
        "investigate three times creates three Clues"
    );
}

#[test]
fn q79_counter_riders_confirm_suspicions_stale_target_creates_no_clues() {
    let mut e = engine(79_302_001);
    let bolt = cast_bolt(&mut e);
    e.apply_scenario_command(0, &pass()).expect("active passes");
    cast_counter(
        &mut e,
        "confirm_suspicions",
        ManaGift {
            u: 2,
            c: 3,
            ..Default::default()
        },
        target_object(bolt),
    );
    e.apply_scenario_command(1, &pass())
        .expect("counter caster passes priority");
    // The Bolt's controller counters its own spell; Confirm Suspicions then has no legal target.
    inject_card_into_hand(&mut e, 0, "counterspell");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "counterspell");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(bolt)))
        .expect("counter the Bolt");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bolt].zone, Zone::Graveyard);
    assert!(
        battlefield_token_oids(&e, 1, "clue").is_empty(),
        "a spell with no legal target does not investigate"
    );
}

#[test]
fn q79_counter_riders_sokkas_haiku_still_resolves_when_the_spell_target_is_gone() {
    let mut e = engine(79_303_001);
    let bolt = cast_bolt(&mut e);
    let land = inject_permanent_on_battlefield(&mut e, 1, "island");
    e.state.objects.get_mut(&land).expect("target land").tapped = true;
    let library_before = e.state.players[1].library.len();
    let hand_before = e.state.players[1].hand.len();
    e.apply_scenario_command(0, &pass()).expect("active passes");
    cast_counter(
        &mut e,
        "sokkas_haiku",
        ManaGift {
            u: 2,
            c: 3,
            ..Default::default()
        },
        two_targets(bolt, land),
    );
    e.apply_scenario_command(1, &pass())
        .expect("counter caster passes priority");
    inject_card_into_hand(&mut e, 0, "counterspell");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "counterspell");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(bolt)))
        .expect("counter the Bolt");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bolt].zone, Zone::Graveyard);
    assert!(
        !e.state.objects[&land].tapped,
        "the remaining legal land target still untaps"
    );
    assert_eq!(e.state.players[1].life, 20);
    assert_eq!(
        e.state.players[1].hand.len(),
        hand_before + 1,
        "the stale spell target still allows the draw"
    );
    assert_eq!(
        e.state.players[1].library.len(),
        library_before - 4,
        "the stale spell target still allows the draw and mill"
    );
}

#[test]
fn q79_counter_riders_dismissive_denial_basic_landcycling_searches_from_hand() {
    let decks = Some(vec![
        vec!["grizzly_bears".to_string(); 20],
        vec!["forest".to_string(); 20],
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        79_304_001,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    let source = inject_card_into_hand(&mut e, 0, "dismissive_denial");
    let mountain = inject_library_card(&mut e, 0, "mountain");
    let forest = inject_library_card(&mut e, 0, "forest");
    let nonbasic = inject_library_card(&mut e, 0, "taiga");
    e.apply_scenario_command(0, &hand_ability(&e, source, 0))
        .expect_err("the full {2} cost is checked before discarding");
    assert_eq!(e.state.objects[&source].zone, Zone::Hand);
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    e.apply_scenario_command(0, &hand_ability(&e, source, 0))
        .expect("activate basic landcycling");
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    e.apply_scenario_command(0, &pass())
        .expect("controller passes");
    let search = e
        .apply_scenario_command(1, &pass())
        .expect("basic landcycling resolves to a private search");
    let choice = find_resolution_choice(&search).expect("private library search");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(
        choice.candidate_object_ids,
        vec![mountain, forest],
        "basic landcycling excludes the nonbasic Taiga"
    );
    assert!(!choice.candidate_object_ids.contains(&nonbasic));
    let completed = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![forest]))
        .expect("choose the Forest");
    assert_eq!(e.state.objects[&forest].zone, Zone::Hand);
    assert_eq!(e.state.objects[&mountain].zone, Zone::Library);
    assert!(completed.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::CardsRevealed(reveal))
            if reveal.cards.len() == 1 && reveal.cards[0].object_id == forest
    )));
    assert!(completed.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
    )));
}

#[test]
fn q79_counter_riders_cast_exact_printed_costs() {
    for (index, (id, mana)) in [
        (
            "confirm_suspicions",
            ManaGift {
                u: 2,
                c: 3,
                ..Default::default()
            },
        ),
        (
            "sokkas_haiku",
            ManaGift {
                u: 2,
                c: 3,
                ..Default::default()
            },
        ),
        (
            "dismissive_denial",
            ManaGift {
                u: 2,
                c: 2,
                ..Default::default()
            },
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(79_305_000 + index as u64);
        let bolt = cast_bolt(&mut e);
        e.apply_scenario_command(0, &pass()).expect("active passes");
        let land = inject_permanent_on_battlefield(&mut e, 1, "island");
        let targets = if id == "sokkas_haiku" {
            two_targets(bolt, land)
        } else {
            target_object(bolt)
        };
        cast_counter(&mut e, id, mana, targets);
        let pool = &e.state.players[1].mana_pool;
        assert_eq!(
            (
                pool.white,
                pool.blue,
                pool.black,
                pool.red,
                pool.green,
                pool.colorless
            ),
            (0, 0, 0, 0, 0, 0),
            "{id}: exact printed cost"
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&bolt].zone, Zone::Graveyard, "{id}");
        assert_eq!(e.state.players[0].life, 20, "{id}: Bolt was countered");
    }
}
