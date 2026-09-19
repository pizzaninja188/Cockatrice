//! Issue #371 focused scenarios for the eight retained graveyard-count token cards.
//!
//! These drive the generated definitions through the authoritative command path. CR 404.2 keeps
//! the counts on public printed graveyard card data; CR 608.2h re-reads a quantity as the
//! instruction that consumes it resolves; CR 602.5b checks an activation condition before
//! payment; CR 603.4 re-checks an intervening-if on resolution; CR 601.2 records the cast origin
//! consumed by the resolving snapshot; and CR 111.1 keeps token entry status (tapped) part of the
//! created cohort.

use super::helpers::*;
use tricerules_core::{TurnStep, Zone};

fn engine_with(seed: u64, own: &[&str]) -> GameEngine {
    let decks = Some(vec![deck_with("forest", own), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn resolve_top_stack(engine: &mut GameEngine) -> RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    engine.apply_command(first, &pass()).expect("first pass");
    engine
        .apply_command(second, &pass())
        .expect("second pass resolves stack item")
}

fn select_optional_effect() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: tricerules_proto::ruled::v1::ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: 0,
            ..Default::default()
        })),
    }
}

fn decline_optional_effect() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: tricerules_proto::ruled::v1::ResolutionChoiceDecision::Decline as i32,
            ..Default::default()
        })),
    }
}

/// Activate a battlefield ability with explicit cost selections and a generation binding.
fn activate_with_costs(
    engine: &GameEngine,
    permanent: u32,
    ability_index: u32,
    selections: Vec<CostSelection>,
) -> RuledCommand {
    activate_with_costs_and_targets(engine, permanent, ability_index, selections, Vec::new())
}

fn activate_with_costs_and_targets(
    engine: &GameEngine,
    permanent: u32,
    ability_index: u32,
    cost_selections: Vec<CostSelection>,
    targets: Vec<TargetRef>,
) -> RuledCommand {
    let mut command =
        activate_ability_with_costs(permanent, ability_index, targets, cost_selections);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!()
    };
    ability.source_zone = tricerules_proto::ruled::v1::AbilitySourceZone::Battlefield as i32;
    ability.expected_zone_change_generation = engine
        .state
        .zone_change_generation
        .get(&permanent)
        .copied()
        .unwrap_or(0);
    command
}

fn flashback_cast(engine: &GameEngine, object_id: u32) -> RuledCommand {
    let generation = engine
        .state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0);
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            source: Some(graveyard_cast_source(object_id, generation)),
            cast_method: CastMethod::Flashback as i32,
            ..Default::default()
        })),
    }
}

fn move_to_declare_attackers(engine: &mut GameEngine) {
    engine
        .apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    inject_creature_on_battlefield(engine, 0, "grizzly_bears");
    pass_both_players(engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
}

fn seat_on_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let oids: Vec<u32> = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect();
    let library = &mut engine.state.players[player].library;
    library.retain(|object_id| !oids.contains(object_id));
    for oid in oids.iter().rev() {
        library.insert(0, *oid);
    }
    oids
}

#[test]
fn issue_371_aatchik_counts_only_its_controllers_artifact_and_creature_cards() {
    let mut engine = engine_with(371_001, &["aatchik,_emerald_radian"]);
    inject_graveyard_card(&mut engine, 0, "ornithopter");
    inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    inject_graveyard_card(&mut engine, 0, "forest");
    inject_graveyard_card(&mut engine, 1, "ornithopter");
    inject_graveyard_card(&mut engine, 1, "grizzly_bears");

    move_ready_to_battlefield(&mut engine, 0, "aatchik,_emerald_radian");
    resolve_entire_stack_two_player(&mut engine);

    let insects = battlefield_token_oids(&engine, 0, "insect_g_1_1");
    assert_eq!(
        insects.len(),
        2,
        "one artifact and one creature card; the land and both opposing cards are ignored"
    );
    for oid in &insects {
        assert!(!engine.state.objects[oid].tapped, "the token is not tapped");
    }

    // Another Insect dying adds a counter and drains each opponent.
    let victim = insects[0];
    inject_card_into_hand(&mut engine, 0, "lightning_bolt");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_command(0, &cast_spell(bolt, target_object(victim)))
        .expect("bolt the Insect token");
    resolve_entire_stack_two_player(&mut engine);
    let aatchik = battlefield_object_for_card(&engine, 0, "aatchik,_emerald_radian");
    assert_eq!(
        engine.state.objects[&aatchik].counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        1,
        "the dying Insect added a +1/+1 counter"
    );
    assert_eq!(
        engine.state.players[1].life, 19,
        "each opponent loses 1 life"
    );
}

