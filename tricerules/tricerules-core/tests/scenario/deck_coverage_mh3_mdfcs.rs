//! Card-specific ruled scenarios for Bridgeworks Battle // Tanglespan Bridgeworks and
//! Sundering Eruption // Volcanic Fissure.
//!
//! Oracle and rulings checked 2026-09-27. CR 712.11b-c / 712.12 govern face choice; CR 614.1c
//! and 118.3b govern the 3-life entry replacement; CR 701.14 governs fight; CR 608.2b rechecks
//! targets; CR 701.23-24 govern optional search, tapped placement, and shuffle; CR 509.1b and
//! 611.2a-c govern the dynamic until-end-of-turn blocking restriction.

use super::helpers::*;
use tricerules_cards::{ContinuousEffectKind, CounterKind, EffectDuration, Keyword};
use tricerules_core::{GameEngine, TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    ruled_event::Ev, BlockPair, ChoiceKind, ResolutionChoiceDecision, TargetRef, TargetRefKind,
};

const BRIDGEWORKS: &str = "bridgeworks_battle_tanglespan_bridgeworks";
const SUNDERING: &str = "sundering_eruption_volcanic_fissure";

fn permanent_target(object_id: u32, group_index: u32) -> TargetRef {
    TargetRef {
        object_id,
        group_index,
        kind: TargetRefKind::Permanent as i32,
        ..Default::default()
    }
}

fn bridgeworks_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_bridgeworks(engine: &mut GameEngine, targets: Vec<TargetRef>) -> u32 {
    let spell = inject_card_into_hand(engine, 0, BRIDGEWORKS);
    give_mana(
        engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, BRIDGEWORKS);
    engine
        .apply_command(0, &cast_spell_face(slot, targets, 0))
        .expect("cast the Bridgeworks Battle face");
    spell
}

fn leave_battlefield_to_hand(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].hand.push(object_id);
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("object")
        .zone = Zone::Hand;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

fn land_engine(seed: u64, starting_life: i32) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], starting_life, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn advance_main1_to_declare_attackers(engine: &mut GameEngine) {
    let active_player = engine.state.active_player_id();
    let defending_player = engine
        .state
        .sole_defending_player_id()
        .expect("two-player combat has one defender");
    engine
        .apply_command(active_player, &primitive_yield())
        .expect("main1 to beginning of combat");
    engine
        .apply_command(active_player, &pass())
        .expect("active player passes in beginning of combat");
    engine
        .apply_command(defending_player, &pass())
        .expect("defender passes in beginning of combat");
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
}

fn pass_to_declare_blockers(engine: &mut GameEngine) {
    let active_player = engine.state.active_player_id();
    let defending_player = engine
        .state
        .sole_defending_player_id()
        .expect("two-player combat has one defender");
    engine
        .apply_command(active_player, &pass())
        .expect("active player passes after attackers are declared");
    engine
        .apply_command(defending_player, &pass())
        .expect("defender passes after attackers are declared");
    assert_eq!(engine.state.turn_step, TurnStep::DeclareBlockers);
}

fn play_mdfc_land(
    engine: &mut GameEngine,
    card_id: &str,
) -> (u32, tricerules_proto::ruled::v1::RuledEventBatch) {
    let land = inject_card_into_hand(engine, 0, card_id);
    let slot = hand_index_for_card(engine, 0, card_id);
    let batch = engine
        .apply_command(0, &play_land_face(slot, 1))
        .expect("play the land face");
    (land, batch)
}

