//! Issue #373 focused scenarios for the nine retained graveyard-count cards.
//!
//! These drive the generated definitions through the authoritative command path. CR 404.2 keeps
//! graveyard counts on printed public card data; CR 608.2h re-reads a quantity when the instruction
//! that consumes it applies (so Ooze Patrol's mill changes its own counter count); CR 107.3 and
//! CR 202.3 keep the affine damage formulas tied to the resolving stack item, so the resolving
//! spell's own card is on the stack rather than in a graveyard; and CR 603.4/602.5b distinguish
//! intervening-if re-checks from activation conditions.

use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{
    AbilitySourceZone, ActivateAbility, ChoiceKind, ChooseTriggerTarget, ResolutionChoiceDecision,
    RuledCommand, SubmitResolutionChoice, TargetRef, TargetRefKind,
};

fn choose_trigger_targets(targets: Vec<tricerules_proto::ruled::v1::TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets,
        })),
    }
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
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: 0,
            ..Default::default()
        })),
    }
}

fn grouped_targets(first: u32, second: u32) -> Vec<TargetRef> {
    vec![
        TargetRef {
            kind: TargetRefKind::Permanent as i32,
            object_id: first,
            group_index: 0,
            ..Default::default()
        },
        TargetRef {
            kind: TargetRefKind::Permanent as i32,
            object_id: second,
            group_index: 1,
            ..Default::default()
        },
    ]
}

fn graveyard_ability(
    engine: &GameEngine,
    source: u32,
    ability_index: u32,
    targets: Vec<TargetRef>,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: AbilitySourceZone::Graveyard as i32,
            expected_zone_change_generation: engine
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index,
            targets,
            ..Default::default()
        })),
    }
}