#[test]
fn issue_371_arnim_zola_gate_and_land_discard_fail_closed() {
    let mut engine = engine_with(371_010, &["arnim_zola,_bio-fanatic"]);
    let arnim = move_ready_to_battlefield(&mut engine, 0, "arnim_zola,_bio-fanatic");
    grant_pool(&mut engine, 0);

    // One creature card is below the printed two-or-more gate.
    inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &activate_with_costs(&engine, arnim, 0, Vec::new()))
        .expect_err("activation requires two or more creature cards in the graveyard");
    assert_eq!(engine.state.objects[&arnim].zone, Zone::Battlefield);

    // The second creature card enables the activation; the tapped Villain token enters on
    // resolution.
    inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &activate_with_costs(&engine, arnim, 0, Vec::new()))
        .expect("activate with two creature cards");
    resolve_top_stack(&mut engine);
    let villains = battlefield_token_oids(&engine, 0, "villain_b_2_1_menace");
    assert_eq!(villains.len(), 1);
    assert!(
        engine.state.objects[&villains[0]].tapped,
        "the Villain enters tapped"
    );
    assert!(
        engine.effective_has_keyword(villains[0], tricerules_cards::Keyword::Menace),
        "the printed Villain has menace"
    );
    assert_eq!(engine.state.players[0].graveyard.len(), 2);
}

#[test]
fn issue_371_hydra_troopers_branches_villain_or_mill_two() {
    // Two creature cards: the Villain branch applies and nothing is milled.
    let mut villain = engine_with(371_020, &["hydra_troopers"]);
    inject_graveyard_card(&mut villain, 0, "grizzly_bears");
    inject_graveyard_card(&mut villain, 0, "grizzly_bears");
    move_ready_to_battlefield(&mut villain, 0, "hydra_troopers");
    let library_before = villain.state.players[0].library.len();
    resolve_entire_stack_two_player(&mut villain);
    assert_eq!(
        battlefield_token_oids(&villain, 0, "villain_b_2_1_menace").len(),
        1
    );
    assert_eq!(
        villain.state.players[0].library.len(),
        library_before,
        "the otherwise mill does not also run"
    );

    // One creature card: the otherwise branch mills exactly two.
    let mut mill = engine_with(371_021, &["hydra_troopers"]);
    inject_graveyard_card(&mut mill, 0, "grizzly_bears");
    move_ready_to_battlefield(&mut mill, 0, "hydra_troopers");
    let library_before = mill.state.players[0].library.len();
    resolve_entire_stack_two_player(&mut mill);
    assert!(battlefield_token_oids(&mill, 0, "villain_b_2_1_menace").is_empty());
    assert_eq!(
        mill.state.players[0].library.len(),
        library_before - 2,
        "the otherwise branch mills two cards"
    );
    assert_eq!(
        mill.state.players[0].graveyard.len(),
        3,
        "one injected creature card plus two milled cards"
    );
}

#[test]
fn issue_371_kiora_etb_loots_two_and_the_threshold_attack_is_optional() {
    let mut etb = engine_with(371_030, &["kiora,_the_rising_tide"]);
    move_ready_to_battlefield(&mut etb, 0, "kiora,_the_rising_tide");
    let hand_before = etb.state.players[0].hand.len();
    let library_before = etb.state.players[0].library.len();
    let resolution = resolve_top_stack(&mut etb);
    let choice = find_resolution_choice(&resolution).expect("discard-two choice");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.candidate_object_ids.len(), hand_before + 2);
    let discards = choice
        .candidate_object_ids
        .iter()
        .take(2)
        .copied()
        .collect::<Vec<_>>();
    etb.apply_command(0, &submit_resolution_choice(discards.clone()))
        .expect("discard two cards");
    assert_eq!(
        etb.state.players[0].hand.len(),
        hand_before,
        "CR 121.1/701.9: draw two then discard two"
    );
    assert_eq!(etb.state.players[0].library.len(), library_before - 2);
    for oid in discards {
        assert!(etb.state.players[0].graveyard.contains(&oid));
    }

    // Below threshold the attack trigger never goes on the stack.
    let mut below = engine_with(371_031, &[]);
    for _ in 0..6 {
        inject_graveyard_card(&mut below, 0, "forest");
    }
    let kiora = inject_creature_on_battlefield(&mut below, 0, "kiora,_the_rising_tide");
    move_to_declare_attackers(&mut below);
    below
        .apply_command(0, &declare_attackers(vec![kiora]))
        .expect("attack with Kiora below threshold");
    assert!(below.state.stack.is_empty(), "no trigger below seven cards");
    assert!(battlefield_token_oids(&below, 0, "scion_of_the_deep").is_empty());

    // At seven or more the optional trigger resolves through the shipped choice surface.
    let mut at = engine_with(371_032, &[]);
    for _ in 0..7 {
        inject_graveyard_card(&mut at, 0, "forest");
    }
    let kiora = inject_creature_on_battlefield(&mut at, 0, "kiora,_the_rising_tide");
    move_to_declare_attackers(&mut at);
    at.apply_command(0, &declare_attackers(vec![kiora]))
        .expect("attack with Kiora at threshold");
    let prompt = resolve_top_stack(&mut at);
    find_resolution_choice(&prompt).expect("optional Scion creation choice");
    at.apply_command(0, &select_optional_effect())
        .expect("create the Scion");
    let scions = battlefield_token_oids(&at, 0, "scion_of_the_deep");
    assert_eq!(scions.len(), 1);
    assert!(
        !at.state.objects[&scions[0]].tapped,
        "the Scion enters untapped"
    );

    // Declining the optional creation leaves no token.
    let mut decline = engine_with(371_033, &[]);
    for _ in 0..7 {
        inject_graveyard_card(&mut decline, 0, "forest");
    }
    let kiora = inject_creature_on_battlefield(&mut decline, 0, "kiora,_the_rising_tide");
    move_to_declare_attackers(&mut decline);
    decline
        .apply_command(0, &declare_attackers(vec![kiora]))
        .expect("attack with Kiora at threshold");
    let prompt = resolve_top_stack(&mut decline);
    find_resolution_choice(&prompt).expect("optional Scion creation choice");
    decline
        .apply_command(0, &decline_optional_effect())
        .expect("decline the Scion");
    assert!(battlefield_token_oids(&decline, 0, "scion_of_the_deep").is_empty());
}

