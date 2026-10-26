use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::CounterKind;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice,
};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        std::iter::repeat_n("forest".to_string(), 20).collect(),
        std::iter::repeat_n("mountain".to_string(), 20).collect(),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            chosen_object_ids: Vec::new(),
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            cast_spell: None,
            spell_cast_announcement: None,
            chosen_combat_defender: None,
            payment: None,
            restricted_mana: vec![],
            chosen_player_ids: vec![],
        })),
    }
}

fn cast_endure_creature(engine: &mut GameEngine, card_id: &str) -> u32 {
    let object_id = inject_card_into_hand(engine, 0, card_id);
    grant_pool(engine, 0);
    let index = hand_index_for_card(engine, 0, card_id);
    engine
        .apply_scenario_command(0, &cast_spell(index, Vec::new()))
        .expect("cast Endure creature");
    pass_both_players(engine);
    object_id
}

fn cast_noncreature_endure(engine: &mut GameEngine, card_id: &str) -> u32 {
    let object_id = inject_card_into_hand(engine, 0, card_id);
    grant_pool(engine, 0);
    engine
        .apply_scenario_command(
            0,
            &cast_spell(hand_index_for_card(engine, 0, card_id), Vec::new()),
        )
        .expect("cast the synthetic zero-cost artifact");
    pass_both_players(engine);
    object_id
}

fn transfer_control(engine: &mut GameEngine, object_id: u32, to_player: usize) {
    for player in &mut engine.state.players {
        player
            .battlefield
            .retain(|candidate| *candidate != object_id);
    }
    let player_id = engine.state.players[to_player].id;
    engine.state.players[to_player].battlefield.push(object_id);
    let object = engine.state.objects.get_mut(&object_id).expect("source");
    object.base_controller = player_id;
    object.controller = player_id;
}

fn noncreature_endure_engine(seed: u64, card_id: &str, prohibit_counters: bool) -> GameEngine {
    noncreature_endure_engine_with_counter(seed, card_id, prohibit_counters, "PlusOnePlusOne")
}