fn engine_with(seed: u64, own: &[&str]) -> GameEngine {
    let decks = Some(vec![deck_with("forest", own), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn damage_on(engine: &GameEngine, object_id: u32) -> u32 {
    engine.state.objects[&object_id].damage
}

fn advance_to_declare_attackers_from_main1(engine: &mut GameEngine) {
    engine
        .apply_command(0, &primitive_yield())
        .expect("move to beginning of combat");
    pass_both_players(engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
}

#[test]
fn issue_373_combustion_technique_counts_lessons_and_not_itself() {
    // No Lesson cards in any graveyard: the spell's own card is on the stack, so the affine
    // formula reads 2 + 0.
    let mut no_lessons = engine_with(373_001, &["combustion_technique"]);
    let target = inject_creature_with_stats(&mut no_lessons, 1, "grizzly_bears", 2, 5);
    ensure_in_hand(&mut no_lessons, 0, "combustion_technique");
    give_mana(
        &mut no_lessons,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&no_lessons, 0, "combustion_technique");
    no_lessons
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Combustion Technique");
    resolve_entire_stack_two_player(&mut no_lessons);
    assert_eq!(
        damage_on(&no_lessons, target),
        2,
        "the resolving spell must not count its own card"
    );

    // Two Lesson cards in the graveyard raise the same instruction to 4.
    let mut two_lessons = engine_with(373_002, &["combustion_technique"]);
    let target = inject_creature_with_stats(&mut two_lessons, 1, "grizzly_bears", 2, 5);
    inject_graveyard_card(&mut two_lessons, 0, "combustion_technique");
    inject_graveyard_card(&mut two_lessons, 0, "combustion_technique");
    ensure_in_hand(&mut two_lessons, 0, "combustion_technique");
    give_mana(
        &mut two_lessons,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&two_lessons, 0, "combustion_technique");
    two_lessons
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Combustion Technique");
    resolve_entire_stack_two_player(&mut two_lessons);
    assert_eq!(
        damage_on(&two_lessons, target),
        4,
        "2 plus the number of Lesson cards"
    );

    // The companion exile rider replaces the death of the exact damaged creature.
    let mut lethal = engine_with(373_003, &["combustion_technique"]);
    let target = inject_creature_in_play(&mut lethal, 1, "grizzly_bears");
    ensure_in_hand(&mut lethal, 0, "combustion_technique");
    give_mana(
        &mut lethal,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&lethal, 0, "combustion_technique");
    lethal
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Combustion Technique");
    resolve_entire_stack_two_player(&mut lethal);
    assert_eq!(
        lethal.state.objects[&target].zone,
        Zone::Exile,
        "the damaged creature is exiled instead of dying"
    );
    assert!(!lethal.state.players[1].graveyard.contains(&target));
}

fn inject_creature_in_play(engine: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    inject_creature_on_battlefield(engine, player, card_id)
}

#[test]
fn issue_373_frantic_firebolt_union_counts_instants_sorceries_and_adventures() {
    // Instant, sorcery, or Adventure qualify; a plain artifact creature does not.
    let mut matched = engine_with(373_010, &["frantic_firebolt"]);
    let target = inject_creature_with_stats(&mut matched, 1, "grizzly_bears", 1, 9);
    inject_graveyard_card(&mut matched, 0, "unsummon");
    inject_graveyard_card(&mut matched, 0, "bonecrusher_giant_stomp");
    inject_graveyard_card(&mut matched, 0, "ornithopter");
    ensure_in_hand(&mut matched, 0, "frantic_firebolt");
    give_mana(
        &mut matched,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&matched, 0, "frantic_firebolt");
    matched
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Frantic Firebolt");
    resolve_entire_stack_two_player(&mut matched);
    assert_eq!(
        damage_on(&matched, target),
        4,
        "2 plus the instant and Adventure cards; the artifact creature is ignored"
    );

    let mut unmatched = engine_with(373_011, &["frantic_firebolt"]);
    let target = inject_creature_with_stats(&mut unmatched, 1, "grizzly_bears", 1, 9);
    inject_graveyard_card(&mut unmatched, 0, "ornithopter");
    ensure_in_hand(&mut unmatched, 0, "frantic_firebolt");
    give_mana(
        &mut unmatched,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&unmatched, 0, "frantic_firebolt");
    unmatched
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Frantic Firebolt");
    resolve_entire_stack_two_player(&mut unmatched);
    assert_eq!(damage_on(&unmatched, target), 2, "only the base constant");
}

#[test]
fn issue_373_ooze_patrol_reevaluates_its_country_after_its_own_mill() {
    let mut engine = engine_with(373_020, &["ooze_patrol"]);
    // One artifact-or-creature card is already in the graveyard.
    inject_graveyard_card(&mut engine, 0, "ornithopter");
    // The top two library cards are both artifact/creature cards.
    let first = inject_library_card(&mut engine, 0, "ornithopter");
    let second = inject_library_card(&mut engine, 0, "grizzly_bears");
    let library = &mut engine.state.players[0].library;
    library.retain(|object_id| *object_id != first && *object_id != second);
    library.insert(0, second);
    library.insert(0, first);

    let patrol = move_ready_to_battlefield(&mut engine, 0, "ooze_patrol");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&patrol].counter_count(CounterKind::PlusOnePlusOne),
        3,
        "the milled cards are counted by the following instruction in the same resolution"
    );
    assert!(engine.state.players[0].graveyard.contains(&first));
    assert!(engine.state.players[0].graveyard.contains(&second));
}

#[test]
fn issue_373_cloud_of_darkness_particle_beam_scales_with_permanent_cards() {
    let mut engine = engine_with(373_030, &["cloud_of_darkness"]);
    let target = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 5, 5);
    inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    inject_graveyard_card(&mut engine, 0, "forest");
    inject_graveyard_card(&mut engine, 0, "ornithopter");
    // An instant card is not a permanent card.
    inject_graveyard_card(&mut engine, 0, "unsummon");

    move_ready_to_battlefield(&mut engine, 0, "cloud_of_darkness");
    engine
        .apply_command(0, &choose_trigger_targets(target_object(target)))
        .expect("choose Particle Beam target");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.effective_power(target), Some(2));
    assert_eq!(engine.effective_toughness(target), Some(2));
}