#[test]
fn issue_371_lluwen_activation_counts_lands_and_requires_a_land_discard() {
    let mut engine = engine_with(371_040, &["lluwen,_imperfect_naturalist"]);
    let lluwen = move_ready_to_battlefield(&mut engine, 0, "lluwen,_imperfect_naturalist");
    inject_graveyard_card(&mut engine, 0, "forest");
    inject_graveyard_card(&mut engine, 0, "forest");
    inject_graveyard_card(&mut engine, 0, "unsummon");
    inject_card_into_hand(&mut engine, 0, "forest");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 3,
            c: 2,
            ..Default::default()
        },
    );
    let land_slot = hand_index_for_card(&engine, 0, "forest") as u32;
    engine
        .apply_command(
            0,
            &activate_with_costs(&engine, lluwen, 0, vec![hand_cost_selection(2, land_slot)]),
        )
        .expect("activate Lluwen with a land discard");
    resolve_top_stack(&mut engine);
    assert_eq!(
        battlefield_token_oids(&engine, 0, "worm_bg_1_1").len(),
        3,
        "CR 608.2h: the discarded land is already in the graveyard when the count is read"
    );

    // The filtered discard cost rejects a nonland card.
    engine
        .state
        .objects
        .get_mut(&lluwen)
        .expect("Lluwen")
        .tapped = false;
    inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    grant_pool(&mut engine, 0);
    let bear_slot = hand_index_for_card(&engine, 0, "grizzly_bears") as u32;
    engine
        .apply_command(
            0,
            &activate_with_costs(&engine, lluwen, 0, vec![hand_cost_selection(2, bear_slot)]),
        )
        .expect_err("the discard cost requires a land card");
}