#[test]
fn bridgeworks_requires_its_own_target_and_fights_only_when_the_optional_target_is_chosen() {
    let mut engine = bridgeworks_engine(20_260_927_301);
    let own = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 1, 1);
    let opponent = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let spell = inject_card_into_hand(&mut engine, 0, BRIDGEWORKS);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, BRIDGEWORKS);
    let offer = engine.initial_response_batch();
    let legal = offer.legal_by_player[&0]
        .valid_targets_by_hand_slot
        .get(&((slot as u32) << 8))
        .expect("Bridgeworks publishes both target groups");
    assert_eq!(legal.groups.len(), 2);
    assert_eq!(legal.groups[0].valid_permanent_ids, [own]);
    assert_eq!(legal.groups[1].valid_permanent_ids, [opponent]);

    assert!(
        engine
            .apply_command(
                0,
                &cast_spell_face(slot, vec![permanent_target(opponent, 0)], 0),
            )
            .is_err(),
        "the required first target must be a creature its caster controls"
    );
    engine
        .apply_command(0, &cast_spell_face(slot, vec![permanent_target(own, 0)], 0))
        .expect("the optional opposing fight target may be omitted");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.effective_power(own), Some(3));
    assert_eq!(engine.effective_toughness(own), Some(3));
    assert_eq!(engine.state.objects[&opponent].damage, 0);
    assert_eq!(engine.state.objects[&opponent].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn bridgeworks_pumps_before_fight_and_keeps_the_pump_when_its_optional_target_becomes_illegal() {
    let mut ordered = bridgeworks_engine(20_260_927_302);
    let own = inject_creature_with_stats(&mut ordered, 0, "grizzly_bears", 1, 1);
    let opponent = inject_creature_with_stats(&mut ordered, 1, "grizzly_bears", 2, 2);
    cast_bridgeworks(
        &mut ordered,
        vec![permanent_target(own, 0), permanent_target(opponent, 1)],
    );
    resolve_entire_stack_two_player(&mut ordered);
    assert_eq!(ordered.effective_power(own), Some(3));
    assert_eq!(ordered.effective_toughness(own), Some(3));
    assert_eq!(ordered.state.objects[&own].damage, 2);
    assert_eq!(
        ordered.state.objects[&own].zone,
        Zone::Battlefield,
        "the +2/+2 happens before the fight: the 1/1 survives the 2/2's return damage"
    );
    assert_eq!(
        ordered.state.objects[&opponent].zone,
        Zone::Graveyard,
        "the pumped creature deals lethal 3 damage during the fight"
    );

    let mut partial = bridgeworks_engine(20_260_927_303);
    let own = inject_creature_with_stats(&mut partial, 0, "grizzly_bears", 1, 1);
    let opponent = inject_creature_with_stats(&mut partial, 1, "grizzly_bears", 2, 2);
    cast_bridgeworks(
        &mut partial,
        vec![permanent_target(own, 0), permanent_target(opponent, 1)],
    );
    leave_battlefield_to_hand(&mut partial, 1, opponent);
    resolve_entire_stack_two_player(&mut partial);
    assert_eq!(partial.effective_power(own), Some(3));
    assert_eq!(partial.effective_toughness(own), Some(3));
    assert_eq!(partial.state.objects[&own].damage, 0);
    assert_eq!(partial.state.objects[&opponent].zone, Zone::Hand);
}

#[test]
fn both_mh3_land_faces_offer_three_life_and_produce_their_exact_color() {
    for (card_id, expected_red, expected_green, seed) in [
        (BRIDGEWORKS, 0, 1, 20_260_927_311),
        (SUNDERING, 1, 0, 20_260_927_312),
    ] {
        let mut paid = land_engine(seed, 20);
        let (land, offer) = play_mdfc_land(&mut paid, card_id);
        assert_eq!(paid.state.objects[&land].zone, Zone::Hand);
        assert_eq!(paid.state.players[0].life, 20);
        let choice = find_resolution_choice(&offer).expect("3-life entry choice");
        assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
        assert_eq!(choice.deciding_player_id, 0);
        assert_eq!((choice.min, choice.max), (0, 1));
        assert!(choice.resolution_branches[0].selectable);
        paid.apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("pay 3 life");
        assert_eq!(paid.state.players[0].life, 17);
        assert_eq!(paid.state.objects[&land].zone, Zone::Battlefield);
        assert_eq!(paid.state.objects[&land].face_up_index, 1);
        assert!(!paid.state.objects[&land].tapped);
        apply_ability(&mut paid, 0, land, 0, vec![]).expect("tap for the printed color");
        assert_eq!(paid.state.players[0].mana_pool.red, expected_red);
        assert_eq!(paid.state.players[0].mana_pool.green, expected_green);
        assert_eq!(paid.state.players[0].mana_pool.blue, 0);
        assert_eq!(paid.state.players[0].mana_pool.white, 0);
        assert_eq!(paid.state.players[0].mana_pool.black, 0);
        assert_eq!(paid.state.players[0].mana_pool.colorless, 0);

        let mut declined = land_engine(seed + 1, 20);
        let (land, offer) = play_mdfc_land(&mut declined, card_id);
        assert!(find_resolution_choice(&offer).is_some());
        declined
            .apply_command(
                0,
                &submit_resolution_decision(ResolutionChoiceDecision::Decline),
            )
            .expect("decline the entry payment");
        assert_eq!(declined.state.players[0].life, 20);
        assert_eq!(declined.state.objects[&land].zone, Zone::Battlefield);
        assert!(declined.state.objects[&land].tapped);

        let mut unable = land_engine(seed + 2, 2);
        let (land, offer) = play_mdfc_land(&mut unable, card_id);
        let choice =
            find_resolution_choice(&offer).expect("unaffordable payment still offers decline");
        assert!(!choice.resolution_branches[0].selectable);
        assert!(unable
            .apply_command(
                0,
                &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
            )
            .is_err());
        assert!(unable.state.pending_resolution.is_some());
        unable
            .apply_command(
                0,
                &submit_resolution_decision(ResolutionChoiceDecision::Decline),
            )
            .expect("decline when 3 life cannot be paid");
        assert_eq!(unable.state.players[0].life, 2);
        assert_eq!(unable.state.objects[&land].zone, Zone::Battlefield);
        assert!(unable.state.objects[&land].tapped);
    }
}

fn sundering_engine(seed: u64, target_card: &str) -> (GameEngine, u32) {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let target = inject_permanent_on_battlefield(&mut engine, 1, target_card);
    (engine, target)
}

fn cast_sundering(engine: &mut GameEngine, target: u32) -> u32 {
    let spell = inject_card_into_hand(engine, 0, SUNDERING);
    give_mana(
        engine,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, SUNDERING);
    engine
        .apply_command(
            0,
            &cast_spell_face(slot, vec![permanent_target(target, 0)], 0),
        )
        .expect("cast the Sundering Eruption face");
    spell
}

fn resolve_sundering_to_choice(
    engine: &mut GameEngine,
) -> tricerules_proto::ruled::v1::RuledEventBatch {
    engine.apply_command(0, &pass()).expect("caster passes");
    engine
        .apply_command(1, &pass())
        .expect("opponent passes and Sundering resolves to its optional search")
}

fn shuffle_logs(batch: &tricerules_proto::ruled::v1::RuledEventBatch, player: i32) -> usize {
    let expected = format!("P{player} shuffles their library.");
    batch
        .events
        .iter()
        .filter(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == expected))
        .count()
}