#[test]
fn issue_373_gran_pulse_ochu_counts_only_permanent_cards() {
    let mut engine = engine_with(373_040, &["gran_pulse_ochu"]);
    let ochu = move_ready_to_battlefield(&mut engine, 0, "gran_pulse_ochu");
    inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    inject_graveyard_card(&mut engine, 0, "forest");
    inject_graveyard_card(&mut engine, 0, "unsummon");
    inject_graveyard_card(&mut engine, 0, "bump_in_the_night");

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 8,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &activate_ability_for(&engine, ochu, 0, vec![]))
        .expect("activate Gran Pulse Ochu");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.effective_power(ochu), Some(3));
    assert_eq!(engine.effective_toughness(ochu), Some(3));
}

#[test]
fn issue_373_malamet_veteran_checks_the_intervening_permanent_count() {
    // Three permanent cards plus two instant cards stay below Descend 4.
    let mut below = engine_with(373_050, &["malamet_veteran"]);
    let veteran = move_ready_to_battlefield(&mut below, 0, "malamet_veteran");
    inject_graveyard_card(&mut below, 0, "grizzly_bears");
    inject_graveyard_card(&mut below, 0, "forest");
    inject_graveyard_card(&mut below, 0, "ornithopter");
    inject_graveyard_card(&mut below, 0, "unsummon");
    inject_graveyard_card(&mut below, 0, "bump_in_the_night");
    advance_to_declare_attackers_from_main1(&mut below);
    below
        .apply_command(0, &declare_attackers(vec![veteran]))
        .expect("declare Malamet Veteran");
    assert!(
        below.state.stack.is_empty(),
        "three permanent cards do not satisfy the intervening-if"
    );

    // The fourth permanent card turns the trigger on at the inclusive boundary.
    let mut at = engine_with(373_051, &["malamet_veteran"]);
    let veteran = move_ready_to_battlefield(&mut at, 0, "malamet_veteran");
    for _ in 0..4 {
        inject_graveyard_card(&mut at, 0, "forest");
    }
    inject_graveyard_card(&mut at, 0, "unsummon");
    advance_to_declare_attackers_from_main1(&mut at);
    at.apply_command(0, &declare_attackers(vec![veteran]))
        .expect("declare Malamet Veteran");
    at.apply_command(0, &choose_trigger_targets(target_object(veteran)))
        .expect("choose the counter target");
    resolve_entire_stack_two_player(&mut at);
    assert_eq!(
        at.state.objects[&veteran].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "four permanent cards satisfy Descend 4"
    );
}

#[test]
fn issue_373_thought_shucker_threshold_and_once_limit() {
    let mut engine = engine_with(373_060, &["thought_shucker"]);
    let shucker = move_ready_to_battlefield(&mut engine, 0, "thought_shucker");
    for _ in 0..6 {
        inject_graveyard_card(&mut engine, 0, "forest");
    }
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            c: 2,
            ..Default::default()
        },
    );
    assert!(
        engine
            .apply_command(0, &activate_ability_for(&engine, shucker, 0, vec![]))
            .is_err(),
        "six cards in the graveyard do not satisfy Threshold seven"
    );

    inject_graveyard_card(&mut engine, 0, "forest");
    let hand_before = engine.state.players[0].hand.len();
    engine
        .apply_command(0, &activate_ability_for(&engine, shucker, 0, vec![]))
        .expect("seven cards satisfy Threshold");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&shucker].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before + 1,
        "the activation draws a card"
    );
    assert!(
        engine
            .apply_command(0, &activate_ability_for(&engine, shucker, 0, vec![]))
            .is_err(),
        "the PerObject limit allows only one activation"
    );
}

#[test]
fn issue_373_gloom_ripper_sums_battlefield_elves_and_graveyard_elf_cards() {
    let mut engine = engine_with(373_070, &["gloom_ripper"]);
    let ripper = move_ready_to_battlefield(&mut engine, 0, "gloom_ripper");
    // A second Elf on the battlefield and one Elf card in the graveyard.
    inject_creature_on_battlefield(&mut engine, 0, "elvish_visionary");
    inject_graveyard_card(&mut engine, 0, "elvish_visionary");
    let own_target = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 2, 2);
    let opposing_target = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 2);

    let _ = ripper;
    engine
        .apply_command(
            0,
            &choose_trigger_targets(vec![
                tricerules_proto::ruled::v1::TargetRef {
                    kind: tricerules_proto::ruled::v1::TargetRefKind::Permanent as i32,
                    object_id: own_target,
                    group_index: 0,
                    ..Default::default()
                },
                tricerules_proto::ruled::v1::TargetRef {
                    kind: tricerules_proto::ruled::v1::TargetRefKind::Permanent as i32,
                    object_id: opposing_target,
                    group_index: 1,
                    ..Default::default()
                },
            ]),
        )
        .expect("choose both Gloom Ripper targets");
    resolve_entire_stack_two_player(&mut engine);

    // X = two battlefield Elves (Ripper plus Visionary) + one Elf card in the graveyard.
    assert_eq!(engine.effective_power(own_target), Some(5));
    assert_eq!(engine.effective_toughness(own_target), Some(2));
    assert_eq!(
        engine.state.objects[&opposing_target].zone,
        Zone::Graveyard,
        "the -0/-3 pump kills the opposing 2/2"
    );
}

