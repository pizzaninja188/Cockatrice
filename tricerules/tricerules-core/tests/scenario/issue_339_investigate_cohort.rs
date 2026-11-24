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

// Batch 2: Alquist Proft, Out Cold, Homicide Investigator, Hostile Investigator, Malcolm,
// Curious Inquiry, Agent 13, Meddling Youths, Chalk Outline and Drag the Canal.

fn stack_spell(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id,
        kind: tricerules_proto::ruled::v1::TargetRefKind::Stack as i32,
        ..Default::default()
    }]
}

fn choose_trigger_player(player: i32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(
            tricerules_proto::ruled::v1::ChooseTriggerTarget {
                decline: false,
                selected_modes: Vec::new(),
                targets: target_player(player),
            },
        )),
    }
}

fn enter_combat(e: &mut GameEngine) {
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_scenario_command(0, &pass())
        .expect("ap pass begin combat");
    e.apply_scenario_command(1, &pass())
        .expect("nap pass begin combat");
}

/// Resolve the stack, answering each pending resolution choice with its first `min` candidates.
fn resolve_answering(e: &mut GameEngine) {
    for _ in 0..64 {
        if let Some(pending) = e.state.pending_resolution.as_ref() {
            let chooser = pending.deciding_player;
            let chosen: Vec<u32> = pending
                .presentation
                .candidates
                .iter()
                .copied()
                .take(pending.presentation.min as usize)
                .collect();
            e.apply_scenario_command(chooser, &submit_resolution_choice(chosen))
                .expect("answer the pending choice");
            continue;
        }
        if e.state.stack.is_empty() {
            return;
        }
        pass_both_players(e);
    }
    panic!("resolution did not settle");
}

fn hand_size(e: &GameEngine, player: usize) -> usize {
    e.state.players[player].hand.len()
}

#[test]
fn alquist_proft_investigates_on_entry() {
    let mut e = engine(339_201);
    cast(&mut e, "alquist_proft,_master_sleuth", vec![]);
    let batch = resolve_all(&mut e);
    let alquist = battlefield_card(&e, 0, "alquist_proft,_master_sleuth");
    assert!(e.effective_has_keyword(alquist, Keyword::Vigilance));
    assert_eq!(
        (e.effective_power(alquist), e.effective_toughness(alquist)),
        (Some(3), Some(3))
    );
    assert_eq!(investigate_logs(&batch), 1);
    assert_eq!(clues(&e, 0), 1);
    assert_eq!(clues(&e, 1), 0);
}

#[test]
fn alquist_proft_sacrifices_a_clue_to_draw_and_gain_x() {
    let mut e = engine(339_202);
    let alquist = inject_creature_on_battlefield(&mut e, 0, "alquist_proft,_master_sleuth");
    assert!(
        e.apply_scenario_command(0, &activate_ability_for(&e, alquist, 0, vec![]))
            .is_err(),
        "the ability needs a Clue to sacrifice"
    );
    let clue = inject_permanent_on_battlefield(&mut e, 0, "clue");
    for _ in 0..2 {
        inject_library_card(&mut e, 0, "island");
    }
    let hand_before = hand_size(&e, 0);
    let life_before = e.state.players[0].life;
    e.state.players[0].mana_pool = Default::default();
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            u: 2,
            c: 2,
            ..Default::default()
        },
    );
    let mut command = with_generation(
        &e,
        activate_ability_with_costs(alquist, 0, vec![], vec![permanent_cost_selection(2, clue)]),
    );
    if let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() {
        ability.x_value = 2;
    }
    e.apply_scenario_command(0, &command)
        .expect("X=2 with a Clue sacrificed");
    assert!(e.state.objects[&alquist].tapped);
    assert_eq!(clues(&e, 0), 0, "the Clue was sacrificed as a cost");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(hand_size(&e, 0), hand_before + 2);
    assert_eq!(e.state.players[0].life, life_before + 2);
}

#[test]
fn out_cold_taps_and_stuns_up_to_two_creatures_and_investigates() {
    let mut e = engine(339_203);
    let a = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let b = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let mut targets = target_object(a);
    targets.extend(target_object(b));
    cast(&mut e, "out_cold", targets);
    resolve_entire_stack_two_player(&mut e);
    for creature in [a, b] {
        assert!(e.state.objects[&creature].tapped);
        assert_eq!(
            e.state.objects[&creature].counter_count(tricerules_cards::CounterKind::Stun),
            1
        );
    }
    assert_eq!(clues(&e, 0), 1);
    assert_eq!(clues(&e, 1), 0);
}

#[test]
fn out_cold_with_no_targets_still_investigates_and_cannot_be_countered() {
    let mut e = engine(339_204);
    cast(&mut e, "out_cold", vec![]);
    let out_cold = e.state.stack.last().expect("Out Cold on the stack").id;
    cast(&mut e, "counterspell", stack_spell(out_cold));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(count_card_id_in_graveyard(&e, 0, "out_cold"), 1);
    assert_eq!(clues(&e, 0), 1, "Out Cold resolved despite Counterspell");
}