fn has_sundering_restriction(engine: &GameEngine) -> bool {
    engine.state.continuous_effects.iter().any(|effect| {
        effect.duration == EffectDuration::UntilEndOfTurn
            && matches!(&effect.kind, ContinuousEffectKind::CombatRestriction(_))
    })
}

#[test]
fn sundering_search_belongs_to_destroyed_land_controller_enters_tapped_and_checks_future_blockers()
{
    let (mut engine, target) = sundering_engine(20_260_927_321, "taiga");
    let existing_nonflyer = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let existing_flyer = inject_creature_on_battlefield(&mut engine, 1, "wind_drake");
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let basic = inject_library_card(&mut engine, 1, "forest");
    let nonbasic = inject_library_card(&mut engine, 1, "taiga");
    cast_sundering(&mut engine, target);

    let first = resolve_sundering_to_choice(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    let choice =
        find_resolution_choice(&first).expect("optional search for target land's controller");
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!(choice.deciding_player_id, 1);
    assert_eq!((choice.min, choice.max), (0, 1));
    let search = engine
        .apply_command(
            1,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("target controller elects to search");
    let choice = find_resolution_choice(&search).expect("private basic-land choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!(choice.deciding_player_id, 1);
    assert!(choice.candidate_object_ids.contains(&basic));
    assert!(!choice.candidate_object_ids.contains(&nonbasic));
    let completion = engine
        .apply_command(1, &submit_resolution_choice(vec![basic]))
        .expect("target controller finds a basic land");
    assert_eq!(shuffle_logs(&completion, 1), 1);
    assert_eq!(engine.state.objects[&basic].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&basic].owner,
        engine.state.players[1].id
    );
    assert_eq!(
        engine.state.objects[&basic].controller,
        engine.state.players[1].id
    );
    assert!(engine.state.objects[&basic].tapped);
    assert!(has_sundering_restriction(&engine));

    let future_nonflyer = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare an attacker");
    pass_to_declare_blockers(&mut engine);
    let legal_pairs = &engine.initial_response_batch().legal_by_player[&1].legal_block_pairs;
    assert!(
        legal_pairs
            .iter()
            .any(|pair| { pair.attacker_id == attacker && pair.blocker_id == existing_flyer }),
        "a creature that has flying when blockers are declared may block"
    );
    assert!(
        !legal_pairs.iter().any(|pair| {
            pair.attacker_id == attacker
                && (pair.blocker_id == existing_nonflyer || pair.blocker_id == future_nonflyer)
        }),
        "the dynamic restriction covers existing creatures and later entrants"
    );
    assert!(
        engine
            .apply_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: attacker,
                    blocker_id: future_nonflyer,
                }]),
            )
            .is_err(),
        "a later nonflying entrant cannot block"
    );

    engine
        .state
        .objects
        .get_mut(&future_nonflyer)
        .expect("future creature")
        .set_counter(CounterKind::Keyword(Keyword::Flying), 1);
    assert!(engine.effective_has_keyword(future_nonflyer, Keyword::Flying));
    let after_flying = &engine.initial_response_batch().legal_by_player[&1].legal_block_pairs;
    assert!(
        after_flying
            .iter()
            .any(|pair| { pair.attacker_id == attacker && pair.blocker_id == future_nonflyer }),
        "gaining flying after resolution removes the creature from the matching scope"
    );
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: future_nonflyer,
            }]),
        )
        .expect("the now-flying creature may block");
}