#[test]
fn issue_371_lluwen_etb_tops_only_a_milled_creature_or_land() {
    let mut engine = engine_with(371_050, &["lluwen,_imperfect_naturalist"]);
    let milled = seat_on_top(
        &mut engine,
        0,
        &["grizzly_bears", "forest", "unsummon", "lightning_bolt"],
    );
    move_ready_to_battlefield(&mut engine, 0, "lluwen,_imperfect_naturalist");
    let resolution = resolve_top_stack(&mut engine);
    let choice = find_resolution_choice(&resolution).expect("milled-card choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::GraveyardCards);
    assert_eq!((choice.min, choice.max), (0, 1));
    let mut candidates = choice.candidate_object_ids.clone();
    candidates.sort_unstable();
    let mut expected = vec![milled[0], milled[1]];
    expected.sort_unstable();
    assert_eq!(
        candidates, expected,
        "only the milled creature and land are legal choices"
    );

    let chosen = milled[1];
    engine
        .apply_command(0, &submit_resolution_choice(vec![chosen]))
        .expect("put the milled land on top");
    assert_eq!(
        engine.state.players[0].library.front().copied(),
        Some(chosen),
        "the chosen card is on top of the library"
    );
    for oid in [milled[0], milled[2], milled[3]] {
        assert!(engine.state.players[0].graveyard.contains(&oid));
    }
}

#[test]
fn issue_371_morcant_is_sorcery_speed_and_sacrifices_itself() {
    let mut engine = engine_with(371_060, &["morcants_eyes"]);
    let morcant = move_ready_to_battlefield(&mut engine, 0, "morcants_eyes");
    inject_graveyard_card(&mut engine, 0, "llanowar_elves");
    inject_graveyard_card(&mut engine, 0, "lluwen,_imperfect_naturalist");
    inject_graveyard_card(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 6,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &activate_with_costs(&engine, morcant, 0, Vec::new()))
        .expect("activate at sorcery speed");
    assert_eq!(
        engine.state.objects[&morcant].zone,
        Zone::Graveyard,
        "the enchantment is sacrificed as the activation cost"
    );
    resolve_top_stack(&mut engine);
    let elves = battlefield_token_oids(&engine, 0, "elf_bg_2_2");
    assert_eq!(
        elves.len(),
        3,
        "CR 608.2h: the sacrificed Elf enchantment itself is in the graveyard when X is read, \
         so two injected Elf cards plus the source produce three tokens"
    );

    // The same activation is illegal once the main phase has ended.
    let mut instant = engine_with(371_061, &["morcants_eyes"]);
    let morcant = move_ready_to_battlefield(&mut instant, 0, "morcants_eyes");
    inject_graveyard_card(&mut instant, 0, "llanowar_elves");
    inject_graveyard_card(&mut instant, 0, "lluwen,_imperfect_naturalist");
    give_mana(
        &mut instant,
        0,
        ManaGift {
            g: 6,
            ..Default::default()
        },
    );
    move_to_declare_attackers(&mut instant);
    instant
        .apply_command(0, &activate_with_costs(&instant, morcant, 0, Vec::new()))
        .expect_err("activate only as a sorcery");
}

#[test]
fn issue_371_revenge_of_the_rats_creates_tapped_rats_from_its_own_graveyard() {
    let mut engine = engine_with(371_070, &["revenge_of_the_rats"]);
    inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    inject_graveyard_card(&mut engine, 0, "unsummon");
    inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "revenge_of_the_rats");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 4,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "revenge_of_the_rats");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Revenge of the Rats");
    resolve_entire_stack_two_player(&mut engine);

    let rats = battlefield_token_oids(&engine, 0, "rat_b_1_1");
    assert_eq!(
        rats.len(),
        2,
        "two of its controller's creature cards; the instant and opposing cards do not count"
    );
    assert!(
        battlefield_token_oids(&engine, 0, "rat_b_1_1_cant_block").is_empty(),
        "the printed token has no combat restriction"
    );
    for oid in &rats {
        assert!(engine.state.objects[oid].tapped, "the Rat enters tapped");
    }
}

#[test]
fn issue_371_the_final_days_uses_two_from_hand_and_x_from_graveyard() {
    // Cast from hand: the unconditional fallback makes exactly two tapped Horrors.
    let mut hand = engine_with(371_080, &["the_final_days"]);
    for _ in 0..3 {
        inject_graveyard_card(&mut hand, 0, "grizzly_bears");
    }
    ensure_in_hand(&mut hand, 0, "the_final_days");
    give_mana(
        &mut hand,
        0,
        ManaGift {
            b: 4,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&hand, 0, "the_final_days");
    hand.apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast The Final Days from hand");
    resolve_entire_stack_two_player(&mut hand);
    let horrors = battlefield_token_oids(&hand, 0, "horror_b_2_2");
    assert_eq!(horrors.len(), 2, "the from-hand branch creates exactly two");
    for oid in &horrors {
        assert!(hand.state.objects[oid].tapped, "the Horror enters tapped");
    }

    // Cast with flashback: the cast-origin snapshot switches the branch to X.
    let mut flashback = engine_with(371_081, &[]);
    let spell = inject_graveyard_card(&mut flashback, 0, "the_final_days");
    inject_graveyard_card(&mut flashback, 0, "grizzly_bears");
    inject_graveyard_card(&mut flashback, 0, "ornithopter");
    inject_graveyard_card(&mut flashback, 0, "grizzly_bears");
    grant_pool(&mut flashback, 0);
    flashback
        .apply_command(0, &flashback_cast(&flashback, spell))
        .expect("cast The Final Days from the graveyard");
    resolve_entire_stack_two_player(&mut flashback);
    let horrors = battlefield_token_oids(&flashback, 0, "horror_b_2_2");
    assert_eq!(
        horrors.len(),
        3,
        "X is the number of creature cards in the graveyard"
    );
    assert!(
        flashback.state.players[0].exile.contains(&spell),
        "CR 702.34: a flashback cast is exiled on resolution"
    );
}