#[test]
fn out_cold_with_all_targets_illegal_does_not_investigate() {
    let mut e = engine(339_205);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast(&mut e, "out_cold", target_object(bear));
    cast(&mut e, "unsummon", target_object(bear));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bear].zone, Zone::Hand);
    assert_eq!(clues(&e, 0), 0);
}

#[test]
fn homicide_investigator_investigates_once_for_simultaneous_deaths_including_its_own() {
    let mut e = engine(339_206);
    inject_creature_on_battlefield(&mut e, 0, "homicide_investigator");
    inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    cast(&mut e, "wrath_of_god", vec![]);
    resolve_all(&mut e);
    assert_eq!(
        count_card_id_in_graveyard(&e, 0, "homicide_investigator"),
        1
    );
    assert_eq!(clues(&e, 0), 1, "one trigger for the simultaneous deaths");
}

#[test]
fn homicide_investigator_triggers_once_each_turn_for_your_nontoken_creatures() {
    let mut e = engine(339_207);
    inject_creature_on_battlefield(&mut e, 0, "homicide_investigator");
    let token = inject_creature_on_battlefield(&mut e, 0, "soldier_w_1_1");
    let theirs = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast(&mut e, "lightning_bolt", target_object(token));
    resolve_all(&mut e);
    cast(&mut e, "lightning_bolt", target_object(theirs));
    resolve_all(&mut e);
    assert_eq!(
        clues(&e, 0),
        0,
        "a token or an opponent's creature does not count"
    );

    let first = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    cast(&mut e, "lightning_bolt", target_object(first));
    resolve_all(&mut e);
    assert_eq!(clues(&e, 0), 1);
    cast(&mut e, "lightning_bolt", target_object(second));
    resolve_all(&mut e);
    assert_eq!(clues(&e, 0), 1, "the ability triggers only once each turn");
}

#[test]
fn hostile_investigator_makes_an_opponent_discard_then_investigates_once_each_turn() {
    let mut e = engine(339_208);
    for _ in 0..3 {
        inject_card_into_hand(&mut e, 1, "grizzly_bears");
    }
    let opponent_hand = hand_size(&e, 1);
    cast(&mut e, "hostile_investigator", vec![]);
    pass_both_players(&mut e);
    let investigator = battlefield_card(&e, 0, "hostile_investigator");
    assert_eq!(
        (
            e.effective_power(investigator),
            e.effective_toughness(investigator)
        ),
        (Some(4), Some(3))
    );
    e.apply_scenario_command(0, &choose_trigger_player(1))
        .expect("target the opponent");
    resolve_answering(&mut e);
    assert_eq!(hand_size(&e, 1), opponent_hand - 1);
    assert_eq!(clues(&e, 0), 1, "the discard triggers investigate");

    cast(&mut e, "mind_rot", target_player(1));
    resolve_answering(&mut e);
    assert_eq!(hand_size(&e, 1), opponent_hand - 3);
    assert_eq!(clues(&e, 0), 1, "the ability triggers only once each turn");
}

#[test]
fn malcolm_investigates_on_the_second_spell_counting_itself() {
    let mut e = engine(339_209);
    cast(&mut e, "malcolm,_the_eyes", vec![]);
    resolve_all(&mut e);
    let malcolm = battlefield_card(&e, 0, "malcolm,_the_eyes");
    assert!(e.effective_has_keyword(malcolm, Keyword::Flying));
    assert!(e.effective_has_keyword(malcolm, Keyword::Haste));
    assert_eq!(clues(&e, 0), 0, "Malcolm was the first spell");
    cast(&mut e, "lightning_bolt", target_player(1));
    resolve_all(&mut e);
    assert_eq!(clues(&e, 0), 1, "the second spell this turn");
    cast(&mut e, "lightning_bolt", target_player(1));
    resolve_all(&mut e);
    assert_eq!(clues(&e, 0), 1, "the third spell does not trigger");
}

#[test]
fn curious_inquiry_pumps_and_grants_a_combat_damage_investigate() {
    let mut e = engine(339_210);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    cast(&mut e, "curious_inquiry", target_object(bear));
    resolve_all(&mut e);
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(3), Some(3))
    );
    enter_combat(&mut e);
    e.apply_scenario_command(0, &declare_attackers(vec![bear]))
        .expect("declare the enchanted bear");
    for _ in 0..2 {
        e.apply_scenario_command(0, &pass()).expect("ap pass");
        e.apply_scenario_command(1, &pass()).expect("nap pass");
    }
    resolve_all(&mut e);
    assert_eq!(e.state.players[1].life, 17);
    assert_eq!(clues(&e, 0), 1);
    assert_eq!(clues(&e, 1), 0);
}

