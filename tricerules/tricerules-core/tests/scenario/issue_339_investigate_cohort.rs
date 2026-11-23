//! Actual-card scenarios for the first #339 investigate cohort.
//!
//! Expectations are printed Oracle values. CR 701.16a: each investigate creates one Clue token
//! (CR 111.10f); "investigate twice" is two investigate actions. Pinned rulings: a spell whose
//! only target is illegal on resolution doesn't resolve and creates no Clue (CR 608.2b).
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_event::Ev;
use tricerules_proto::ruled::v1::{ChoiceKind, ResolutionChoiceDecision};

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
    grant_pool(&mut e, 0);
    e
}

fn cast(e: &mut GameEngine, card: &str, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, 0, card);
    grant_pool(e, 0);
    let slot = hand_index_for_card(e, 0, card);
    semantic::accepted(e, 0, &cast_spell(slot, targets));
}

fn clues(e: &GameEngine, player: usize) -> usize {
    battlefield_token_oids(e, player, "clue").len()
}

fn battlefield_card(e: &GameEngine, player: usize, card: &str) -> u32 {
    e.state.players[player]
        .battlefield
        .iter()
        .copied()
        .find(|oid| e.state.objects[oid].card_id == card)
        .unwrap_or_else(|| panic!("{card} on the battlefield"))
}

fn investigate_logs(batch: &RuledEventBatch) -> usize {
    batch
        .events
        .iter()
        .filter(|event| {
            matches!(&event.ev, Some(Ev::Log(log)) if log.text.starts_with("P0 investigates ("))
        })
        .count()
}

fn resolve_all(e: &mut GameEngine) -> RuledEventBatch {
    let mut all = RuledEventBatch::default();
    while !e.state.stack.is_empty() && e.state.pending_resolution.is_none() {
        all.events.extend(pass_both_players_with_batch(e).events);
    }
    all
}

fn with_generation(e: &GameEngine, mut command: RuledCommand) -> RuledCommand {
    if let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() {
        ability.expected_zone_change_generation = e
            .state
            .zone_change_generation
            .get(&ability.source_object_id)
            .copied()
            .unwrap_or(0);
    }
    command
}

fn select_branch(index: u32) -> RuledCommand {
    let mut command = submit_resolution_choice(vec![]);
    if let Some(Cmd::SubmitResolutionChoice(choice)) = command.cmd.as_mut() {
        choice.decision = ResolutionChoiceDecision::SelectBranch as i32;
        choice.selected_branch_index = index;
    }
    command
}

#[test]
fn the_chase_is_on_pumps_grants_first_strike_and_investigates() {
    let mut e = engine(339_101);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    cast(&mut e, "the_chase_is_on", target_object(bear));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(5), Some(2))
    );
    assert!(e.effective_has_keyword(bear, Keyword::FirstStrike));
    assert_eq!(clues(&e, 0), 1);
    assert_eq!(clues(&e, 1), 0);
}

#[test]
fn targeted_investigate_spells_create_no_clue_when_their_target_is_illegal() {
    for card in ["the_chase_is_on", "toxin_analysis"] {
        let mut e = engine(339_102);
        let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        cast(&mut e, card, target_object(bear));
        cast(&mut e, "unsummon", target_object(bear));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&bear].zone, Zone::Hand, "{card}");
        assert_eq!(clues(&e, 0), 0, "{card}: the spell did not resolve");
    }
}

#[test]
fn toxin_analysis_grants_deathtouch_and_lifelink_and_investigates() {
    let mut e = engine(339_103);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast(&mut e, "toxin_analysis", target_object(bear));
    resolve_entire_stack_two_player(&mut e);
    assert!(e.effective_has_keyword(bear, Keyword::Deathtouch));
    assert!(e.effective_has_keyword(bear, Keyword::Lifelink));
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(2), Some(2))
    );
    assert_eq!(
        clues(&e, 0),
        1,
        "the caster investigates, not the target's controller"
    );
    assert_eq!(clues(&e, 1), 0);
}