fn noncreature_endure_engine_with_counter(
    seed: u64,
    card_id: &str,
    prohibit_counters: bool,
    counter: &str,
) -> GameEngine {
    let static_ability = if prohibit_counters {
        r#"static_abilities: [(ability_id: "static_01", presentation: Fallback,
            definition: ProhibitCounters(affected: Self_))],"#
    } else {
        ""
    };
    let card = format!(
        r#"(id: "{card_id}", name: "{card_id}", face_id: "{card_id}",
            mana_cost: "{{0}}", types: ["Artifact"], {static_ability}
            triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback,
                trigger: WhenSelfEntersBattlefield, effect: [],
                program: Some((body: (steps: [(id: "choose_endure", kind: Choose(
                    choice: "endure", chooser: SourceController, optional: false,
                    branches: [
                        (branch_id: "counters", presentation: Fallback, cost: None,
                            requirement: EffectsApplicable,
                            body: (steps: [(id: "put_counter", kind: Perform(PutCounters(
                                counter: {counter}, count: 1, subject: Source,
                            )))], yields: [])),
                        (branch_id: "spirit", presentation: Fallback, cost: None,
                            requirement: Always,
                            body: (steps: [(id: "create_spirit", kind: Perform(CreateTokens(
                                token: "spirit_w_1_1", count: 1, who: SourceController,
                                tapped: false, sacrifice_timing: None,
                            )))], yields: [])),
                    ],
                    otherwise: (steps: [], yields: []),
                ))], yields: []), target_bindings: [])))]
        )"#
    );
    let registry = Box::leak(Box::new(
        tricerules_cards::CardRegistry::from_chunks_and_tokens(
            &[
                &card,
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[include_str!(
                "../../../tricerules-cards/data/tokens/spirit_w_1_1.ron"
            )],
        )
        .expect("synthetic noncreature Endure fixture compiles"),
    ));
    let decks = Some(vec![
        std::iter::repeat_n("forest".to_string(), 20).collect(),
        std::iter::repeat_n("forest".to_string(), 20).collect(),
    ]);
    let mut engine = GameEngine::new(registry, seed, &[0, 1], 20, decks, true)
        .expect("synthetic noncreature Endure engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn zombie_endure_reanimation_engine(seed: u64) -> GameEngine {
    let card_id = "issue_142_zombie_endure_fixture";
    let card = format!(
        r#"(id: "{card_id}", name: "{card_id}", face_id: "{card_id}",
            mana_cost: "{{2}}{{B}}", types: ["Creature", "Zombie"], power: 2, toughness: 1,
            triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback,
                trigger: WhenSelfEntersBattlefield, effect: [],
                program: Some((body: (steps: [(id: "choose_endure", kind: Choose(
                    choice: "endure", chooser: SourceController, optional: false,
                    branches: [
                        (branch_id: "counters", presentation: Fallback, cost: None,
                            requirement: EffectsApplicable,
                            body: (steps: [(id: "put_counter", kind: Perform(PutCounters(
                                counter: PlusOnePlusOne, count: 1, subject: Source,
                            )))], yields: [])),
                        (branch_id: "spirit", presentation: Fallback, cost: None,
                            requirement: Always,
                            body: (steps: [(id: "create_spirit", kind: Perform(CreateTokens(
                                token: "spirit_w_1_1", count: 1, who: SourceController,
                                tapped: false, sacrifice_timing: None,
                            )))], yields: [])),
                    ],
                    otherwise: (steps: [], yields: []),
                ))], yields: []), target_bindings: []))) ]
        )"#
    );
    let registry = Box::leak(Box::new(
        tricerules_cards::CardRegistry::from_chunks_and_tokens(
            &[
                &card,
                include_str!("../../../tricerules-cards/data/deadly_plot.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/lightning_bolt.ron"),
            ],
            &[include_str!(
                "../../../tricerules-cards/data/tokens/spirit_w_1_1.ron"
            )],
        )
        .expect("synthetic Zombie Endure fixture and existing instant spells compile"),
    ));
    let decks = Some(vec![
        std::iter::repeat_n("forest".to_string(), 20).collect(),
        std::iter::repeat_n("forest".to_string(), 20).collect(),
    ]);
    let mut engine = GameEngine::new(registry, seed, &[0, 1], 20, decks, true)
        .expect("synthetic Zombie Endure reanimation engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

#[test]
fn issue_142_endure_cards_are_registered() {
    let registry = tricerules_cards::registry::global();

    for card_id in [
        "kin-tree_nurturer",
        "dusyut_earthcarver",
        "sandskitter_outrider",
    ] {
        assert!(
            registry.get(card_id).is_some(),
            "issue #142 requires {card_id} to be implemented"
        );
    }
}

#[test]
fn endure_one_two_and_three_offer_both_branches_and_apply_the_choice() {
    for (seed, card_id, amount, token_id) in [
        (142_001, "kin-tree_nurturer", 1, "spirit_w_1_1"),
        (142_002, "sandskitter_outrider", 2, "spirit_w_2_2"),
        (142_003, "dusyut_earthcarver", 3, "spirit_w_3_3"),
    ] {
        let mut counters_engine = engine(seed);
        let source = cast_endure_creature(&mut counters_engine, card_id);
        pass_both_players(&mut counters_engine);
        let pending = counters_engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Endure branch choice");
        assert_eq!(pending.deciding_player, 0);
        counters_engine
            .apply_scenario_command(0, &select_branch(0))
            .expect("choose counters");
        assert_eq!(
            counters_engine
                .state
                .objects
                .get(&source)
                .expect("source")
                .counter_count(CounterKind::PlusOnePlusOne),
            amount
        );
        assert!(battlefield_token_oids(&counters_engine, 0, token_id).is_empty());

        let mut token_engine = engine(seed + 100);
        cast_endure_creature(&mut token_engine, card_id);
        pass_both_players(&mut token_engine);
        token_engine
            .apply_scenario_command(0, &select_branch(1))
            .expect("choose Spirit");
        assert_eq!(battlefield_token_oids(&token_engine, 0, token_id).len(), 1);
    }
}

#[test]
fn removed_source_forces_the_spirit_fallback_without_a_prompt() {
    let mut engine = engine(142_010);
    let source = cast_endure_creature(&mut engine, "kin-tree_nurturer");
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
        .apply_scenario_command(0, &cast_spell(bolt, target_object(source)))
        .expect("cast Lightning Bolt over the Endure trigger");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects.get(&source).expect("source").zone,
        tricerules_core::Zone::Graveyard
    );

    pass_both_players(&mut engine);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(battlefield_token_oids(&engine, 0, "spirit_w_1_1").len(), 1);
}