#[test]
fn agent_13_investigates_only_when_a_creature_attacks_alone() {
    let mut e = engine(339_211);
    let agent = inject_creature_on_battlefield(&mut e, 0, "agent_13,_sharon_carter");
    inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    enter_combat(&mut e);
    e.apply_scenario_command(0, &declare_attackers(vec![agent]))
        .expect("attack alone");
    resolve_all(&mut e);
    assert_eq!(clues(&e, 0), 1);

    let mut e = engine(339_212);
    let agent = inject_creature_on_battlefield(&mut e, 0, "agent_13,_sharon_carter");
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    enter_combat(&mut e);
    e.apply_scenario_command(0, &declare_attackers(vec![agent, bear]))
        .expect("attack with two");
    assert!(e.state.stack.is_empty() && e.state.pending_triggers.is_empty());
}

#[test]
fn meddling_youths_investigates_when_three_or_more_attack() {
    let mut e = engine(339_213);
    let youths = inject_creature_on_battlefield(&mut e, 0, "meddling_youths");
    assert!(e.effective_has_keyword(youths, Keyword::Haste));
    let a = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let b = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    enter_combat(&mut e);
    e.apply_scenario_command(0, &declare_attackers(vec![youths, a, b]))
        .expect("attack with three");
    resolve_all(&mut e);
    assert_eq!(clues(&e, 0), 1);

    let mut e = engine(339_214);
    let youths = inject_creature_on_battlefield(&mut e, 0, "meddling_youths");
    let a = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    enter_combat(&mut e);
    e.apply_scenario_command(0, &declare_attackers(vec![youths, a]))
        .expect("attack with two");
    assert!(e.state.stack.is_empty() && e.state.pending_triggers.is_empty());
}

fn exile_opponent_graveyard_with_lantern(e: &mut GameEngine) {
    let lantern = inject_permanent_on_battlefield(e, 0, "soul-guide_lantern");
    e.apply_scenario_command(0, &activate_ability_for(e, lantern, 0, vec![]))
        .expect("exile each opponent's graveyard");
    resolve_all(e);
}

#[test]
fn chalk_outline_makes_one_detective_and_one_clue_for_simultaneous_creature_cards() {
    let mut e = engine(339_215);
    inject_permanent_on_battlefield(&mut e, 1, "chalk_outline");
    inject_graveyard_card(&mut e, 1, "grizzly_bears");
    inject_graveyard_card(&mut e, 1, "grizzly_bears");
    inject_graveyard_card(&mut e, 1, "lightning_bolt");
    exile_opponent_graveyard_with_lantern(&mut e);
    let detectives = battlefield_token_oids(&e, 1, "detective_wu_2_2");
    assert_eq!(detectives.len(), 1, "one trigger for one event");
    assert_eq!(
        (
            e.effective_power(detectives[0]),
            e.effective_toughness(detectives[0])
        ),
        (Some(2), Some(2))
    );
    assert_eq!(clues(&e, 1), 1);
    assert_eq!(clues(&e, 0), 0);
}

#[test]
fn chalk_outline_ignores_noncreature_cards_leaving_the_graveyard() {
    let mut e = engine(339_216);
    inject_permanent_on_battlefield(&mut e, 1, "chalk_outline");
    inject_graveyard_card(&mut e, 1, "lightning_bolt");
    exile_opponent_graveyard_with_lantern(&mut e);
    assert!(battlefield_token_oids(&e, 1, "detective_wu_2_2").is_empty());
    assert_eq!(clues(&e, 1), 0);
}

#[test]
fn drag_the_canal_without_a_death_only_makes_a_detective() {
    let mut e = engine(339_217);
    let life = e.state.players[0].life;
    cast(&mut e, "drag_the_canal", vec![]);
    e.apply_scenario_command(0, &pass()).expect("caster pass");
    let batch = e.apply_scenario_command(1, &pass()).expect("resolve");
    assert!(
        find_resolution_choice(&batch).is_none() && e.state.pending_resolution.is_none(),
        "no surveil choice without a death this turn"
    );
    assert!(e.state.stack.is_empty());
    assert_eq!(battlefield_token_oids(&e, 0, "detective_wu_2_2").len(), 1);
    assert_eq!(e.state.players[0].life, life);
    assert_eq!(clues(&e, 0), 0);
}

#[test]
fn drag_the_canal_after_a_death_gains_life_surveils_then_investigates() {
    let mut e = engine(339_218);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast(&mut e, "lightning_bolt", target_object(bear));
    resolve_all(&mut e);
    assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard);
    inject_library_card(&mut e, 0, "island");
    inject_library_card(&mut e, 0, "island");
    let life = e.state.players[0].life;
    cast(&mut e, "drag_the_canal", vec![]);
    e.apply_scenario_command(0, &pass()).expect("caster pass");
    let batch = e.apply_scenario_command(1, &pass()).expect("resolve");
    let choice = find_resolution_choice(&batch).expect("surveil choice");
    assert_eq!(battlefield_token_oids(&e, 0, "detective_wu_2_2").len(), 1);
    assert_eq!(e.state.players[0].life, life + 2);
    assert_eq!(clues(&e, 0), 0, "investigate follows surveil");
    assert_eq!(choice.candidate_object_ids.len(), 2, "surveil 2");
    resolve_answering(&mut e);
    assert_eq!(clues(&e, 0), 1);
}