#[test]
fn issue_373_join_the_dead_replaces_the_base_pump_at_descend_four() {
    // Three permanent cards: the printed base -5/-5 applies.
    let mut below = engine_with(373_100, &["join_the_dead"]);
    let target = inject_creature_with_stats(&mut below, 1, "grizzly_bears", 2, 12);
    for _ in 0..3 {
        inject_graveyard_card(&mut below, 0, "forest");
    }
    inject_graveyard_card(&mut below, 0, "unsummon");
    ensure_in_hand(&mut below, 0, "join_the_dead");
    give_mana(
        &mut below,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&below, 0, "join_the_dead");
    below
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Join the Dead");
    resolve_entire_stack_two_player(&mut below);
    assert_eq!(
        below.effective_toughness(target),
        Some(7),
        "three permanent cards apply the -5/-5 branch"
    );

    // The fourth permanent card switches to -10/-10, which replaces the base value instead of
    // stacking with it.
    let mut at = engine_with(373_101, &["join_the_dead"]);
    let target = inject_creature_with_stats(&mut at, 1, "grizzly_bears", 2, 12);
    for _ in 0..4 {
        inject_graveyard_card(&mut at, 0, "forest");
    }
    inject_graveyard_card(&mut at, 0, "unsummon");
    ensure_in_hand(&mut at, 0, "join_the_dead");
    give_mana(
        &mut at,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&at, 0, "join_the_dead");
    at.apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Join the Dead");
    resolve_entire_stack_two_player(&mut at);
    assert_eq!(
        at.effective_toughness(target),
        Some(2),
        "four permanent cards replace -5/-5 with -10/-10, never both"
    );
}

#[test]
fn issue_373_lasyd_prowler_mills_per_controlled_land() {
    let mut engine = engine_with(373_110, &["lasyd_prowler"]);
    for _ in 0..2 {
        move_ready_to_battlefield(&mut engine, 0, "forest");
    }
    let graveyard_before = engine.state.players[0].graveyard.len();
    move_ready_to_battlefield(&mut engine, 0, "lasyd_prowler");
    let prompt = resolve_top_stack(&mut engine);
    assert_eq!(
        find_resolution_choice(&prompt)
            .expect("optional ETB mill choice")
            .choice_kind(),
        ChoiceKind::ResolutionBranch
    );
    engine
        .apply_command(0, &select_optional_effect())
        .expect("accept the optional ETB mill");
    assert_eq!(
        engine.state.players[0].graveyard.len(),
        graveyard_before + 2,
        "two controlled lands mill exactly two cards"
    );
}

#[test]
fn issue_373_lasyd_prowler_renew_counts_graveyard_land_cards() {
    let mut engine = engine_with(373_111, &["lasyd_prowler"]);
    let prowler = inject_graveyard_card(&mut engine, 0, "lasyd_prowler");
    let target = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 2);
    inject_graveyard_card(&mut engine, 0, "forest");
    inject_graveyard_card(&mut engine, 0, "forest");
    inject_graveyard_card(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(
            0,
            &graveyard_ability(&engine, prowler, 0, target_object(target)),
        )
        .expect("activate Renew");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::PlusOnePlusOne),
        2,
        "two land cards in the graveyard, not the instant"
    );
    assert_eq!(engine.state.objects[&prowler].zone, Zone::Exile);
}