#[test]
fn source_controller_chooses_and_receives_the_spirit() {
    let mut engine = engine(142_020);
    let source = cast_endure_creature(&mut engine, "sandskitter_outrider");
    transfer_control(&mut engine, source, 1);

    pass_both_players(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Endure prompt")
            .deciding_player,
        1
    );
    engine
        .apply_scenario_command(1, &select_branch(1))
        .expect("source controller chooses Spirit");
    assert_eq!(battlefield_token_oids(&engine, 1, "spirit_w_2_2").len(), 1);
    assert!(battlefield_token_oids(&engine, 0, "spirit_w_2_2").is_empty());
}

#[test]
fn source_controller_lki_routes_forced_fallback_after_source_leaves() {
    let mut engine = engine(142_030);
    let source = cast_endure_creature(&mut engine, "kin-tree_nurturer");
    transfer_control(&mut engine, source, 1);
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
        .apply_scenario_command(0, &cast_spell(bolt, target_object(source)))
        .expect("cast Lightning Bolt");
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(battlefield_token_oids(&engine, 1, "spirit_w_1_1").len(), 1);
    assert!(battlefield_token_oids(&engine, 0, "spirit_w_1_1").is_empty());
}

#[test]
fn endure_program_counter_path_applies_to_a_noncreature_source() {
    let card_id = "issue_142_noncreature_endure_fixture";
    let mut engine = noncreature_endure_engine(142_060, card_id, false);
    let source = cast_noncreature_endure(&mut engine, card_id);
    assert!(
        engine.effective_power(source).is_none(),
        "the Endure source is a noncreature artifact"
    );
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the artifact ETB trigger is stacked"
    );

    pass_both_players(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("both Endure branches are legal")
            .deciding_player,
        0
    );
    engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("choose the counter branch on a noncreature source");

    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn effects_applicable_source_counter_branch_applies_a_charge_counter() {
    let card_id = "issue_142_charge_counter_endure_fixture";
    let mut engine = noncreature_endure_engine_with_counter(142_063, card_id, false, "Charge");
    let source = cast_noncreature_endure(&mut engine, card_id);
    pass_both_players(&mut engine);

    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the Charge counter branch is currently applicable")
            .deciding_player,
        0
    );
    engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("choose the fixed Charge counter branch");

    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Charge),
        1,
        "the existing source counter effect applies the authored kind"
    );
    assert!(battlefield_token_oids(&engine, 0, "spirit_w_1_1").is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn prohibited_endure_counter_branch_auto_falls_back_without_a_prompt() {
    let card_id = "issue_142_prohibited_noncreature_endure_fixture";
    let mut engine = noncreature_endure_engine(142_061, card_id, true);
    let source = cast_noncreature_endure(&mut engine, card_id);
    pass_both_players(&mut engine);

    assert!(
        engine.state.pending_resolution.is_none(),
        "the modeled prohibition removes the counter branch, leaving mandatory Spirit fallback"
    );
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    assert_eq!(battlefield_token_oids(&engine, 0, "spirit_w_1_1").len(), 1);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn old_endure_lki_uses_p1_after_the_same_card_returns_under_p0() {
    let card_id = "issue_142_zombie_endure_fixture";
    let mut engine = zombie_endure_reanimation_engine(142_062);
    let source = cast_endure_creature(&mut engine, card_id);
    transfer_control(&mut engine, source, 1);
    let old_generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the original Endure trigger remains stacked"
    );
    assert_eq!(engine.state.stack[0].source_zone_change, old_generation);

    inject_card_into_hand(&mut engine, 0, "lightning_bolt");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(
            0,
            &cast_spell(
                hand_index_for_card(&engine, 0, "lightning_bolt"),
                target_object(source),
            ),
        )
        .expect("cast Lightning Bolt over the old Endure trigger");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&source].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        engine
            .state
            .last_known_controller_by_generation
            .get(&(source, old_generation)),
        Some(&1),
        "the real zone change captures P1 as the old incarnation's LKI controller"
    );

    inject_card_into_hand(&mut engine, 0, "deadly_plot");
    grant_pool(&mut engine, 0);
    engine
        .apply_scenario_command(
            0,
            &cast_modal_spell(
                hand_index_for_card(&engine, 0, "deadly_plot"),
                vec![(1, target_object(source))],
            ),
        )
        .expect("cast Deadly Plot's Zombie return mode at instant speed");
    pass_both_players(&mut engine);
    let new_generation = engine.state.zone_change_generation[&source];
    assert!(
        new_generation > old_generation,
        "real zone changes create a new incarnation"
    );
    assert_eq!(engine.state.objects[&source].controller, 0);
    assert_eq!(
        engine.state.stack.len(),
        2,
        "both ETB triggers remain on the stack"
    );

    pass_both_players(&mut engine);
    assert!(
        engine.state.pending_resolution.is_some(),
        "the new incarnation can Endure"
    );
    engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("the new incarnation takes its own counter branch");
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "the new generation receives its counter"
    );

    pass_both_players(&mut engine);
    assert!(
        engine.state.pending_resolution.is_none(),
        "the old generation's counter choice is inapplicable and auto-falls back"
    );
    assert_eq!(battlefield_token_oids(&engine, 1, "spirit_w_1_1").len(), 1);
    assert!(battlefield_token_oids(&engine, 0, "spirit_w_1_1").is_empty());
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "the old trigger cannot put counters on the returned incarnation"
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn endure_counter_child_replacement_parks_and_resumes_once() {
    let mut engine = engine(142_063);
    let source = cast_endure_creature(&mut engine, "kin-tree_nurturer");
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    pass_both_players(&mut engine);
    assert!(
        engine.state.pending_resolution.is_some(),
        "Endure offers both branches"
    );

    let counter_offer = engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("select the direct PutCounters Endure child");
    let replacement = find_resolution_choice(&counter_offer)
        .expect("two counter replacements park the Endure child for ordering");
    assert_eq!(
        replacement.choice_kind,
        tricerules_proto::ruled::v1::ChoiceKind::ReplacementEffect as i32
    );
    assert_eq!(replacement.candidate_object_ids.len(), 2);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        0
    );

    let resumed = engine
        .apply_scenario_command(
            0,
            &submit_resolution_choice(vec![replacement.candidate_object_ids[0]]),
        )
        .expect("order the replacement effects and resume the parked PutCounters child");
    assert!(find_resolution_choice(&resumed).is_none());
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        4,
        "one authored counter is doubled once by each replacement, despite the child parking"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn leave_and_return_generation_forces_fallback_and_stale_counter_choice_is_restored() {
    let mut engine = engine(142_040);
    let source = cast_endure_creature(&mut engine, "kin-tree_nurturer");
    pass_both_players(&mut engine);
    let original_generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    engine
        .state
        .last_known_controller_by_generation
        .insert((source, original_generation), 0);
    *engine
        .state
        .zone_change_generation
        .entry(source)
        .or_insert(0) += 2;

    assert!(engine.apply_scenario_command(0, &select_branch(0)).is_err());
    assert!(engine.state.pending_resolution.is_some());
    engine
        .apply_scenario_command(0, &select_branch(1))
        .expect("fallback remains legal");
    assert_eq!(battlefield_token_oids(&engine, 0, "spirit_w_1_1").len(), 1);
    assert_eq!(
        engine
            .state
            .objects
            .get(&source)
            .expect("returned source")
            .counter_count(CounterKind::PlusOnePlusOne),
        0
    );
}

#[test]
fn endure_replay_is_deterministic_for_the_same_seed_and_commands() {
    fn replay() -> (usize, u32, u64) {
        let mut engine = engine(142_050);
        let source = cast_endure_creature(&mut engine, "dusyut_earthcarver");
        pass_both_players(&mut engine);
        engine
            .apply_scenario_command(0, &select_branch(0))
            .expect("choose counters");
        (
            battlefield_token_oids(&engine, 0, "spirit_w_3_3").len(),
            engine
                .state
                .objects
                .get(&source)
                .expect("source")
                .counter_count(CounterKind::PlusOnePlusOne),
            engine.state.command_index,
        )
    }

    assert_eq!(replay(), replay());
}