#[test]
fn they_went_this_way_fetches_a_tapped_basic_then_investigates() {
    let mut e = engine(339_104);
    let basic = inject_library_card(&mut e, 0, "forest");
    let nonbasic = inject_library_card(&mut e, 0, "azorius_guildgate");
    cast(&mut e, "they_went_this_way", vec![]);
    e.apply_scenario_command(0, &pass()).expect("caster pass");
    let batch = e
        .apply_scenario_command(1, &pass())
        .expect("resolve to search");
    let choice = find_resolution_choice(&batch).expect("library search choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert!(choice.candidate_object_ids.contains(&basic));
    assert!(!choice.candidate_object_ids.contains(&nonbasic));
    assert_eq!(clues(&e, 0), 0, "investigate follows the search");
    e.apply_scenario_command(0, &submit_resolution_choice(vec![basic]))
        .expect("choose the basic land");
    assert_eq!(e.state.objects[&basic].zone, Zone::Battlefield);
    assert!(e.state.objects[&basic].tapped);
    assert_eq!(clues(&e, 0), 1);
    assert!(e.state.stack.is_empty());
}

#[test]
fn they_went_this_way_still_investigates_after_finding_nothing() {
    let mut e = engine(339_105);
    cast(&mut e, "they_went_this_way", vec![]);
    e.apply_scenario_command(0, &pass()).expect("caster pass");
    e.apply_scenario_command(1, &pass())
        .expect("resolve to search");
    if e.state.pending_resolution.is_some() {
        e.apply_scenario_command(0, &submit_resolution_choice(vec![]))
            .expect("find nothing");
    }
    assert_eq!(clues(&e, 0), 1);
    assert!(e.state.stack.is_empty());
}

#[test]
fn magnifying_glass_taps_for_colorless_or_investigates_for_four() {
    let mut e = engine(339_106);
    let glass = inject_permanent_on_battlefield(&mut e, 0, "magnifying_glass");
    e.state.players[0].mana_pool = Default::default();
    e.apply_scenario_command(0, &activate_ability_for(&e, glass, 0, vec![]))
        .expect("tap for mana");
    assert_eq!(e.state.players[0].mana_pool.colorless, 1);
    assert!(
        e.state.stack.is_empty(),
        "mana abilities do not use the stack"
    );

    let mut e = engine(339_107);
    let glass = inject_permanent_on_battlefield(&mut e, 0, "magnifying_glass");
    e.state.players[0].mana_pool = Default::default();
    assert!(
        e.apply_scenario_command(0, &activate_ability_for(&e, glass, 1, vec![]))
            .is_err(),
        "investigating costs four mana"
    );
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    e.apply_scenario_command(0, &activate_ability_for(&e, glass, 1, vec![]))
        .expect("investigate");
    assert!(e.state.objects[&glass].tapped);
    let batch = pass_both_players_with_batch(&mut e);
    assert_eq!(investigate_logs(&batch), 1);
    assert_eq!(clues(&e, 0), 1);
}

#[test]
fn cold_case_cracker_investigates_for_its_controller_when_it_dies() {
    let mut e = engine(339_108);
    let cracker = inject_permanent_on_battlefield(&mut e, 0, "cold_case_cracker");
    assert!(e.effective_has_keyword(cracker, Keyword::Flying));
    assert_eq!(
        (e.effective_power(cracker), e.effective_toughness(cracker)),
        (Some(3), Some(3))
    );
    cast(&mut e, "lightning_bolt", target_object(cracker));
    resolve_all(&mut e);
    assert_eq!(e.state.objects[&cracker].zone, Zone::Graveyard);
    assert_eq!(clues(&e, 0), 1);
    assert_eq!(clues(&e, 1), 0);
}

#[test]
fn ezrim_investigates_twice_and_sacrifices_an_artifact_for_a_chosen_keyword() {
    let mut e = engine(339_109);
    cast(&mut e, "ezrim,_agency_chief", vec![]);
    let batch = resolve_all(&mut e);
    assert_eq!(clues(&e, 0), 2);
    assert_eq!(
        investigate_logs(&batch),
        2,
        "two separate investigate actions"
    );
    let ezrim = battlefield_card(&e, 0, "ezrim,_agency_chief");
    assert!(e.effective_has_keyword(ezrim, Keyword::Flying));
    assert_eq!(
        (e.effective_power(ezrim), e.effective_toughness(ezrim)),
        (Some(5), Some(5))
    );

    let clue = battlefield_token_oids(&e, 0, "clue")[0];
    e.state.players[0].mana_pool = Default::default();
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    e.apply_scenario_command(
        0,
        &with_generation(
            &e,
            activate_ability_with_costs(ezrim, 0, vec![], vec![permanent_cost_selection(1, clue)]),
        ),
    )
    .expect("sacrifice a Clue to Ezrim");
    assert!(
        !e.state.objects.contains_key(&clue) || e.state.objects[&clue].zone != Zone::Battlefield
    );
    assert_eq!(clues(&e, 0), 1);
    pass_both_players(&mut e);
    assert_eq!(
        e.state
            .pending_resolution
            .as_ref()
            .expect("keyword choice")
            .presentation
            .choice_kind,
        ChoiceKind::ResolutionBranch
    );
    e.apply_scenario_command(0, &select_branch(1))
        .expect("choose lifelink");
    assert!(e.effective_has_keyword(ezrim, Keyword::Lifelink));
    assert!(!e.effective_has_keyword(ezrim, Keyword::Vigilance));
    assert!(!e.effective_has_keyword(ezrim, Keyword::Hexproof));
}

#[test]
fn ezrim_requires_an_artifact_to_sacrifice() {
    let mut e = engine(339_110);
    let ezrim = inject_permanent_on_battlefield(&mut e, 0, "ezrim,_agency_chief");
    assert!(e
        .apply_scenario_command(0, &activate_ability(ezrim, 0, vec![]))
        .is_err());
}

#[test]
fn detectives_satchel_investigates_twice_and_makes_thopters_only_after_an_artifact_sacrifice() {
    let mut e = engine(339_111);
    cast(&mut e, "detectives_satchel", vec![]);
    let batch = resolve_all(&mut e);
    assert_eq!(clues(&e, 0), 2);
    assert_eq!(
        investigate_logs(&batch),
        2,
        "two separate investigate actions"
    );
    let satchel = battlefield_card(&e, 0, "detectives_satchel");

    assert!(
        e.apply_scenario_command(0, &activate_ability_for(&e, satchel, 0, vec![]))
            .is_err(),
        "no artifact was sacrificed this turn"
    );
    assert!(!e.state.objects[&satchel].tapped);

    // Crack a Clue: sacrificing it satisfies the restriction.
    let clue = battlefield_token_oids(&e, 0, "clue")[0];
    grant_pool(&mut e, 0);
    e.apply_scenario_command(0, &activate_ability_for(&e, clue, 0, vec![]))
        .expect("crack a Clue");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(clues(&e, 0), 1);

    e.apply_scenario_command(0, &activate_ability_for(&e, satchel, 0, vec![]))
        .expect("Thopter after an artifact sacrifice");
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.objects[&satchel].tapped);
    let thopters = battlefield_token_oids(&e, 0, "thopter_c_1_1_flying");
    assert_eq!(thopters.len(), 1);
    assert!(e.effective_has_keyword(thopters[0], Keyword::Flying));
}