#[test]
fn issue_373_violent_urge_delirium_adds_double_strike_at_four_types() {
    // Three card types stay below Delirium: only the pump and first strike apply.
    let mut below = engine_with(373_120, &["violent_urge"]);
    let creature = inject_creature_with_stats(&mut below, 0, "grizzly_bears", 2, 2);
    for card in ["forest", "unsummon", "bump_in_the_night"] {
        inject_graveyard_card(&mut below, 0, card);
    }
    ensure_in_hand(&mut below, 0, "violent_urge");
    give_mana(
        &mut below,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&below, 0, "violent_urge");
    below
        .apply_command(0, &cast_spell(slot, target_object(creature)))
        .expect("cast Violent Urge");
    resolve_entire_stack_two_player(&mut below);
    assert_eq!(below.effective_power(creature), Some(3));
    assert!(below.effective_has_keyword(creature, Keyword::FirstStrike));
    assert!(!below.effective_has_keyword(creature, Keyword::DoubleStrike));

    // The fourth card type satisfies the inclusive Delirium boundary.
    let mut at = engine_with(373_121, &["violent_urge"]);
    let creature = inject_creature_with_stats(&mut at, 0, "grizzly_bears", 2, 2);
    for card in ["forest", "unsummon", "bump_in_the_night", "ornithopter"] {
        inject_graveyard_card(&mut at, 0, card);
    }
    ensure_in_hand(&mut at, 0, "violent_urge");
    give_mana(
        &mut at,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&at, 0, "violent_urge");
    at.apply_command(0, &cast_spell(slot, target_object(creature)))
        .expect("cast Violent Urge");
    resolve_entire_stack_two_player(&mut at);
    assert!(at.effective_has_keyword(creature, Keyword::FirstStrike));
    assert!(at.effective_has_keyword(creature, Keyword::DoubleStrike));
}

#[test]
fn issue_373_beastie_beatdown_counters_precede_the_power_damage() {
    // Three card types: no counters, so the unpumped 4/4 deals 4 to the 2/5.
    let mut below = engine_with(373_130, &["beastie_beatdown"]);
    let own = inject_creature_with_stats(&mut below, 0, "grizzly_bears", 4, 4);
    let opposing = inject_creature_with_stats(&mut below, 1, "grizzly_bears", 2, 5);
    for card in ["forest", "unsummon", "bump_in_the_night"] {
        inject_graveyard_card(&mut below, 0, card);
    }
    ensure_in_hand(&mut below, 0, "beastie_beatdown");
    give_mana(
        &mut below,
        0,
        ManaGift {
            r: 1,
            g: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&below, 0, "beastie_beatdown");
    below
        .apply_command(0, &cast_spell(slot, grouped_targets(own, opposing)))
        .expect("cast Beastie Beatdown");
    resolve_entire_stack_two_player(&mut below);
    assert_eq!(
        below.state.objects[&own].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    assert_eq!(damage_on(&below, opposing), 4);
    assert_eq!(below.state.objects[&opposing].zone, Zone::Battlefield);

    // Delirium places two counters before the same creature deals its now-6 power.
    let mut at = engine_with(373_131, &["beastie_beatdown"]);
    let own = inject_creature_with_stats(&mut at, 0, "grizzly_bears", 4, 4);
    let opposing = inject_creature_with_stats(&mut at, 1, "grizzly_bears", 2, 5);
    for card in ["forest", "unsummon", "bump_in_the_night", "ornithopter"] {
        inject_graveyard_card(&mut at, 0, card);
    }
    ensure_in_hand(&mut at, 0, "beastie_beatdown");
    give_mana(
        &mut at,
        0,
        ManaGift {
            r: 1,
            g: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&at, 0, "beastie_beatdown");
    at.apply_command(0, &cast_spell(slot, grouped_targets(own, opposing)))
        .expect("cast Beastie Beatdown");
    resolve_entire_stack_two_player(&mut at);
    assert_eq!(
        at.state.objects[&own].counter_count(CounterKind::PlusOnePlusOne),
        2,
        "the controlled creature gets the counters"
    );
    assert_eq!(
        at.state.objects[&opposing].zone,
        Zone::Graveyard,
        "6 power after the counters kills the 2/5"
    );
}