#[test]
fn sundering_decline_skips_search_and_shuffle_but_keeps_restriction_until_cleanup() {
    let (mut engine, target) = sundering_engine(20_260_927_322, "taiga");
    let basic = inject_library_card(&mut engine, 1, "forest");
    let library_before = engine.state.players[1].library.clone();
    cast_sundering(&mut engine, target);
    let first = resolve_sundering_to_choice(&mut engine);
    let choice = find_resolution_choice(&first).expect("target controller optional search");
    assert_eq!(choice.deciding_player_id, 1);
    let completion = engine
        .apply_command(
            1,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline the optional search");
    assert_eq!(engine.state.players[1].library, library_before);
    assert!(engine.state.players[1].library.contains(&basic));
    assert_eq!(shuffle_logs(&completion, 1), 0);
    assert!(find_resolution_choice(&completion).is_none());
    assert!(has_sundering_restriction(&engine));

    end_active_turn(&mut engine, 0);
    assert!(
        !engine
            .state
            .continuous_effects
            .iter()
            .any(|effect| { matches!(&effect.kind, ContinuousEffectKind::CombatRestriction(_)) }),
        "the restriction expires at this turn's cleanup"
    );
}

#[test]
fn sundering_indestructible_land_still_allows_search_and_applies_restriction() {
    let (mut engine, target) = sundering_engine(20_260_927_323, "darksteel_citadel");
    let basic = inject_library_card(&mut engine, 1, "mountain");
    cast_sundering(&mut engine, target);
    let first = resolve_sundering_to_choice(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    let choice = find_resolution_choice(&first).expect("indestructible target remains legal");
    assert_eq!(choice.deciding_player_id, 1);
    let search = engine
        .apply_command(
            1,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("target controller chooses to search");
    let choice = find_resolution_choice(&search).expect("basic-land choice");
    assert!(choice.candidate_object_ids.contains(&basic));
    engine
        .apply_command(1, &submit_resolution_choice(vec![basic]))
        .expect("find Mountain");
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&basic].zone, Zone::Battlefield);
    assert!(engine.state.objects[&basic].tapped);
    assert!(has_sundering_restriction(&engine));
}

#[test]
fn sundering_illegal_target_fizzles_search_and_block_restriction_together() {
    let (mut engine, target) = sundering_engine(20_260_927_324, "taiga");
    let basic = inject_library_card(&mut engine, 1, "forest");
    let library_before = engine.state.players[1].library.clone();
    let spell = cast_sundering(&mut engine, target);
    leave_battlefield_to_hand(&mut engine, 1, target);
    engine.apply_command(0, &pass()).expect("caster passes");
    let completion = engine
        .apply_command(1, &pass())
        .expect("the spell's sole target is illegal");
    assert!(find_resolution_choice(&completion).is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&target].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[1].library, library_before);
    assert!(engine.state.players[1].library.contains(&basic));
    assert!(!has_sundering_restriction(&engine));
}
