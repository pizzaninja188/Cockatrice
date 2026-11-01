//! Capability-only fixtures for Tekuthal's Proliferate action replacement.
//! Actual Karn's Bastion and Tezzeret's Gambit definitions are included unchanged from shipped
//! card data; the local static fixture is not an admitted Tekuthal card.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use std::sync::OnceLock;
use tricerules_card_model::card_def::RawCardDefinition;
use tricerules_card_model::{AbilityId, AbilityPresentation, IdentifiedStaticAbility};
use tricerules_cards::primitives::CounterKind;
use tricerules_cards::primitives::{
    ContinuousEffectKind, ControllerReference, EffectDuration, StaticAbilityDef,
};
use tricerules_cards::CardRegistry;
use tricerules_core::state::{ClassLevelDesignation, CopiableValues, RoomState};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1 as ruled_v1;
use tricerules_proto::ruled::v1::{
    ruled_event, ResolutionChoiceDecision, ResolutionChoiceRequired, RuledEventBatch,
    SubmitResolutionChoice,
};

const STATIC_FIXTURE_ID: &str = "tekuthal_proliferate_capability_fixture";
const ROOM_FIXTURE_ID: &str = "tekuthal_left_tekuthal_right";
const CLASS_FIXTURE_ID: &str = "tekuthal_class_capability_fixture";
const STATIC_FIXTURE_RON: &str = r#"(
    id: "tekuthal_proliferate_capability_fixture",
    name: "Tekuthal Proliferate Capability Fixture",
    face_id: "tekuthal_proliferate_capability_fixture",
    types: ["Creature", "Phyrexian", "Horror"],
    power: 3,
    toughness: 5,
    static_abilities: [(
        ability_id: "tekuthal_static",
        presentation: Fallback,
        definition: DoubleControllerProliferations,
    )],
)"#;

fn runtime_registry() -> &'static CardRegistry {
    static REGISTRY: OnceLock<CardRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        CardRegistry::from_chunks_and_tokens(
            &[
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/doubling_season.ron"),
                include_str!("../../../tricerules-cards/data/karns_bastion.ron"),
                include_str!("../../../tricerules-cards/data/ripples_of_potential.ron"),
                include_str!("../../../tricerules-cards/data/tezzerets_gambit.ron"),
                include_str!("../../../tricerules-cards/data/ezuri_stalker_of_spheres.ron"),
                STATIC_FIXTURE_RON,
            ],
            &[],
        )
        .expect("capability-only Tekuthal fixture and shipped action sources are valid")
    })
}

fn runtime_engine(seed: u64) -> GameEngine {
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        runtime_registry(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("Tekuthal action fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn runtime_four_player_engine(seed: u64) -> GameEngine {
    let deck = std::iter::repeat_n("forest".to_string(), 30).collect::<Vec<_>>();
    let mut engine = GameEngine::new(
        runtime_registry(),
        seed,
        &[0, 1, 2, 3],
        20,
        Some(vec![deck; 4]),
        true,
    )
    .expect("four-player Tekuthal action fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn put_poison_on_player(engine: &mut GameEngine, player_id: i32) {
    let index = engine.state.player_idx(player_id).expect("fixture player");
    engine.state.players[index]
        .counters
        .insert(CounterKind::Poison, 1);
}

fn proliferate_choice(objects: Vec<u32>, players: Vec<i32>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            chosen_object_ids: objects,
            chosen_player_ids: players,
            decision: ResolutionChoiceDecision::Unspecified as i32,
            selected_branch_index: 0,
            ..Default::default()
        })),
    }
}

fn assert_proliferate_choice(engine: &GameEngine, label: &str) {
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap_or_else(|| panic!("{label}: a Proliferate child must offer recipients"));
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::Proliferate,
        "{label}"
    );
}

fn replacement_choice(batch: &RuledEventBatch) -> ResolutionChoiceRequired {
    batch
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref()? {
            ruled_event::Ev::ResolutionChoiceRequired(choice)
                if choice.choice_kind == ChoiceKind::ReplacementEffect as i32 =>
            {
                Some(choice.clone())
            }
            _ => None,
        })
        .expect("the current replacement offer is included in the command result")
}

fn answer_one_empty_leaf(engine: &mut GameEngine, actor: i32, label: &str) {
    assert_proliferate_choice(engine, label);
    let batch = engine
        .apply_scenario_command(actor, &proliferate_choice(vec![], vec![]))
        .expect("the next Proliferate child accepts an empty subset");
    assert_eq!(
        proliferate_log_count(&batch, actor),
        1,
        "each child emits one Proliferated log"
    );
}

fn proliferate_log_count(batch: &RuledEventBatch, actor: i32) -> usize {
    let expected = format!("P{actor} proliferates.");
    batch
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref()? {
            ruled_event::Ev::Log(log) => Some(&log.text),
            _ => None,
        })
        .filter(|text| text.as_str() == expected)
        .count()
}

fn answer_empty_leaf_count(engine: &mut GameEngine, actor: i32, count: usize, label: &str) {
    for leaf in 0..count {
        answer_one_empty_leaf(engine, actor, label);
        if leaf + 1 < count {
            assert_proliferate_choice(engine, "the replacement tree has another child");
        }
    }
    assert!(
        engine.state.pending_resolution.is_none(),
        "all {count} Proliferate children complete before the original instruction resumes"
    );
}

fn answer_first_leaf_and_require_second(engine: &mut GameEngine, label: &str) {
    answer_empty_leaf_count(engine, 0, 2, label);
}

fn pass_live_players_with_batch(engine: &mut GameEngine) -> RuledEventBatch {
    let pass_count = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count();
    let mut batch = RuledEventBatch::default();
    for _ in 0..pass_count {
        let priority = engine.state.priority_player_id();
        batch = engine
            .apply_scenario_command(priority, &pass())
            .expect("every live player passes priority");
    }
    batch
}

fn raw_card_fixture(source: &str) -> RawCardDefinition {
    ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(source)
        .expect("local source fixture parses as RawCardDefinition")
}

fn copied_static_fixture(ability_id: &str) -> IdentifiedStaticAbility {
    let source = raw_card_fixture(STATIC_FIXTURE_RON);
    let mut ability = source
        .static_abilities
        .into_iter()
        .find(|ability| ability.ability_id.to_string() == "tekuthal_static")
        .expect("the validated local static fixture has its identified doubling ability");
    ability.ability_id = AbilityId::new(ability_id).expect("valid local ability id");
    ability.presentation = AbilityPresentation::Fallback;
    ability
}

fn local_room_fixture_ron() -> String {
    let source = include_str!("../../../tricerules-cards/data/derelict_attic__widows_walk.ron");
    let mut room = raw_card_fixture(source);
    room.id = ROOM_FIXTURE_ID.into();
    room.name = "Tekuthal Left // Tekuthal Right".into();
    assert_eq!(
        room.faces.len(),
        2,
        "the shipped Room scaffold has two doors"
    );
    assert_ne!(
        room.faces[0].face_id, room.faces[1].face_id,
        "keep the scaffold's distinct stable face identities"
    );
    for (face, name) in room
        .faces
        .iter_mut()
        .zip(["Tekuthal Left", "Tekuthal Right"])
    {
        assert!(face.types.iter().any(|type_line| type_line == "Room"));
        assert!(face
            .types
            .iter()
            .any(|type_line| type_line == "Enchantment"));
        face.name = name.into();
        face.triggered_abilities
            .retain(|ability| ability.ability_id.to_string() != "triggered_01");
        assert!(
            face.triggered_abilities.is_empty(),
            "the Room fixture retains no scaffold triggers"
        );
        face.static_abilities = vec![copied_static_fixture("door_static")];
    }
    ron::ser::to_string(&room).expect("typed Room fixture serializes")
}

fn local_class_fixture_ron() -> String {
    let source = include_str!("../../../tricerules-cards/data/wizard_class.ron");
    let mut class = raw_card_fixture(source);
    class.id = CLASS_FIXTURE_ID.into();
    class.name = "Tekuthal Class Capability Fixture".into();
    class
        .static_abilities
        .retain(|ability| ability.ability_id.to_string() != "no_max_hand_size");
    assert!(
        class.static_abilities.is_empty(),
        "the base no-maximum-hand-size static is removed from this fixture"
    );
    let levels = class
        .class_level_bars
        .iter()
        .map(|bar| bar.level)
        .collect::<Vec<_>>();
    assert_eq!(levels, vec![2, 3], "retain the required Class level bars");
    for bar in &mut class.class_level_bars {
        bar.triggered_abilities.retain(|ability| {
            !matches!(
                ability.ability_id.to_string().as_str(),
                "level_2_trigger" | "level_3_trigger"
            )
        });
        assert!(
            bar.triggered_abilities.is_empty(),
            "the fixture retains no level transition or draw triggers"
        );
        bar.level_ability.presentation = AbilityPresentation::Fallback;
        bar.static_abilities.clear();
        let ability_id = match bar.level {
            2 => "class_two_static",
            3 => "class_three_static",
            level => panic!("unexpected Class bar level {level}"),
        };
        bar.static_abilities.push(copied_static_fixture(ability_id));
    }
    ron::ser::to_string(&class).expect("typed Class fixture serializes")
}

fn runtime_registry_with_local_fixtures(local_fixtures: &[&str]) -> &'static CardRegistry {
    let mut chunks = vec![
        include_str!("../../../tricerules-cards/data/forest.ron"),
        include_str!("../../../tricerules-cards/data/doubling_season.ron"),
        include_str!("../../../tricerules-cards/data/karns_bastion.ron"),
        include_str!("../../../tricerules-cards/data/ripples_of_potential.ron"),
        include_str!("../../../tricerules-cards/data/tezzerets_gambit.ron"),
        include_str!("../../../tricerules-cards/data/ezuri_stalker_of_spheres.ron"),
        STATIC_FIXTURE_RON,
    ];
    chunks.extend_from_slice(local_fixtures);
    let registry = CardRegistry::from_chunks_and_tokens(&chunks, &[])
        .expect("local typed capability fixture validates with unchanged shipped data");
    Box::leak(Box::new(registry))
}

fn runtime_engine_with_registry(seed: u64, registry: &'static CardRegistry) -> GameEngine {
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("prepared capability fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn published_choice(batch: &RuledEventBatch, choice_kind: ChoiceKind) -> ResolutionChoiceRequired {
    batch
        .events
        .iter()
        .rev()
        .find_map(|event| match event.ev.as_ref()? {
            ruled_event::Ev::ResolutionChoiceRequired(choice)
                if choice.choice_kind == choice_kind as i32 =>
            {
                Some(choice.clone())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("the command batch publishes its {choice_kind:?} choice"))
}

fn resolve_bastion_action(
    engine: &mut GameEngine,
    bastion: u32,
    recipient: u32,
    expected_leaves: usize,
    expected_order_source: Option<(u32, u64)>,
) -> usize {
    grant_pool(engine, 0);
    apply_ability(engine, 0, bastion, 1, vec![]).expect("activate shipped Karn's Bastion");
    let mut batch = pass_both_players_with_batch(engine);
    let mut completed_leaves = 0;
    let mut proliferate_logs = 0;
    let mut checked_order_source = false;

    for step in 0..32 {
        let Some(pending) = engine.state.pending_resolution.as_ref() else {
            break;
        };
        assert!(
            step < 31,
            "Bastion replacement resolution exceeded its bounded test guard"
        );
        let choice_kind = pending.presentation.choice_kind;
        let choice = published_choice(&batch, choice_kind);
        match choice_kind {
            ChoiceKind::ReplacementEffect => {
                assert!(!choice.replacement_options.is_empty());
                if let Some((source, generation)) = expected_order_source {
                    assert!(
                        !checked_order_source,
                        "the expected order prompt occurs once"
                    );
                    assert_eq!(choice.min, 1);
                    assert_eq!(choice.max, 1);
                    assert_eq!(choice.replacement_options.len(), 2);
                    let offered_ids = choice
                        .replacement_options
                        .iter()
                        .map(|option| option.application_id)
                        .collect::<Vec<_>>();
                    assert_ne!(offered_ids[0], offered_ids[1]);
                    assert!(offered_ids
                        .iter()
                        .all(|application_id| *application_id != 0));
                    assert!(choice.replacement_options.iter().all(|option| {
                        option.source_object_id == source
                            && option.source_zone_change_generation == generation
                    }));
                    assert_eq!(
                        engine
                            .state
                            .pending_resolution
                            .as_ref()
                            .expect("the order prompt remains pending")
                            .deciding_player,
                        0
                    );
                    checked_order_source = true;
                }
                let application_id = choice.replacement_options[0].application_id;
                batch = engine
                    .apply_scenario_command(0, &proliferate_choice(vec![application_id], vec![]))
                    .expect("choose one published opaque replacement application");
            }
            ChoiceKind::Proliferate => {
                let candidates = &pending.presentation.candidates;
                assert_eq!(
                    candidates,
                    &[recipient],
                    "the counter-bearing Forest is the only current recipient"
                );
                batch = engine
                    .apply_scenario_command(0, &proliferate_choice(vec![recipient], vec![]))
                    .expect("choose the same counter-bearing Forest for this Proliferate leaf");
                let logs = proliferate_log_count(&batch, 0);
                assert_eq!(logs, 1, "each completed leaf emits exactly one P0 log");
                proliferate_logs += logs;
                completed_leaves += 1;
            }
            other => panic!("unexpected Bastion choice kind {other:?}"),
        }
    }

    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(completed_leaves, expected_leaves);
    assert_eq!(proliferate_logs, expected_leaves);
    if expected_order_source.is_some() {
        assert!(
            checked_order_source,
            "the required same-source order prompt was published"
        );
    }
    proliferate_logs
}

#[test]
fn prepared_static_doubles_karns_bastion_action_before_recipient_choice() {
    let mut engine = runtime_engine(810_001);
    inject_permanent_on_battlefield(&mut engine, 0, STATIC_FIXTURE_ID);
    put_poison_on_player(&mut engine, 0);
    let bastion = inject_permanent_on_battlefield(&mut engine, 0, "karns_bastion");
    grant_pool(&mut engine, 0);

    apply_ability(&mut engine, 0, bastion, 1, vec![]).expect("activate shipped Karn's Bastion");
    pass_both_players_with_batch(&mut engine);
    answer_first_leaf_and_require_second(&mut engine, "Tekuthal creates a second Bastion child");
}

#[test]
fn prepared_static_doubles_tezzerets_gambit_action_after_draw() {
    let mut engine = runtime_engine(810_002);
    inject_permanent_on_battlefield(&mut engine, 0, STATIC_FIXTURE_ID);
    put_poison_on_player(&mut engine, 0);
    let gambit = inject_card_into_hand(&mut engine, 0, "tezzerets_gambit");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "tezzerets_gambit");

    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast shipped Tezzeret's Gambit");
    pass_both_players_with_batch(&mut engine);
    answer_first_leaf_and_require_second(&mut engine, "Tekuthal creates a second Gambit child");
    assert_eq!(engine.state.objects[&gambit].zone, Zone::Graveyard);
}

#[test]
fn no_matching_static_keeps_the_single_original_proliferate_action() {
    let mut engine = runtime_engine(810_003);
    put_poison_on_player(&mut engine, 0);
    let gambit = inject_card_into_hand(&mut engine, 0, "tezzerets_gambit");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "tezzerets_gambit");

    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast shipped Tezzeret's Gambit without a doubling source");
    pass_both_players_with_batch(&mut engine);
    answer_empty_leaf_count(&mut engine, 0, 1, "the unchanged single Proliferate action");
    assert_eq!(engine.state.objects[&gambit].zone, Zone::Graveyard);
}

#[test]
fn bastion_doubling_uses_current_controller_instead_of_source_owner() {
    let mut opponent_source_engine = runtime_engine(810_010);
    let opponent_source =
        inject_permanent_on_battlefield(&mut opponent_source_engine, 1, STATIC_FIXTURE_ID);
    put_poison_on_player(&mut opponent_source_engine, 0);
    let opponent_bastion =
        inject_permanent_on_battlefield(&mut opponent_source_engine, 0, "karns_bastion");
    grant_pool(&mut opponent_source_engine, 0);

    assert_eq!(
        opponent_source_engine.state.objects[&opponent_source].owner,
        1
    );
    assert_eq!(
        opponent_source_engine
            .characteristics(opponent_source)
            .unwrap()
            .controller,
        1
    );
    apply_ability(&mut opponent_source_engine, 0, opponent_bastion, 1, vec![])
        .expect("activate shipped Karn's Bastion with an opponent-controlled source");
    pass_both_players_with_batch(&mut opponent_source_engine);
    assert_proliferate_choice(
        &opponent_source_engine,
        "the opponent-controlled source does not apply",
    );
    let opponent_source_leaf = opponent_source_engine
        .apply_scenario_command(0, &proliferate_choice(vec![], vec![]))
        .expect("the original Proliferate leaf accepts an empty subset");
    assert_eq!(proliferate_log_count(&opponent_source_leaf, 0), 1);
    assert_eq!(proliferate_log_count(&opponent_source_leaf, 1), 0);
    assert!(opponent_source_engine.state.pending_resolution.is_none());
    assert!(opponent_source_engine.state.stack.is_empty());

    let mut stolen_source_engine = runtime_engine(810_011);
    let stolen_source =
        inject_permanent_on_battlefield(&mut stolen_source_engine, 1, STATIC_FIXTURE_ID);
    put_poison_on_player(&mut stolen_source_engine, 0);
    let stolen_bastion =
        inject_permanent_on_battlefield(&mut stolen_source_engine, 0, "karns_bastion");
    let control_timestamp = stolen_source_engine.state.command_index + 1;
    stolen_source_engine
        .state
        .continuous_effects
        .push(ContinuousEffect {
            source_id: None,
            trigger_grant_origin: None,
            affected: AffectedScope::Single(stolen_source),
            kind: ContinuousEffectKind::Layer2Control {
                controller: ControllerReference::Fixed(0),
            },
            condition: None,
            duration: EffectDuration::Indefinite,
            timestamp: control_timestamp,
        });
    grant_pool(&mut stolen_source_engine, 0);

    assert_eq!(stolen_source_engine.state.objects[&stolen_source].owner, 1);
    assert_eq!(
        stolen_source_engine
            .characteristics(stolen_source)
            .unwrap()
            .controller,
        0
    );
    apply_ability(&mut stolen_source_engine, 0, stolen_bastion, 1, vec![])
        .expect("activate shipped Karn's Bastion with the stolen source");
    pass_both_players_with_batch(&mut stolen_source_engine);
    for child in 0..2 {
        assert_proliferate_choice(
            &stolen_source_engine,
            "the actor-controlled source creates each child",
        );
        let child_batch = stolen_source_engine
            .apply_scenario_command(0, &proliferate_choice(vec![], vec![]))
            .expect("each doubled Proliferate leaf accepts an empty subset");
        assert_eq!(proliferate_log_count(&child_batch, 0), 1, "child {child}");
        assert_eq!(proliferate_log_count(&child_batch, 1), 0, "child {child}");
    }
    assert!(stolen_source_engine.state.pending_resolution.is_none());
    assert!(stolen_source_engine.state.stack.is_empty());
}

#[test]
fn bastion_doubling_stops_for_removed_abilities_and_restores_without_source_reentry() {
    let mut engine = runtime_engine(810_012);
    let source = inject_permanent_on_battlefield(&mut engine, 0, STATIC_FIXTURE_ID);
    let bastion = inject_permanent_on_battlefield(&mut engine, 0, "karns_bastion");
    put_poison_on_player(&mut engine, 0);
    grant_pool(&mut engine, 0);
    let source_generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    let removal_timestamp = engine.state.command_index + 1;
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: removal_timestamp,
    });

    apply_ability(&mut engine, 0, bastion, 1, vec![])
        .expect("activate shipped Karn's Bastion while the source has no abilities");
    pass_both_players_with_batch(&mut engine);
    assert_proliferate_choice(&engine, "ability removal suppresses the doubling source");
    let suppressed_leaf = engine
        .apply_scenario_command(0, &proliferate_choice(vec![], vec![]))
        .expect("the suppressed source leaves one empty Proliferate child");
    assert_eq!(proliferate_log_count(&suppressed_leaf, 0), 1);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0),
        source_generation
    );

    let matching_removals = engine
        .state
        .continuous_effects
        .iter()
        .filter(|effect| {
            effect.timestamp == removal_timestamp
                && effect.affected == AffectedScope::Single(source)
                && effect.kind == ContinuousEffectKind::Layer6RemoveAllAbilities
        })
        .count();
    assert_eq!(
        matching_removals, 1,
        "the prepared removal is uniquely identified"
    );
    let removal_index = engine
        .state
        .continuous_effects
        .iter()
        .position(|effect| {
            effect.timestamp == removal_timestamp
                && effect.affected == AffectedScope::Single(source)
                && effect.kind == ContinuousEffectKind::Layer6RemoveAllAbilities
        })
        .expect("find the exact prepared removal effect");
    let removed_effect = engine.state.continuous_effects.remove(removal_index);
    assert_eq!(removed_effect.affected, AffectedScope::Single(source));
    assert_eq!(
        removed_effect.kind,
        ContinuousEffectKind::Layer6RemoveAllAbilities
    );
    assert_eq!(removed_effect.timestamp, removal_timestamp);
    assert!(!engine.state.continuous_effects.iter().any(|effect| {
        effect.timestamp == removal_timestamp
            && effect.affected == AffectedScope::Single(source)
            && effect.kind == ContinuousEffectKind::Layer6RemoveAllAbilities
    }));

    engine.state.objects.get_mut(&bastion).unwrap().tapped = false;
    grant_pool(&mut engine, 0);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0),
        source_generation
    );
    apply_ability(&mut engine, 0, bastion, 1, vec![])
        .expect("activate shipped Karn's Bastion after the source regains its abilities");
    pass_both_players_with_batch(&mut engine);
    for child in 0..2 {
        assert_proliferate_choice(&engine, "the restored source doubles the later action");
        let child_batch = engine
            .apply_scenario_command(0, &proliferate_choice(vec![], vec![]))
            .expect("each restored Proliferate leaf accepts an empty subset");
        assert_eq!(proliferate_log_count(&child_batch, 0), 1, "child {child}");
    }
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0),
        source_generation
    );
}

#[test]
fn phasing_suppresses_source_and_rejects_stale_recipient_before_fresh_leaf() {
    let mut engine = runtime_engine(810_013);
    let source = inject_permanent_on_battlefield(&mut engine, 0, STATIC_FIXTURE_ID);
    let recipient_r = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let recipient_s = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let bastion = inject_permanent_on_battlefield(&mut engine, 0, "karns_bastion");
    engine
        .state
        .objects
        .get_mut(&recipient_r)
        .unwrap()
        .set_counter(CounterKind::Charge, 1);
    engine
        .state
        .objects
        .get_mut(&recipient_s)
        .unwrap()
        .set_counter(CounterKind::Charge, 1);
    let source_generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    let recipient_r_generation = engine
        .state
        .zone_change_generation
        .get(&recipient_r)
        .copied()
        .unwrap_or(0);
    let recipient_s_generation = engine
        .state
        .zone_change_generation
        .get(&recipient_s)
        .copied()
        .unwrap_or(0);
    engine.state.objects.get_mut(&source).unwrap().phase_status =
        Some(tricerules_core::state::PhasedOutStatus {
            phase_out_under: 0,
            phase_in_due_after_departed_turn: false,
            phase_in_with: None,
            direct_attachment_snapshot: None,
        });
    grant_pool(&mut engine, 0);

    apply_ability(&mut engine, 0, bastion, 1, vec![])
        .expect("activate shipped Karn's Bastion while the source is phased out");
    pass_both_players_with_batch(&mut engine);
    let phased_source_offer = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the phased source still leaves one ordinary Proliferate choice")
        .presentation
        .clone();
    assert_eq!(
        phased_source_offer.candidates,
        vec![recipient_r, recipient_s]
    );
    let phased_source_leaf = engine
        .apply_scenario_command(0, &proliferate_choice(vec![recipient_r], vec![]))
        .expect("choose R for the single leaf while the source is phased out");
    assert_eq!(proliferate_log_count(&phased_source_leaf, 0), 1);
    assert_eq!(
        engine.state.objects[&recipient_r].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(
        engine.state.objects[&recipient_s].counter_count(CounterKind::Charge),
        1
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());

    engine.state.objects.get_mut(&source).unwrap().phase_status = None;
    engine.state.objects.get_mut(&bastion).unwrap().tapped = false;
    grant_pool(&mut engine, 0);
    apply_ability(&mut engine, 0, bastion, 1, vec![])
        .expect("activate shipped Karn's Bastion after the source phases in");
    pass_both_players_with_batch(&mut engine);
    let doubled_offer = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the restored source doubles the Bastion action")
        .presentation
        .clone();
    assert_eq!(doubled_offer.candidates, vec![recipient_r, recipient_s]);
    engine
        .state
        .objects
        .get_mut(&recipient_r)
        .unwrap()
        .phase_status = Some(tricerules_core::state::PhasedOutStatus {
        phase_out_under: 0,
        phase_in_due_after_departed_turn: false,
        phase_in_with: None,
        direct_attachment_snapshot: None,
    });
    let state_before_stale_answer =
        serde_json::to_value(&engine.state).expect("serialize state before stale recipient answer");
    let stale_answer =
        engine.apply_scenario_command(0, &proliferate_choice(vec![recipient_r], vec![]));
    assert!(
        stale_answer.is_err(),
        "the phased-out offered recipient is stale"
    );
    assert_eq!(
        serde_json::to_value(&engine.state).expect("serialize state after stale recipient answer"),
        state_before_stale_answer
    );
    assert_proliferate_choice(
        &engine,
        "the first doubled leaf remains pending after rejection",
    );
    assert_eq!(
        engine.state.objects[&recipient_r].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(
        engine.state.objects[&recipient_s].counter_count(CounterKind::Charge),
        1
    );

    let first_doubled_leaf = engine
        .apply_scenario_command(0, &proliferate_choice(vec![recipient_s], vec![]))
        .expect("choose S for the still-pending first doubled leaf");
    assert_eq!(proliferate_log_count(&first_doubled_leaf, 0), 1);
    assert_eq!(
        engine.state.objects[&recipient_r].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(
        engine.state.objects[&recipient_s].counter_count(CounterKind::Charge),
        2
    );
    let fresh_leaf_offer = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the second doubled leaf gets a fresh recipient choice")
        .presentation
        .clone();
    assert_eq!(fresh_leaf_offer.candidates, vec![recipient_s]);

    let second_doubled_leaf = engine
        .apply_scenario_command(0, &proliferate_choice(vec![recipient_s], vec![]))
        .expect("choose S again for the fresh second doubled leaf");
    assert_eq!(proliferate_log_count(&second_doubled_leaf, 0), 1);
    assert_eq!(
        engine.state.objects[&recipient_r].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(
        engine.state.objects[&recipient_s].counter_count(CounterKind::Charge),
        3
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0),
        source_generation
    );
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&recipient_r)
            .copied()
            .unwrap_or(0),
        recipient_r_generation
    );
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&recipient_s)
            .copied()
            .unwrap_or(0),
        recipient_s_generation
    );
}

#[test]
fn two_independent_static_occurrences_create_four_depth_first_children() {
    let mut engine = runtime_engine(810_004);
    let first_source = inject_permanent_on_battlefield(&mut engine, 0, STATIC_FIXTURE_ID);
    let second_source = inject_permanent_on_battlefield(&mut engine, 0, STATIC_FIXTURE_ID);
    put_poison_on_player(&mut engine, 0);
    let gambit = inject_card_into_hand(&mut engine, 0, "tezzerets_gambit");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "tezzerets_gambit");

    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast shipped Tezzeret's Gambit with two nonlegendary source instances");
    let outer_batch = pass_both_players_with_batch(&mut engine);
    let choice = replacement_choice(&outer_batch);
    assert_eq!(choice.min, 1);
    assert_eq!(choice.max, 1);
    assert_eq!(choice.replacement_options.len(), 2);
    assert_ne!(
        choice.replacement_options[0].application_id,
        choice.replacement_options[1].application_id
    );
    let mut sources = choice
        .replacement_options
        .iter()
        .map(|option| {
            (
                option.source_object_id,
                option.source_zone_change_generation,
            )
        })
        .collect::<Vec<_>>();
    sources.sort_unstable();
    assert_eq!(
        sources,
        vec![
            (
                first_source,
                engine
                    .state
                    .zone_change_generation
                    .get(&first_source)
                    .copied()
                    .unwrap_or(0),
            ),
            (
                second_source,
                engine
                    .state
                    .zone_change_generation
                    .get(&second_source)
                    .copied()
                    .unwrap_or(0),
            ),
        ]
    );
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        0,
        "the original action actor chooses the replacement order"
    );

    let selected_handle = choice.replacement_options[0].application_id;
    let pending_before = format!("{:?}", engine.state.pending_resolution);
    assert!(
        engine
            .apply_scenario_command(1, &proliferate_choice(vec![selected_handle], vec![]))
            .is_err(),
        "a different player cannot answer the original actor's offer"
    );
    assert_eq!(
        format!("{:?}", engine.state.pending_resolution),
        pending_before,
        "wrong-actor answer leaves the offer intact"
    );
    assert!(
        engine
            .apply_scenario_command(0, &proliferate_choice(vec![u32::MAX], vec![]))
            .is_err(),
        "an unoffered opaque replacement handle is rejected"
    );
    assert_eq!(
        format!("{:?}", engine.state.pending_resolution),
        pending_before,
        "foreign handle leaves the offer intact"
    );

    engine
        .apply_scenario_command(0, &proliferate_choice(vec![selected_handle], vec![]))
        .expect("the actor applies one of the offered replacements");
    assert_proliferate_choice(&engine, "first depth-first child");
    let pending_after_order = format!("{:?}", engine.state.pending_resolution);
    assert!(
        engine
            .apply_scenario_command(0, &proliferate_choice(vec![selected_handle], vec![]))
            .is_err(),
        "an accepted replacement handle cannot be replayed as a recipient selection"
    );
    assert_eq!(
        format!("{:?}", engine.state.pending_resolution),
        pending_after_order,
        "duplicate old handle leaves the child prompt intact"
    );
    answer_empty_leaf_count(
        &mut engine,
        0,
        4,
        "four children from two replacement occurrences",
    );
    assert_eq!(engine.state.objects[&gambit].zone, Zone::Graveyard);
}

#[test]
fn bastion_children_recompute_nonempty_recipients_and_commit_separate_counter_events() {
    let mut engine = runtime_engine(810_005);
    inject_permanent_on_battlefield(&mut engine, 0, STATIC_FIXTURE_ID);
    let first = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let second = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    engine
        .state
        .objects
        .get_mut(&first)
        .expect("first counter-bearing permanent")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    engine
        .state
        .objects
        .get_mut(&first)
        .expect("first counter-bearing permanent")
        .set_counter(CounterKind::Stun, 1);
    engine
        .state
        .objects
        .get_mut(&second)
        .expect("second counter-bearing permanent")
        .set_counter(CounterKind::Charge, 1);
    let bastion = inject_permanent_on_battlefield(&mut engine, 0, "karns_bastion");
    grant_pool(&mut engine, 0);

    apply_ability(&mut engine, 0, bastion, 1, vec![]).expect("activate shipped Karn's Bastion");
    pass_both_players_with_batch(&mut engine);
    assert_proliferate_choice(
        &engine,
        "the first child offers existing counter recipients",
    );
    let first_leaf = engine
        .apply_scenario_command(0, &proliferate_choice(vec![first], vec![]))
        .expect("choose A for the first Proliferate child");
    assert_eq!(proliferate_log_count(&first_leaf, 0), 1);
    let first_counter_events = first_leaf
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref()? {
            ruled_event::Ev::Log(log) => Some(log.text.as_str()),
            _ => None,
        })
        .filter(|text| text.contains("gets one of each kind of counter"))
        .count();
    assert_eq!(first_counter_events, 1);
    assert_eq!(
        engine.state.objects[&first].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    assert_eq!(
        engine.state.objects[&first].counter_count(CounterKind::Stun),
        2
    );
    assert_eq!(
        engine.state.objects[&second].counter_count(CounterKind::Charge),
        1
    );

    let second_offer = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the second child gets a fresh recipient choice")
        .presentation
        .clone();
    assert_eq!(second_offer.choice_kind, ChoiceKind::Proliferate);
    assert_eq!(second_offer.candidates, vec![first, second]);
    let second_leaf = engine
        .apply_scenario_command(0, &proliferate_choice(vec![second], vec![]))
        .expect("choose B for the second child after A's completed counter batch");
    assert_eq!(proliferate_log_count(&second_leaf, 0), 1);
    let second_counter_events = second_leaf
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref()? {
            ruled_event::Ev::Log(log) => Some(log.text.as_str()),
            _ => None,
        })
        .filter(|text| text.contains("gets one of each kind of counter"))
        .count();
    assert_eq!(second_counter_events, 1);
    assert_eq!(
        engine.state.objects[&first].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    assert_eq!(
        engine.state.objects[&first].counter_count(CounterKind::Stun),
        2
    );
    assert_eq!(
        engine.state.objects[&second].counter_count(CounterKind::Charge),
        2
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn ripples_unions_receipts_from_distinct_replaced_children_after_each_nested_counter_batch() {
    let mut engine = runtime_engine(810_006);
    inject_permanent_on_battlefield(&mut engine, 0, STATIC_FIXTURE_ID);
    let doublers = [
        inject_permanent_on_battlefield(&mut engine, 0, "doubling_season"),
        inject_permanent_on_battlefield(&mut engine, 0, "doubling_season"),
    ];
    let first = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let second = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let unchosen = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    for permanent in [first, second, unchosen] {
        engine
            .state
            .objects
            .get_mut(&permanent)
            .expect("counter-bearing forest")
            .set_counter(CounterKind::PlusOnePlusOne, 1);
    }
    put_poison_on_player(&mut engine, 1);

    let spell = inject_card_into_hand(&mut engine, 0, "ripples_of_potential");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "ripples_of_potential");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast shipped Ripples of Potential");
    pass_both_players_with_batch(&mut engine);
    assert_proliferate_choice(&engine, "the first replacement child starts at Proliferate");

    let first_leaf = engine
        .apply_scenario_command(0, &proliferate_choice(vec![first], vec![1]))
        .expect("select only A and the opponent's poison counter for the first leaf");
    let first_replacement = replacement_choice(&first_leaf);
    assert_eq!(
        first_replacement.choice_kind,
        ChoiceKind::ReplacementEffect as i32
    );
    assert_eq!(first_replacement.replacement_options.len(), 2);
    let mut first_sources = first_replacement
        .replacement_options
        .iter()
        .map(|option| option.source_object_id)
        .collect::<Vec<_>>();
    first_sources.sort_unstable();
    let mut expected_doublers = doublers.to_vec();
    expected_doublers.sort_unstable();
    assert_eq!(first_sources, expected_doublers);

    let first_completed = engine
        .apply_scenario_command(
            0,
            &submit_resolution_choice(vec![
                first_replacement.replacement_options[0].application_id,
            ]),
        )
        .expect("the first nested counter batch completes before the next Proliferate child");
    assert_eq!(proliferate_log_count(&first_completed, 0), 1);
    let first_counter_events = first_completed
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref()? {
            ruled_event::Ev::Log(log) => Some(log.text.as_str()),
            _ => None,
        })
        .filter(|text| text.contains("gets one of each kind of counter"))
        .count();
    assert_eq!(
        first_counter_events, 2,
        "the first leaf commits its object and player placements"
    );
    assert_eq!(
        engine.state.objects[&first].counter_count(CounterKind::PlusOnePlusOne),
        5,
        "the first leaf applies both counter doublers to A"
    );
    assert_eq!(
        engine.state.objects[&second].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "B is unchanged until the second child chooses it"
    );
    assert_eq!(
        engine.state.objects[&unchosen].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "the unchosen forest is not affected"
    );
    let opponent = engine.state.player_idx(1).expect("opponent");
    assert_eq!(
        engine.state.players[opponent].counters[&CounterKind::Poison],
        2,
        "the first child also completes the selected player's counter placement"
    );
    assert_proliferate_choice(
        &engine,
        "the second replacement child starts with a fresh offer",
    );

    let second_leaf = engine
        .apply_scenario_command(0, &proliferate_choice(vec![second], vec![]))
        .expect("select only B for the second leaf");
    let second_replacement = replacement_choice(&second_leaf);
    assert_eq!(second_replacement.replacement_options.len(), 2);
    let second_completed = engine
        .apply_scenario_command(
            0,
            &submit_resolution_choice(vec![
                second_replacement.replacement_options[0].application_id,
            ]),
        )
        .expect("the second nested counter batch completes before Ripples' Program tail");
    assert_eq!(proliferate_log_count(&second_completed, 0), 1);
    let second_counter_events = second_completed
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref()? {
            ruled_event::Ev::Log(log) => Some(log.text.as_str()),
            _ => None,
        })
        .filter(|text| text.contains("gets one of each kind of counter"))
        .count();
    assert_eq!(
        second_counter_events, 1,
        "the second leaf commits only B's placement"
    );
    assert_eq!(
        engine.state.objects[&first].counter_count(CounterKind::PlusOnePlusOne),
        5,
        "the second child does not repeat its sibling's completed placement"
    );
    assert_eq!(
        engine.state.objects[&second].counter_count(CounterKind::PlusOnePlusOne),
        5,
        "the second child commits its own two-doubler placement to B"
    );

    let tail = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the two children finish before Ripples asks for its Program result")
        .presentation
        .clone();
    assert_eq!(tail.choice_kind, ChoiceKind::PermanentObjects);
    assert_eq!((tail.min, tail.max), (0, 2));
    assert_eq!(tail.candidates, vec![first, second]);
    let tail_offer = find_resolution_choice(&second_completed)
        .expect("Ripples' Program tail is published after both leaves");
    assert_eq!(tail_offer.choice_kind, ChoiceKind::PermanentObjects as i32);
    assert_eq!(tail_offer.candidate_object_ids, vec![first, second]);
    assert!(tail_offer.candidate_player_ids.is_empty());
    assert!(!tail_offer.candidate_object_ids.contains(&unchosen));

    let completed = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![first, second]))
        .expect("choose both independently receipted permanents in Ripples' original Program");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert!(engine.state.objects[&first].phase_status.is_some());
    assert!(engine.state.objects[&second].phase_status.is_some());
    assert!(engine.state.objects[&unchosen].phase_status.is_none());
    let resolution_logs = completed
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref()? {
            ruled_event::Ev::Log(log) => Some(&log.text),
            _ => None,
        })
        .filter(|text| text.as_str() == "Ripples of Potential resolves.")
        .count();
    let stack_resolutions = completed
        .events
        .iter()
        .filter(|event| {
            matches!(
                &event.ev,
                Some(ruled_event::Ev::StackResolved(resolved)) if resolved.object_id == spell
            )
        })
        .count();
    assert_eq!(resolution_logs, 1);
    assert_eq!(
        stack_resolutions, 1,
        "the original Program resumes exactly once"
    );
}

#[test]
fn outer_replacement_order_refreshes_after_actor_and_sources_leave() {
    let mut engine = runtime_four_player_engine(810_007);
    inject_permanent_on_battlefield(&mut engine, 0, STATIC_FIXTURE_ID);
    inject_permanent_on_battlefield(&mut engine, 0, STATIC_FIXTURE_ID);
    put_poison_on_player(&mut engine, 2);
    let spell = inject_card_into_hand(&mut engine, 0, "ripples_of_potential");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "ripples_of_potential");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Ripples before its Program Proliferate starts resolving");
    let mut outer = RuledEventBatch::default();
    let passes = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count();
    for _ in 0..passes {
        let priority = engine.state.priority_player_id();
        outer = engine
            .apply_scenario_command(priority, &pass())
            .expect("every live player passes before Ripples resolves");
    }
    let initial_order = replacement_choice(&outer);
    assert_eq!(initial_order.deciding_player_id, 0);
    assert_eq!(initial_order.replacement_options.len(), 2);
    assert!(engine.state.pending_resolution.is_some());

    let departure = engine
        .apply_scenario_command(0, &concede())
        .expect("the begun replacement transaction survives its actor's concession");
    assert!(engine.state.players[engine.state.player_idx(0).unwrap()].has_lost);
    let selector = find_resolution_choice(&departure)
        .expect("P1 gets a delegate selector after P0 and their source objects leave");
    assert_eq!(selector.choice_kind, ChoiceKind::ResolutionBranch as i32);
    assert_eq!(selector.deciding_player_id, 1);
    assert_ne!(selector.resolution_prompt_id, 0);
    assert_eq!(selector.resolution_branches.len(), 3);

    let delegated = engine
        .apply_scenario_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: selector.resolution_prompt_id,
                    decision: ResolutionChoiceDecision::SelectBranch as i32,
                    selected_branch_index: 1,
                    ..Default::default()
                })),
            },
        )
        .expect("P1 delegates the remaining Proliferate choice to P2");
    let recipient_choice = find_resolution_choice(&delegated)
        .expect("P2 receives the original action's fresh Proliferate recipient offer");
    assert_eq!(recipient_choice.choice_kind, ChoiceKind::Proliferate as i32);
    assert_eq!(recipient_choice.deciding_player_id, 2);
    assert_eq!(recipient_choice.candidate_player_ids, vec![2]);
    assert!(recipient_choice.candidate_object_ids.is_empty());
    let player2_index = engine.state.player_idx(2).expect("delegated P2");
    assert_eq!(
        engine.state.players[player2_index]
            .counters
            .get(&CounterKind::Poison)
            .copied(),
        Some(1),
        "P2's poison counter still exists when the delegated recipient choice is offered"
    );

    let completed = engine
        .apply_scenario_command(2, &proliferate_choice(vec![], vec![2]))
        .expect("P2 chooses their poison counter for the continued original Program");
    assert_eq!(proliferate_log_count(&completed, 0), 1);
    let player2_logs = completed
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref()? {
            ruled_event::Ev::Log(log) => Some(log.text.as_str()),
            _ => None,
        })
        .filter(|text| text.contains("P2 gets one of each kind of counter"))
        .count();
    let poison_after = engine.state.players[player2_index]
        .counters
        .get(&CounterKind::Poison)
        .copied()
        .unwrap_or(0);
    assert_eq!(
        poison_after, 2,
        "P2's poison counter is doubled by the continued original action"
    );
    assert_eq!(player2_logs, 1);
    let stack_resolutions = completed
        .events
        .iter()
        .filter(|event| matches!(&event.ev, Some(ruled_event::Ev::StackResolved(resolved)) if resolved.object_id == spell))
        .count();
    assert_eq!(
        stack_resolutions, 0,
        "the conceded owner's removed spell is not rebound to a later stack exit"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    // CR 800.4a removes cards owned by a player who leaves the game.
    assert!(!engine.state.objects.contains_key(&spell));
}
#[test]
fn flat_gambit_outer_replacement_order_refreshes_after_actor_and_sources_leave() {
    let mut engine = runtime_four_player_engine(810_008);
    inject_permanent_on_battlefield(&mut engine, 0, STATIC_FIXTURE_ID);
    inject_permanent_on_battlefield(&mut engine, 0, STATIC_FIXTURE_ID);
    put_poison_on_player(&mut engine, 2);
    let spell = inject_card_into_hand(&mut engine, 0, "tezzerets_gambit");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "tezzerets_gambit");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast shipped Tezzeret's Gambit before its flat Proliferate starts resolving");
    let mut outer = RuledEventBatch::default();
    let passes = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count();
    for _ in 0..passes {
        let priority = engine.state.priority_player_id();
        outer = engine
            .apply_scenario_command(priority, &pass())
            .expect("every live player passes before Tezzeret's Gambit resolves");
    }
    let initial_order = replacement_choice(&outer);
    assert_eq!(initial_order.deciding_player_id, 0);
    assert_eq!(initial_order.replacement_options.len(), 2);
    assert!(engine.state.pending_resolution.is_some());

    let departure = engine
        .apply_scenario_command(0, &concede())
        .expect("the begun replacement transaction survives its actor's concession");
    assert!(engine.state.players[engine.state.player_idx(0).unwrap()].has_lost);
    let selector = find_resolution_choice(&departure)
        .expect("P1 gets a delegate selector after P0 and their source objects leave");
    assert_eq!(selector.choice_kind, ChoiceKind::ResolutionBranch as i32);
    assert_eq!(selector.deciding_player_id, 1);
    assert_ne!(selector.resolution_prompt_id, 0);
    assert_eq!(selector.resolution_branches.len(), 3);

    let delegated = engine
        .apply_scenario_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: selector.resolution_prompt_id,
                    decision: ResolutionChoiceDecision::SelectBranch as i32,
                    selected_branch_index: 1,
                    ..Default::default()
                })),
            },
        )
        .expect("P1 delegates the remaining Proliferate choice to P2");
    let recipient_choice = find_resolution_choice(&delegated)
        .expect("P2 receives the original action's fresh Proliferate recipient offer");
    assert_eq!(recipient_choice.choice_kind, ChoiceKind::Proliferate as i32);
    assert_eq!(recipient_choice.deciding_player_id, 2);
    assert_eq!(recipient_choice.candidate_player_ids, vec![2]);
    assert_ne!(recipient_choice.resolution_prompt_id, 0);
    // Prompt handles are actor-scoped, so use the fresh event even if its number repeats.
    assert!(recipient_choice.candidate_object_ids.is_empty());
    let player2_index = engine.state.player_idx(2).expect("delegated P2");
    assert_eq!(
        engine.state.players[player2_index]
            .counters
            .get(&CounterKind::Poison)
            .copied(),
        Some(1),
        "P2's poison counter still exists when the delegated recipient choice is offered"
    );

    let completed = engine
        .apply_scenario_command(
            2,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: recipient_choice.resolution_prompt_id,
                    chosen_player_ids: vec![2],
                    decision: ResolutionChoiceDecision::Unspecified as i32,
                    ..Default::default()
                })),
            },
        )
        .expect("P2 chooses the poison counter for the original flat Gambit action");
    assert_eq!(proliferate_log_count(&completed, 0), 1);
    let player2_logs = completed
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref()? {
            ruled_event::Ev::Log(log) => Some(log.text.as_str()),
            _ => None,
        })
        .filter(|text| text.contains("P2 gets one of each kind of counter"))
        .count();
    let poison_after = engine.state.players[player2_index]
        .counters
        .get(&CounterKind::Poison)
        .copied()
        .unwrap_or(0);
    assert_eq!(
        poison_after, 2,
        "P2's poison counter is doubled by the continued original action"
    );
    assert_eq!(player2_logs, 1);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    // CR 800.4a removes cards owned by a player who leaves the game.
    assert!(!engine.state.objects.contains_key(&spell));
}

#[test]
fn flat_gambit_empty_recipient_refresh_finishes_both_leaves_and_triggers_ezuri_twice() {
    let mut engine = runtime_four_player_engine(810_009);
    inject_permanent_on_battlefield(&mut engine, 0, STATIC_FIXTURE_ID);
    // Direct battlefield injection prepares the observer without its enter-the-battlefield trigger.
    let ezuri = inject_creature_on_battlefield(&mut engine, 0, "ezuri,_stalker_of_spheres");
    put_poison_on_player(&mut engine, 1);
    let gambit = inject_card_into_hand(&mut engine, 0, "tezzerets_gambit");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "tezzerets_gambit");

    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("P0 casts shipped Tezzeret's Gambit");
    let first_offer = pass_live_players_with_batch(&mut engine);
    let first_choice = find_resolution_choice(&first_offer)
        .expect("the flat Gambit first leaf offers its eligible Proliferate recipients");
    assert_eq!(first_choice.choice_kind, ChoiceKind::Proliferate as i32);
    assert_eq!(first_choice.deciding_player_id, 0);
    assert_eq!(first_choice.candidate_object_ids, Vec::<u32>::new());
    assert_eq!(first_choice.candidate_player_ids, vec![1]);
    let hand_at_first_prompt = engine.state.players[engine.state.player_idx(0).unwrap()]
        .hand
        .len();

    let departure = engine
        .apply_scenario_command(1, &concede())
        .expect("P1 concedes while the first Gambit recipient choice is parked");
    assert!(engine.state.players[engine.state.player_idx(1).unwrap()].has_lost);
    assert!(!engine.state.players[engine.state.player_idx(0).unwrap()].has_lost);
    assert_eq!(engine.state.objects[&gambit].zone, Zone::Graveyard);
    assert!(engine.state.pending_resolution.is_none());
    let parent_resolutions = departure
        .events
        .iter()
        .filter(|event| matches!(event.ev.as_ref(), Some(ruled_event::Ev::StackResolved(resolved)) if resolved.object_id == gambit))
        .count();
    assert_eq!(
        parent_resolutions, 1,
        "the original Gambit finishes exactly once"
    );

    assert!(departure.events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(ruled_event::Ev::TriggerOrderRequired(_))
    )));
    let trigger_order = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("both Proliferated events wait for Ezuri trigger ordering");
    assert_eq!(trigger_order.deciding_player, 0);
    assert_eq!(trigger_order.candidates.len(), 2);
    assert!(trigger_order.candidates.iter().all(|trigger| {
        trigger.source_permanent_id == ezuri
            && trigger.controller == 0
            && trigger.card_id == "ezuri,_stalker_of_spheres"
            && trigger.ability_index == 1
    }));
    assert!(answer_trigger_order_in_engine_order(&mut engine));
    assert!(engine.state.pending_trigger_order.is_none());
    let observer_triggers = engine
        .state
        .stack
        .iter()
        .filter(|item| {
            item.is_triggered
                && item.source_permanent_id == Some(ezuri)
                && item.ability_index == Some(1)
        })
        .count();
    assert_eq!(
        observer_triggers, 2,
        "both Ezuri triggered_02 abilities are stacked"
    );
    assert!(engine.state.stack.iter().all(|item| {
        item.is_triggered
            && item.source_permanent_id == Some(ezuri)
            && item.ability_index == Some(1)
    }));

    while !engine.state.stack.is_empty() {
        pass_live_players_with_batch(&mut engine);
    }
    let hand_after_observer_triggers = engine.state.players[engine.state.player_idx(0).unwrap()]
        .hand
        .len();
    assert_eq!(
        hand_after_observer_triggers,
        hand_at_first_prompt + 2,
        "the two surviving Ezuri triggers each draw one card after Gambit resolves"
    );
}

#[test]
fn copied_effective_face_acquires_and_loses_proliferate_doubling() {
    let mut engine = runtime_engine(810_101);
    let host = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let recipient = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    engine
        .state
        .objects
        .get_mut(&recipient)
        .expect("counter-bearing Forest recipient")
        .set_counter(CounterKind::Charge, 1);
    let bastion = inject_permanent_on_battlefield(&mut engine, 0, "karns_bastion");
    let host_generation = engine
        .state
        .zone_change_generation
        .get(&host)
        .copied()
        .unwrap_or(0);
    let donor = runtime_registry()
        .get(STATIC_FIXTURE_ID)
        .expect("the local validated static donor is in the fixture registry");
    let donor_face = donor.primary_face().clone();
    let donor_face_id = donor_face.face_id.clone();
    let donor_name = donor_face.name.clone();
    {
        let host_object = engine
            .state
            .objects
            .get_mut(&host)
            .expect("physical Forest host");
        assert_eq!(host_object.card_id, "forest");
        host_object.copiable_values = Some(CopiableValues {
            source_card_id: STATIC_FIXTURE_ID.into(),
            source_face_index: 0,
            face: donor_face,
            prototyped: false,
            room_faces: None,
            display_name: donor_name,
        });
        host_object.copy_revision += 1;
        assert!(host_object.active_copy_occurrence.is_none());
    }
    let first_snapshot = engine.state.objects[&host]
        .copiable_values
        .as_ref()
        .expect("prepared owned copied face");
    assert_eq!(first_snapshot.source_card_id, STATIC_FIXTURE_ID);
    assert_eq!(first_snapshot.source_face_index, 0);
    assert_eq!(first_snapshot.face.face_id, donor_face_id);
    assert_eq!(engine.state.objects[&host].copy_revision, 1);

    assert_eq!(
        resolve_bastion_action(&mut engine, bastion, recipient, 2, None),
        2
    );
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        3
    );
    assert_eq!(engine.state.objects[&host].card_id, "forest");
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&host)
            .copied()
            .unwrap_or(0),
        host_generation
    );

    {
        let host_object = engine
            .state
            .objects
            .get_mut(&host)
            .expect("same physical Forest host");
        host_object.copiable_values = None;
        host_object.copy_revision += 1;
        assert!(host_object.active_copy_occurrence.is_none());
    }
    assert!(engine.state.objects[&host].copiable_values.is_none());
    assert_eq!(engine.state.objects[&host].copy_revision, 2);
    assert!(engine.state.objects[&bastion].tapped);
    engine
        .state
        .objects
        .get_mut(&bastion)
        .expect("same Bastion for prepared second activation")
        .tapped = false;
    assert_eq!(
        resolve_bastion_action(&mut engine, bastion, recipient, 1, None),
        1
    );
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        4
    );
    assert_eq!(engine.state.objects[&host].card_id, "forest");
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&host)
            .copied()
            .unwrap_or(0),
        host_generation
    );
}

#[test]
fn room_door_occurrences_with_shared_local_ability_id_create_four_leaves() {
    let room_ron = local_room_fixture_ron();
    let registry = runtime_registry_with_local_fixtures(&[room_ron.as_str()]);
    let room_definition = registry
        .get(ROOM_FIXTURE_ID)
        .expect("validated typed two-door Room fixture");
    assert_eq!(room_definition.faces.len(), 2);
    assert_ne!(
        room_definition.faces[0].face_id,
        room_definition.faces[1].face_id
    );
    assert_eq!(
        room_definition.faces[0].static_abilities[0]
            .ability_id
            .to_string(),
        "door_static"
    );
    assert_eq!(
        room_definition.faces[1].static_abilities[0]
            .ability_id
            .to_string(),
        "door_static"
    );

    for (case, unlocked, expected_leaves, expected_charge, seed) in [
        ("left door", [true, false], 2, 3, 810_102),
        ("right door", [false, true], 2, 3, 810_103),
        ("both native doors", [true, true], 4, 5, 810_104),
    ] {
        let mut engine = runtime_engine_with_registry(seed, registry);
        let source = inject_permanent_on_battlefield(&mut engine, 0, ROOM_FIXTURE_ID);
        let recipient = inject_permanent_on_battlefield(&mut engine, 0, "forest");
        engine
            .state
            .objects
            .get_mut(&recipient)
            .expect("counter-bearing Forest recipient")
            .set_counter(CounterKind::Charge, 1);
        let bastion = inject_permanent_on_battlefield(&mut engine, 0, "karns_bastion");
        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        engine
            .state
            .room_states
            .insert(source, RoomState { unlocked });
        assert_eq!(engine.state.objects[&source].card_id, ROOM_FIXTURE_ID);
        assert_eq!(engine.state.room_states[&source].unlocked, unlocked);

        let expected_order_source = if unlocked == [true, true] {
            Some((source, generation))
        } else {
            None
        };
        assert_eq!(
            resolve_bastion_action(
                &mut engine,
                bastion,
                recipient,
                expected_leaves,
                expected_order_source,
            ),
            expected_leaves,
            "{case} completes the independently expected leaves"
        );
        assert_eq!(
            engine.state.objects[&recipient].counter_count(CounterKind::Charge),
            expected_charge,
            "{case} adds one Charge counter per completed leaf"
        );
        assert_eq!(engine.state.objects[&source].card_id, ROOM_FIXTURE_ID);
        assert_eq!(
            engine
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            generation
        );
    }

    let donor = registry
        .get(ROOM_FIXTURE_ID)
        .expect("same validated Room donor definition");
    let selected_donor_face = donor.face(1).expect("the second Room door").clone();
    let expected_room_face_ids = donor
        .faces
        .iter()
        .map(|face| face.face_id.clone())
        .collect::<Vec<_>>();
    let mut engine = runtime_engine_with_registry(810_105, registry);
    let host = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let recipient = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    engine
        .state
        .objects
        .get_mut(&recipient)
        .expect("counter-bearing Forest recipient")
        .set_counter(CounterKind::Charge, 1);
    let bastion = inject_permanent_on_battlefield(&mut engine, 0, "karns_bastion");
    let host_generation = engine
        .state
        .zone_change_generation
        .get(&host)
        .copied()
        .unwrap_or(0);
    {
        let host_object = engine
            .state
            .objects
            .get_mut(&host)
            .expect("physical Forest host");
        assert_eq!(host_object.card_id, "forest");
        host_object.copiable_values = Some(CopiableValues {
            source_card_id: donor.id.clone(),
            source_face_index: 1,
            face: selected_donor_face.clone(),
            prototyped: false,
            room_faces: Some(donor.faces.clone()),
            display_name: donor.name.clone(),
        });
        host_object.copy_revision += 1;
        assert!(host_object.active_copy_occurrence.is_none());
    }
    engine.state.room_states.insert(
        host,
        RoomState {
            unlocked: [true, true],
        },
    );
    let copied_values = engine.state.objects[&host]
        .copiable_values
        .as_ref()
        .expect("prepared owned copied Room face");
    assert_eq!(copied_values.source_card_id, ROOM_FIXTURE_ID);
    assert_eq!(copied_values.source_face_index, 1);
    assert_eq!(copied_values.face.face_id, selected_donor_face.face_id);
    assert_eq!(
        copied_values
            .room_faces
            .as_ref()
            .expect("the copied Room owns both faces")
            .iter()
            .map(|face| face.face_id.clone())
            .collect::<Vec<_>>(),
        expected_room_face_ids
    );
    assert_eq!(engine.state.objects[&host].copy_revision, 1);
    assert!(engine.state.objects[&host].active_copy_occurrence.is_none());
    assert_eq!(engine.state.room_states[&host].unlocked, [true, true]);
    assert_eq!(engine.state.objects[&host].card_id, "forest");
    assert!(
        !engine.state.objects.values().any(|object| {
            object.zone == Zone::Battlefield && object.card_id == ROOM_FIXTURE_ID
        }),
        "the copied Room has no live additional donor permanent"
    );

    assert_eq!(
        resolve_bastion_action(
            &mut engine,
            bastion,
            recipient,
            4,
            Some((host, host_generation)),
        ),
        4
    );
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Charge),
        5
    );
    assert_eq!(engine.state.objects[&host].card_id, "forest");
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&host)
            .copied()
            .unwrap_or(0),
        host_generation
    );
}

#[test]
fn class_level_bars_activate_only_current_generation_distinct_occurrences() {
    let class_ron = local_class_fixture_ron();
    let registry = runtime_registry_with_local_fixtures(&[class_ron.as_str()]);
    let class_definition = registry
        .get(CLASS_FIXTURE_ID)
        .expect("validated typed local Class fixture");
    let class_bars = &class_definition.primary_face().class_level_bars;
    assert_eq!(
        class_bars.iter().map(|bar| bar.level).collect::<Vec<_>>(),
        vec![2, 3]
    );
    assert_eq!(
        class_bars[0].static_abilities[0].ability_id.to_string(),
        "class_two_static"
    );
    assert_eq!(
        class_bars[1].static_abilities[0].ability_id.to_string(),
        "class_three_static"
    );

    for (designation, effective_level, expected_leaves, expected_charge, seed) in [
        (None, 1, 1, 2, 810_106),
        (Some(2), 2, 2, 3, 810_107),
        (Some(3), 3, 4, 5, 810_108),
    ] {
        let mut engine = runtime_engine_with_registry(seed, registry);
        let source = inject_permanent_on_battlefield(&mut engine, 0, CLASS_FIXTURE_ID);
        let recipient = inject_permanent_on_battlefield(&mut engine, 0, "forest");
        engine
            .state
            .objects
            .get_mut(&recipient)
            .expect("counter-bearing Forest recipient")
            .set_counter(CounterKind::Charge, 1);
        let bastion = inject_permanent_on_battlefield(&mut engine, 0, "karns_bastion");
        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        if let Some(level) = designation {
            engine.state.class_levels.insert(
                source,
                ClassLevelDesignation {
                    zone_change_generation: generation,
                    level,
                },
            );
        }
        assert_eq!(engine.state.class_level(source), effective_level);

        let expected_order_source = if designation == Some(3) {
            Some((source, generation))
        } else {
            None
        };
        assert_eq!(
            resolve_bastion_action(
                &mut engine,
                bastion,
                recipient,
                expected_leaves,
                expected_order_source,
            ),
            expected_leaves
        );
        assert_eq!(
            engine.state.objects[&recipient].counter_count(CounterKind::Charge),
            expected_charge
        );
        assert_eq!(engine.state.class_level(source), effective_level);
        assert_eq!(engine.state.objects[&source].card_id, CLASS_FIXTURE_ID);
        assert_eq!(
            engine
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            generation
        );
    }
}

#[test]
fn doubled_proliferate_counter_batches_keep_affected_chooser_apnap_and_reject_stale_answers() {
    #[derive(Clone)]
    struct RecordedCommand {
        actor: i32,
        command: RuledCommand,
        accepted: bool,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct StackIdentity {
        stack_id: u32,
        source_permanent_id: Option<u32>,
        source_zone_change: u64,
        ability_index: Option<usize>,
        controller: i32,
    }

    type RecordedResolutionOffer = (u64, i32, i32, Vec<u32>, Vec<i32>, Vec<u32>);

    #[derive(Debug, Default, PartialEq, Eq)]
    struct ReplayTrace {
        statuses: Vec<bool>,
        rejections: Vec<String>,
        event_batches: Vec<String>,
        offers: Vec<RecordedResolutionOffer>,
        p0_proliferate_logs: usize,
        stack_resolved_object_ids: Vec<u32>,
        original_stack_identity: Option<StackIdentity>,
        diagnostics: Vec<String>,
    }

    fn record_batch(batch: &RuledEventBatch, trace: &mut ReplayTrace) {
        trace.event_batches.push(format!("{:?}", batch.events));
        for event in &batch.events {
            match event.ev.as_ref() {
                Some(ruled_event::Ev::ResolutionChoiceRequired(choice)) => {
                    trace.offers.push((
                        choice.resolution_prompt_id,
                        choice.choice_kind,
                        choice.deciding_player_id,
                        choice.candidate_object_ids.clone(),
                        choice.candidate_player_ids.clone(),
                        choice
                            .replacement_options
                            .iter()
                            .map(|option| option.application_id)
                            .collect(),
                    ));
                }
                Some(ruled_event::Ev::Log(log)) if log.text == "P0 proliferates." => {
                    trace.p0_proliferate_logs += 1;
                }
                Some(ruled_event::Ev::StackResolved(resolved)) => {
                    trace.stack_resolved_object_ids.push(resolved.object_id);
                }
                _ => {}
            }
        }
    }

    fn apply_recorded(
        engine: &mut GameEngine,
        actor: i32,
        command: RuledCommand,
        expected_accepted: bool,
        commands: Option<&mut Vec<RecordedCommand>>,
        trace: &mut ReplayTrace,
    ) -> Option<RuledEventBatch> {
        let diagnostic_before = engine.diagnostic_snapshot().unwrap();
        let pending_before = format!("{:?}", engine.state.pending_resolution);
        let result = engine.apply_scenario_command(actor, &command);
        let accepted = result.is_ok();
        assert_eq!(accepted, expected_accepted, "command actor P{actor}");
        if let Some(commands) = commands {
            commands.push(RecordedCommand {
                actor,
                command: command.clone(),
                accepted,
            });
        }
        trace.statuses.push(accepted);
        let batch = match result {
            Ok(batch) => {
                if let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_ref() {
                    let item = engine
                        .state
                        .stack
                        .last()
                        .expect("accepted Bastion activation is on the stack");
                    assert_eq!(item.source_permanent_id, Some(activation.source_object_id));
                    assert_eq!(item.controller, actor);
                    assert_eq!(item.ability_index, Some(activation.ability_index as usize));
                    trace.original_stack_identity = Some(StackIdentity {
                        stack_id: item.id,
                        source_permanent_id: item.source_permanent_id,
                        source_zone_change: item.source_zone_change,
                        ability_index: item.ability_index,
                        controller: item.controller,
                    });
                }
                record_batch(&batch, trace);
                Some(batch)
            }
            Err(error) => {
                trace.rejections.push(format!("{error:?}"));
                assert_eq!(
                    engine.diagnostic_snapshot().unwrap(),
                    diagnostic_before,
                    "rejected choice leaves diagnostic state unchanged"
                );
                assert_eq!(
                    format!("{:?}", engine.state.pending_resolution),
                    pending_before,
                    "rejected choice leaves the complete pending offer unchanged"
                );
                None
            }
        };
        trace
            .diagnostics
            .push(engine.diagnostic_snapshot().unwrap().to_string());
        batch
    }

    fn answer_for(prompt: &ResolutionChoiceRequired, chosen: Vec<u32>) -> RuledCommand {
        assert_ne!(
            prompt.resolution_prompt_id, 0,
            "use the published prompt ID"
        );
        RuledCommand {
            cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                resolution_prompt_id: prompt.resolution_prompt_id,
                chosen_object_ids: chosen,
                decision: ResolutionChoiceDecision::Unspecified as i32,
                ..Default::default()
            })),
        }
    }

    fn assert_active_counter_recipient(engine: &GameEngine, expected_object: u32) {
        let snapshot = engine.diagnostic_snapshot().unwrap();
        let pending_batch = &snapshot["state"]["pending_replacement_event"];
        assert_eq!(pending_batch["variant"], "Counters");
        let value = &pending_batch["value"];
        let events = value["events"]
            .as_array()
            .expect("the suspended counter batch retains every selected recipient");
        let event_index = value["event_index"]
            .as_str()
            .expect("the active counter event index is retained")
            .parse::<usize>()
            .unwrap();
        let active = events
            .get(event_index)
            .expect("the active counter event index points to a pending proposal");
        assert_eq!(active["recipient"]["variant"], "Object");
        assert_eq!(
            active["recipient"]["value"]["object_id"].as_u64(),
            Some(expected_object as u64),
            "the published order prompt belongs to the active recipient event"
        );
        assert_eq!(active["actor"].as_i64(), Some(0));
    }

    fn assert_counter_order_offer(
        engine: &GameEngine,
        offer: &ResolutionChoiceRequired,
        chooser: i32,
        recipient: u32,
        expected_sources: &[u32],
    ) {
        assert_eq!(offer.choice_kind, ChoiceKind::ReplacementEffect as i32);
        assert_eq!(offer.deciding_player_id, chooser);
        assert_ne!(offer.resolution_prompt_id, 0);
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the replacement order remains pending");
        assert_eq!(pending.deciding_player, chooser);
        assert_eq!(
            pending.presentation.choice_kind,
            ChoiceKind::ReplacementEffect
        );
        assert_eq!(
            pending.resolution_prompt_id.get(),
            offer.resolution_prompt_id
        );
        assert!(matches!(
            &pending.continuation,
            ResolutionContinuation::CounterReplacement { .. }
        ));
        assert_active_counter_recipient(engine, recipient);
        let mut actual_sources = offer
            .replacement_options
            .iter()
            .map(|option| option.source_object_id)
            .collect::<Vec<_>>();
        actual_sources.sort_unstable();
        let mut expected_sources = expected_sources.to_vec();
        expected_sources.sort_unstable();
        assert_eq!(actual_sources, expected_sources);
        assert!(offer
            .replacement_options
            .iter()
            .all(|option| option.application_id != 0));
    }

    fn prepare(seed: u64) -> (GameEngine, u32, [u32; 2], u32, [u32; 2], u32) {
        let mut engine = runtime_four_player_engine(seed);

        // Allocate B and its sources first: B has the smaller ObjectId, but APNAP must choose A first.
        let b = inject_permanent_on_battlefield(&mut engine, 3, "forest");
        let b_doublers = [
            inject_permanent_on_battlefield(&mut engine, 3, "doubling_season"),
            inject_permanent_on_battlefield(&mut engine, 3, "doubling_season"),
        ];
        let a = inject_permanent_on_battlefield(&mut engine, 1, "forest");
        let a_doublers = [
            inject_permanent_on_battlefield(&mut engine, 1, "doubling_season"),
            inject_permanent_on_battlefield(&mut engine, 1, "doubling_season"),
        ];
        assert!(b < a, "B must have the smaller physical ObjectId");
        engine
            .state
            .objects
            .get_mut(&b)
            .expect("B is on the battlefield")
            .set_counter(CounterKind::Charge, 1);
        engine
            .state
            .objects
            .get_mut(&a)
            .expect("A is on the battlefield")
            .set_counter(CounterKind::Charge, 1);

        // Keep ownership at P3 while temporarily controlling B and its doublers with P2.
        let first_control_timestamp = engine.state.command_index + 1;
        for (offset, object_id) in std::iter::once(b).chain(b_doublers).enumerate() {
            engine.state.continuous_effects.push(ContinuousEffect {
                source_id: None,
                trigger_grant_origin: None,
                affected: AffectedScope::Single(object_id),
                kind: ContinuousEffectKind::Layer2Control {
                    controller: ControllerReference::Fixed(2),
                },
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: first_control_timestamp + offset as u64,
            });
        }
        assert_eq!(engine.state.objects[&b].owner, 3);
        assert_eq!(engine.characteristics(b).unwrap().controller, 2);
        assert!(b_doublers.iter().all(|object_id| {
            engine.state.objects[object_id].owner == 3
                && engine.characteristics(*object_id).unwrap().controller == 2
        }));
        assert_eq!(engine.state.objects[&a].owner, 1);
        assert_eq!(engine.characteristics(a).unwrap().controller, 1);
        assert!(a_doublers.iter().all(|object_id| {
            engine.state.objects[object_id].owner == 1
                && engine.characteristics(*object_id).unwrap().controller == 1
        }));

        inject_permanent_on_battlefield(&mut engine, 0, STATIC_FIXTURE_ID);
        let bastion = inject_permanent_on_battlefield(&mut engine, 0, "karns_bastion");
        grant_pool(&mut engine, 0);
        (engine, b, b_doublers, a, a_doublers, bastion)
    }

    const SEED: u64 = 810_110;
    let (mut engine, b, b_doublers, a, a_doublers, bastion) = prepare(SEED);
    let mut commands = Vec::new();
    let mut trace = ReplayTrace::default();

    let activation = activate_ability_for(&engine, bastion, 1, vec![]);
    apply_recorded(
        &mut engine,
        0,
        activation,
        true,
        Some(&mut commands),
        &mut trace,
    )
    .expect("activate actual Karn's Bastion once");
    let original_stack_identity = trace
        .original_stack_identity
        .expect("the actual Bastion activation stack identity is captured");
    assert_eq!(original_stack_identity.source_permanent_id, Some(bastion));
    assert_eq!(original_stack_identity.source_zone_change, 0);
    assert_eq!(original_stack_identity.ability_index, Some(1));
    assert_eq!(original_stack_identity.controller, 0);
    let p0 = engine
        .state
        .player_idx(0)
        .expect("active proliferating player");
    let mana_after_activation = engine.state.players[p0].mana_pool;
    assert!(engine.state.objects[&bastion].tapped);

    let live_players = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count();
    let mut resolved = None;
    for _ in 0..live_players {
        let actor = engine.state.priority_player_id();
        resolved = apply_recorded(
            &mut engine,
            actor,
            pass(),
            true,
            Some(&mut commands),
            &mut trace,
        );
    }
    let first_leaf_offer = find_resolution_choice(&resolved.unwrap())
        .expect("the first actual Bastion Proliferate leaf asks P0 for recipients");
    assert_eq!(first_leaf_offer.choice_kind, ChoiceKind::Proliferate as i32);
    assert_eq!(first_leaf_offer.deciding_player_id, 0);
    assert_ne!(first_leaf_offer.resolution_prompt_id, 0);
    let mut recipient_candidates = first_leaf_offer.candidate_object_ids.clone();
    recipient_candidates.sort_unstable();
    assert_eq!(recipient_candidates, vec![b, a]);

    // Submit [B, A] despite B's smaller ObjectId; P1's affected A order must be offered first.
    let first_order_batch = apply_recorded(
        &mut engine,
        0,
        answer_for(&first_leaf_offer, vec![b, a]),
        true,
        Some(&mut commands),
        &mut trace,
    )
    .expect("P0 selects B then A for the first proliferated leaf");
    let first_p1_order = replacement_choice(&first_order_batch);
    assert_counter_order_offer(&engine, &first_p1_order, 1, a, &a_doublers);
    let first_p1_prompt_id = first_p1_order.resolution_prompt_id;
    let first_p1_handle = first_p1_order.replacement_options[0].application_id;

    apply_recorded(
        &mut engine,
        0,
        answer_for(&first_p1_order, vec![first_p1_handle]),
        false,
        Some(&mut commands),
        &mut trace,
    );
    apply_recorded(
        &mut engine,
        1,
        answer_for(&first_p1_order, vec![u32::MAX]),
        false,
        Some(&mut commands),
        &mut trace,
    );
    apply_recorded(
        &mut engine,
        1,
        answer_for(&first_p1_order, vec![first_p1_handle, first_p1_handle]),
        false,
        Some(&mut commands),
        &mut trace,
    );

    let second_order_batch = apply_recorded(
        &mut engine,
        1,
        answer_for(&first_p1_order, vec![first_p1_handle]),
        true,
        Some(&mut commands),
        &mut trace,
    )
    .expect("P1 chooses one offered replacement for A");
    let p2_order = replacement_choice(&second_order_batch);
    assert_counter_order_offer(&engine, &p2_order, 2, b, &b_doublers);
    assert_eq!(
        engine.state.objects[&a].counter_count(CounterKind::Charge),
        1,
        "A remains uncommitted while B's affected chooser still has to order replacements"
    );
    assert_eq!(
        engine.state.objects[&b].counter_count(CounterKind::Charge),
        1,
        "B remains uncommitted while its replacement order is pending"
    );
    let old_p2_prompt_id = p2_order.resolution_prompt_id;
    let old_p2_handle = p2_order.replacement_options[0].application_id;

    let p2_departure = apply_recorded(
        &mut engine,
        2,
        concede(),
        true,
        Some(&mut commands),
        &mut trace,
    )
    .expect("P2 concedes while the B counter-order prompt is pending");
    assert!(engine.state.players[engine.state.player_idx(2).unwrap()].has_lost);
    assert_eq!(engine.state.objects[&b].owner, 3);
    assert_eq!(engine.characteristics(b).unwrap().controller, 3);
    assert!(b_doublers.iter().all(|object_id| {
        engine.state.objects[object_id].owner == 3
            && engine.characteristics(*object_id).unwrap().controller == 3
    }));
    let p3_order = replacement_choice(&p2_departure);
    assert_counter_order_offer(&engine, &p3_order, 3, b, &b_doublers);
    assert_eq!(
        engine.state.objects[&a].counter_count(CounterKind::Charge),
        1,
        "the whole placement batch remains pending after P2 leaves"
    );
    assert_eq!(
        engine.state.objects[&b].counter_count(CounterKind::Charge),
        1
    );

    apply_recorded(
        &mut engine,
        2,
        RuledCommand {
            cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                resolution_prompt_id: old_p2_prompt_id,
                chosen_object_ids: vec![old_p2_handle],
                decision: ResolutionChoiceDecision::Unspecified as i32,
                ..Default::default()
            })),
        },
        false,
        Some(&mut commands),
        &mut trace,
    );

    let first_leaf_completed = apply_recorded(
        &mut engine,
        3,
        answer_for(
            &p3_order,
            vec![p3_order.replacement_options[0].application_id],
        ),
        true,
        Some(&mut commands),
        &mut trace,
    )
    .expect("P3 orders B and the entire first counter batch commits once");
    assert_eq!(proliferate_log_count(&first_leaf_completed, 0), 1);
    assert_eq!(
        engine.state.objects[&a].counter_count(CounterKind::Charge),
        5
    );
    assert_eq!(
        engine.state.objects[&b].counter_count(CounterKind::Charge),
        5
    );
    let second_leaf_offer = find_resolution_choice(&first_leaf_completed)
        .expect("the fresh second doubled leaf returns to P0");
    assert_eq!(
        second_leaf_offer.choice_kind,
        ChoiceKind::Proliferate as i32
    );
    assert_eq!(second_leaf_offer.deciding_player_id, 0);
    assert_ne!(
        second_leaf_offer.resolution_prompt_id,
        first_leaf_offer.resolution_prompt_id
    );
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        0
    );
    assert!(engine.state.objects[&bastion].tapped);
    assert_eq!(engine.state.players[p0].mana_pool, mana_after_activation);

    let second_leaf_batch = apply_recorded(
        &mut engine,
        0,
        answer_for(&second_leaf_offer, vec![a]),
        true,
        Some(&mut commands),
        &mut trace,
    )
    .expect("P0 selects only A for the fresh second leaf");
    let second_p1_order = replacement_choice(&second_leaf_batch);
    assert_counter_order_offer(&engine, &second_p1_order, 1, a, &a_doublers);
    assert_ne!(second_p1_order.resolution_prompt_id, first_p1_prompt_id);

    apply_recorded(
        &mut engine,
        1,
        RuledCommand {
            cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                resolution_prompt_id: first_p1_prompt_id,
                chosen_object_ids: vec![first_p1_handle],
                decision: ResolutionChoiceDecision::Unspecified as i32,
                ..Default::default()
            })),
        },
        false,
        Some(&mut commands),
        &mut trace,
    );

    let final_batch = apply_recorded(
        &mut engine,
        1,
        answer_for(
            &second_p1_order,
            vec![second_p1_order.replacement_options[0].application_id],
        ),
        true,
        Some(&mut commands),
        &mut trace,
    )
    .expect("P1 answers the current second-leaf offer");
    assert_eq!(proliferate_log_count(&final_batch, 0), 1);
    assert_eq!(
        engine.state.objects[&a].counter_count(CounterKind::Charge),
        9
    );
    assert_eq!(
        engine.state.objects[&b].counter_count(CounterKind::Charge),
        5
    );
    assert_eq!(trace.p0_proliferate_logs, 2);
    assert_eq!(
        trace.stack_resolved_object_ids,
        vec![original_stack_identity.stack_id],
        "the original Bastion activation exits the stack exactly once"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(engine.diagnostic_snapshot().unwrap()["state"]["pending_replacement_event"].is_null());
    assert!(engine.state.objects[&bastion].tapped);
    assert_eq!(engine.state.players[p0].mana_pool, mana_after_activation);
    assert_eq!(
        commands
            .iter()
            .filter(|command| matches!(command.command.cmd.as_ref(), Some(Cmd::ActivateAbility(_))))
            .count(),
        1,
        "the parent Bastion activation and its cost occur once"
    );
    assert_eq!(
        commands.iter().filter(|command| !command.accepted).count(),
        5,
        "the captured stream includes all wrong-actor, foreign, duplicate, and stale probes"
    );

    let (mut replay, replay_b, replay_b_doublers, replay_a, replay_a_doublers, replay_bastion) =
        prepare(SEED);
    assert_eq!((replay_b, replay_a, replay_bastion), (b, a, bastion));
    assert_eq!(replay_b_doublers, b_doublers);
    assert_eq!(replay_a_doublers, a_doublers);
    let mut replay_trace = ReplayTrace::default();
    for command in &commands {
        let result = apply_recorded(
            &mut replay,
            command.actor,
            command.command.clone(),
            command.accepted,
            None,
            &mut replay_trace,
        );
        assert_eq!(result.is_some(), command.accepted);
    }
    assert_eq!(
        replay_trace, trace,
        "the captured command replay is deterministic"
    );
    assert_eq!(
        replay.diagnostic_snapshot().unwrap(),
        engine.diagnostic_snapshot().unwrap()
    );
    assert_eq!(
        replay.state.objects[&replay_a].counter_count(CounterKind::Charge),
        9
    );
    assert_eq!(
        replay.state.objects[&replay_b].counter_count(CounterKind::Charge),
        5
    );
}

#[test]
fn counter_prohibitions_complete_both_ripples_children_without_a_cohort() {
    let registry = runtime_registry_with_local_fixtures(&[include_str!(
        "../../../tricerules-cards/data/tatterkite.ron"
    )]);
    let mut engine = runtime_engine_with_registry(810_111, registry);
    inject_permanent_on_battlefield(&mut engine, 0, STATIC_FIXTURE_ID);
    let tatterkite = inject_creature_on_battlefield(&mut engine, 0, "tatterkite");
    let tatterkite_generation = engine
        .state
        .zone_change_generation
        .get(&tatterkite)
        .copied()
        .unwrap_or(0);
    engine
        .state
        .objects
        .get_mut(&tatterkite)
        .expect("actual Tatterkite")
        .set_counter(CounterKind::Charge, 1);
    let ezuri = inject_creature_on_battlefield(&mut engine, 0, "ezuri,_stalker_of_spheres");
    assert_eq!(
        engine.state.objects[&tatterkite].counter_count(CounterKind::Charge),
        1
    );
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.stack.is_empty());

    let spell = inject_card_into_hand(&mut engine, 0, "ripples_of_potential");
    grant_pool(&mut engine, 0);
    let spell_slot = hand_index_for_card(&engine, 0, "ripples_of_potential");
    engine
        .apply_scenario_command(0, &cast_spell(spell_slot, vec![]))
        .expect("cast unchanged Ripples of Potential from hand");
    assert_eq!(engine.state.stack.last().map(|item| item.id), Some(spell));

    let first_prompt_batch = pass_live_players_with_batch(&mut engine);
    let first_offer = find_resolution_choice(&first_prompt_batch)
        .expect("the first actual Ripples instruction offers its Proliferate selection");
    assert_eq!(first_offer.choice_kind, ChoiceKind::Proliferate as i32);
    assert_eq!(first_offer.deciding_player_id, 0);
    assert_eq!(first_offer.candidate_object_ids, vec![tatterkite]);
    assert_ne!(first_offer.resolution_prompt_id, 0);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("first actual recipient offer remains current")
            .resolution_prompt_id
            .get(),
        first_offer.resolution_prompt_id
    );
    let hand_at_first_prompt = engine.state.players[engine.state.player_idx(0).unwrap()]
        .hand
        .len();

    let submit_tatterkite = |prompt_id| ruled_v1::RuledCommand {
        cmd: Some(ruled_v1::ruled_command::Cmd::SubmitResolutionChoice(
            SubmitResolutionChoice {
                resolution_prompt_id: prompt_id,
                chosen_object_ids: vec![tatterkite],
                decision: ResolutionChoiceDecision::Unspecified as i32,
                ..Default::default()
            },
        )),
    };
    let first_leaf_batch = engine
        .apply_scenario_command(0, &submit_tatterkite(first_offer.resolution_prompt_id))
        .expect("select Tatterkite through the actual first Proliferate prompt");
    assert_eq!(proliferate_log_count(&first_leaf_batch, 0), 1);
    assert_eq!(
        engine.state.objects[&tatterkite].counter_count(CounterKind::Charge),
        1
    );
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&tatterkite)
            .copied()
            .unwrap_or(0),
        tatterkite_generation
    );
    assert!(engine.state.objects[&tatterkite].phase_status.is_none());
    assert!(first_leaf_batch.events.iter().all(|event| !matches!(
        event.ev.as_ref(),
        Some(ruled_event::Ev::ResolutionChoiceRequired(choice))
            if choice.choice_kind == ChoiceKind::PermanentObjects as i32
    )));
    assert_eq!(
        engine.state.players[engine.state.player_idx(0).unwrap()]
            .hand
            .len(),
        hand_at_first_prompt,
        "Ezuri does not resolve between the two original Ripples instructions"
    );
    let second_offer = find_resolution_choice(&first_leaf_batch)
        .expect("the first prohibited placement completes and the second instruction proceeds");
    assert_eq!(second_offer.choice_kind, ChoiceKind::Proliferate as i32);
    assert_eq!(second_offer.deciding_player_id, 0);
    assert_eq!(second_offer.candidate_object_ids, vec![tatterkite]);
    assert_ne!(second_offer.resolution_prompt_id, 0);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the second actual recipient offer is current")
            .resolution_prompt_id
            .get(),
        second_offer.resolution_prompt_id
    );

    let parent_batch = engine
        .apply_scenario_command(0, &submit_tatterkite(second_offer.resolution_prompt_id))
        .expect("select Tatterkite through the actual second Proliferate prompt");
    assert_eq!(proliferate_log_count(&parent_batch, 0), 1);
    assert_eq!(
        engine.state.objects[&tatterkite].counter_count(CounterKind::Charge),
        1
    );
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&tatterkite)
            .copied()
            .unwrap_or(0),
        tatterkite_generation
    );
    assert!(engine.state.objects[&tatterkite].phase_status.is_none());
    assert!(parent_batch.events.iter().all(|event| !matches!(
        event.ev.as_ref(),
        Some(ruled_event::Ev::ResolutionChoiceRequired(choice))
            if choice.choice_kind == ChoiceKind::PermanentObjects as i32
    )));
    assert_eq!(
        parent_batch
            .events
            .iter()
            .filter(|event| matches!(
                event.ev.as_ref(),
                Some(ruled_event::Ev::Log(log)) if log.text == "Ripples of Potential resolves."
            ))
            .count(),
        1
    );
    assert_eq!(
        parent_batch
            .events
            .iter()
            .filter(|event| matches!(
                event.ev.as_ref(),
                Some(ruled_event::Ev::StackResolved(resolved)) if resolved.object_id == spell
            ))
            .count(),
        1,
        "the original physical Ripples spell resolves exactly once"
    );
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.players[engine.state.player_idx(0).unwrap()]
            .hand
            .len(),
        hand_at_first_prompt,
        "both Ezuri triggers wait until the original parent finishes"
    );
    let trigger_order = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("the two actual Proliferated events wait for Ezuri's trigger order");
    assert_eq!(trigger_order.deciding_player, 0);
    assert_eq!(trigger_order.candidates.len(), 2);
    assert!(trigger_order.candidates.iter().all(|trigger| {
        trigger.source_permanent_id == ezuri
            && trigger.controller == 0
            && trigger.card_id == "ezuri,_stalker_of_spheres"
            && trigger.ability_index == 1
    }));
    assert!(answer_trigger_order_in_engine_order(&mut engine));
    assert!(engine.state.pending_trigger_order.is_none());
    assert_eq!(engine.state.stack.len(), 2);
    assert!(engine.state.stack.iter().all(|item| {
        item.is_triggered
            && item.source_permanent_id == Some(ezuri)
            && item.controller == 0
            && item.card_id == "ezuri,_stalker_of_spheres"
            && item.ability_index == Some(1)
    }));

    for expected_remaining in (1..=2).rev() {
        assert_eq!(engine.state.stack.len(), expected_remaining);
        pass_live_players_with_batch(&mut engine);
        assert_eq!(engine.state.stack.len(), expected_remaining - 1);
        assert!(engine.state.pending_resolution.is_none());
    }
    assert_eq!(
        engine.state.players[engine.state.player_idx(0).unwrap()]
            .hand
            .len(),
        hand_at_first_prompt + 2,
        "Ezuri's two authored triggered_02 abilities each draw one card"
    );
    assert_eq!(engine.state.objects[&tatterkite].zone, Zone::Battlefield);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&tatterkite)
            .copied()
            .unwrap_or(0),
        tatterkite_generation
    );
    assert_eq!(
        engine.state.objects[&tatterkite].counter_count(CounterKind::Charge),
        1
    );
    assert!(engine.state.objects[&tatterkite].phase_status.is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_trigger_order.is_none());
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn ezuri_two_original_instructions_double_to_four_delayed_actions_without_interleaf_sbas() {
    let mut engine = runtime_four_player_engine(810_112);
    let stack_resolved_count =
        |batch: &RuledEventBatch, stack_id: u32| {
            batch
            .events
            .iter()
            .filter(|event| matches!(
                event.ev.as_ref(),
                Some(ruled_event::Ev::StackResolved(resolved)) if resolved.object_id == stack_id
            ))
            .count()
        };
    assert_eq!(engine.state.players.len(), 4);
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|player| player.id)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
    assert!(engine
        .state
        .players
        .iter()
        .all(|player| player.counters.is_empty()));
    assert!(engine
        .state
        .objects
        .values()
        .all(|object| object.counters.is_empty()));

    let fixture_definition = raw_card_fixture(STATIC_FIXTURE_RON);
    assert_eq!(
        fixture_definition.static_abilities.len(),
        1,
        "the local action-doubling fixture contributes exactly one effective Double"
    );
    assert!(matches!(
        fixture_definition.static_abilities[0].definition,
        StaticAbilityDef::DoubleControllerProliferations
    ));
    let doubler = inject_permanent_on_battlefield(&mut engine, 0, STATIC_FIXTURE_ID);
    assert_eq!(engine.state.objects[&doubler].owner, 0);
    assert_eq!(engine.state.objects[&doubler].zone, Zone::Battlefield);
    let doubler_generation = engine
        .state
        .zone_change_generation
        .get(&doubler)
        .copied()
        .unwrap_or(0);
    let fixture_sources = engine
        .state
        .players
        .iter()
        .flat_map(|player| player.battlefield.iter())
        .filter(|object_id| engine.state.objects[object_id].card_id == STATIC_FIXTURE_ID)
        .count();
    assert_eq!(
        fixture_sources, 1,
        "exactly one doubler is on the battlefield"
    );
    assert!(engine.state.players.iter().all(|player| {
        player
            .battlefield
            .iter()
            .all(|object_id| engine.state.objects[object_id].card_id != "doubling_season")
    }));

    let p3 = engine.state.player_idx(3).expect("P3 fixture player");
    engine.state.players[p3]
        .counters
        .insert(CounterKind::Poison, 9);
    assert_eq!(engine.state.players[p3].counters.len(), 1);
    assert_eq!(
        engine.state.players[p3].counters.get(&CounterKind::Poison),
        Some(&9)
    );

    let ezuri = inject_card_into_hand(&mut engine, 0, "ezuri,_stalker_of_spheres");
    grant_pool(&mut engine, 0);
    let ezuri_slot = hand_index_for_card(&engine, 0, "ezuri,_stalker_of_spheres");
    engine
        .apply_scenario_command(0, &cast_spell(ezuri_slot, vec![]))
        .expect("cast the actual unchanged Ezuri card from hand");
    assert_eq!(engine.state.stack.last().map(|item| item.id), Some(ezuri));

    pass_live_players_with_batch(&mut engine);
    assert_eq!(engine.state.objects[&ezuri].zone, Zone::Battlefield);
    let ezuri_generation = engine
        .state
        .zone_change_generation
        .get(&ezuri)
        .copied()
        .unwrap_or(0);
    let etb_stack_item = engine
        .state
        .stack
        .last()
        .expect("Ezuri's actual ETB trigger");
    assert!(etb_stack_item.is_triggered);
    assert_eq!(etb_stack_item.source_permanent_id, Some(ezuri));
    assert_eq!(etb_stack_item.controller, 0);
    assert_eq!(etb_stack_item.card_id, "ezuri,_stalker_of_spheres");
    assert_eq!(etb_stack_item.ability_index, Some(0));
    let etb_stack_item_id = etb_stack_item.id;

    let etb_batch = pass_live_players_with_batch(&mut engine);
    let mut etb_parent_resolutions = stack_resolved_count(&etb_batch, etb_stack_item_id);
    let branch_offer = find_resolution_choice(&etb_batch)
        .expect("the actual Ezuri ETB offers its optional branch");
    assert_eq!(
        branch_offer.choice_kind,
        ChoiceKind::ResolutionBranch as i32
    );
    assert_eq!(branch_offer.deciding_player_id, 0);
    assert_ne!(branch_offer.resolution_prompt_id, 0);
    let branch_batch = engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: branch_offer.resolution_prompt_id,
                    decision: ResolutionChoiceDecision::SelectBranch as i32,
                    selected_branch_index: 0,
                    ..Default::default()
                })),
            },
        )
        .expect("select Ezuri's current optional branch index zero");
    etb_parent_resolutions += stack_resolved_count(&branch_batch, etb_stack_item_id);
    let payment_offer = find_resolution_choice(&branch_batch)
        .expect("Ezuri's selected branch opens its single real mana choice");
    assert_eq!(payment_offer.choice_kind, ChoiceKind::ManaPayment as i32);
    assert_eq!(payment_offer.deciding_player_id, 0);
    assert_ne!(payment_offer.resolution_prompt_id, 0);

    let p0_mana_total = |engine: &GameEngine| {
        let pool = &engine.state.players[engine.state.player_idx(0).unwrap()].mana_pool;
        pool.white + pool.blue + pool.black + pool.red + pool.green + pool.colorless
    };
    let mana_before_payment = p0_mana_total(&engine);
    let payment_batch =
        submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
            .expect("pay Ezuri's actual optional {3} cost once");
    etb_parent_resolutions += stack_resolved_count(&payment_batch, etb_stack_item_id);
    let mana_after_payment = p0_mana_total(&engine);
    assert_eq!(mana_before_payment - mana_after_payment, 3);

    let first_offer = find_resolution_choice(&payment_batch)
        .expect("the paid actual Ezuri parent reaches its first doubled Proliferate leaf");
    assert_eq!(first_offer.choice_kind, ChoiceKind::Proliferate as i32);
    assert_eq!(first_offer.deciding_player_id, 0);
    assert!(first_offer.candidate_object_ids.is_empty());
    assert_eq!(first_offer.candidate_player_ids, vec![3]);
    assert_ne!(first_offer.resolution_prompt_id, 0);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the first real Proliferate leaf is parked")
            .resolution_prompt_id
            .get(),
        first_offer.resolution_prompt_id
    );
    let hand_at_first_prompt = engine.state.players[engine.state.player_idx(0).unwrap()]
        .hand
        .len();
    assert!(engine.state.pending_trigger_order.is_none());
    assert!(!engine.state.stack.iter().any(|item| {
        item.is_triggered
            && item.source_permanent_id == Some(ezuri)
            && item.ability_index == Some(1)
    }));

    let reject_priority_while_parked = |engine: &mut GameEngine| {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("a Proliferate leaf remains parked");
        assert_eq!(pending.presentation.choice_kind, ChoiceKind::Proliferate);
        let prompt_id = pending.resolution_prompt_id.get();
        assert_ne!(prompt_id, 0);
        let before = engine
            .diagnostic_snapshot()
            .expect("capture full state before the real priority probe");
        let priority_player = engine.state.priority_player_id();
        assert!(engine
            .apply_scenario_command(priority_player, &pass())
            .is_err());
        assert_eq!(
            engine
                .diagnostic_snapshot()
                .expect("state after rejected priority"),
            before,
            "priority is rejected atomically while the resolution choice is parked"
        );
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .expect("the same choice remains current")
                .resolution_prompt_id
                .get(),
            prompt_id
        );
    };

    let mut leaf_batch = payment_batch;
    for (leaf_index, chosen_players) in [(0, vec![3]), (1, vec![]), (2, vec![]), (3, vec![])] {
        let current_offer = find_resolution_choice(&leaf_batch)
            .expect("each actual leaf offers its current Proliferate recipient prompt");
        assert_eq!(current_offer.choice_kind, ChoiceKind::Proliferate as i32);
        assert_eq!(current_offer.deciding_player_id, 0);
        assert!(current_offer.candidate_object_ids.is_empty());
        assert_eq!(current_offer.candidate_player_ids, vec![3]);
        assert_ne!(current_offer.resolution_prompt_id, 0);
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .expect("current leaf is parked")
                .resolution_prompt_id
                .get(),
            current_offer.resolution_prompt_id
        );
        reject_priority_while_parked(&mut engine);

        leaf_batch = engine
            .apply_scenario_command(0, &proliferate_choice(vec![], chosen_players))
            .expect("answer only the current real Proliferate prompt");
        etb_parent_resolutions += stack_resolved_count(&leaf_batch, etb_stack_item_id);
        assert_eq!(proliferate_log_count(&leaf_batch, 0), 1);
        assert_eq!(p0_mana_total(&engine), mana_after_payment);
        assert_eq!(engine.state.players[p3].counters.len(), 1);
        assert_eq!(
            engine.state.players[p3].counters.get(&CounterKind::Poison),
            Some(&10),
            "only the first chosen leaf adds poison"
        );
        assert_eq!(
            engine.state.players[engine.state.player_idx(0).unwrap()]
                .hand
                .len(),
            hand_at_first_prompt
        );
        assert!(engine.state.objects[&doubler].zone == Zone::Battlefield);
        assert_eq!(
            engine
                .state
                .zone_change_generation
                .get(&doubler)
                .copied()
                .unwrap_or(0),
            doubler_generation
        );
        assert_eq!(
            engine
                .state
                .zone_change_generation
                .get(&ezuri)
                .copied()
                .unwrap_or(0),
            ezuri_generation
        );

        if leaf_index < 3 {
            assert!(!engine.state.players[p3].has_lost);
            assert!(engine.state.pending_trigger_order.is_none());
            assert!(!engine.state.stack.iter().any(|item| {
                item.is_triggered
                    && item.source_permanent_id == Some(ezuri)
                    && item.ability_index == Some(1)
            }));
            let next_offer = find_resolution_choice(&leaf_batch)
                .expect("the same ETB parent exposes the next doubled leaf");
            assert_eq!(next_offer.choice_kind, ChoiceKind::Proliferate as i32);
            assert_eq!(next_offer.deciding_player_id, 0);
            assert!(next_offer.candidate_object_ids.is_empty());
            assert_eq!(next_offer.candidate_player_ids, vec![3]);
            assert_ne!(next_offer.resolution_prompt_id, 0);
            assert_eq!(
                engine
                    .state
                    .pending_resolution
                    .as_ref()
                    .expect("the parent remains parked between authored instructions")
                    .resolution_prompt_id
                    .get(),
                next_offer.resolution_prompt_id
            );
        }
    }

    assert_eq!(
        etb_parent_resolutions, 1,
        "the captured actual Ezuri ETB stack item resolves exactly once across all four leaves"
    );
    assert!(engine.state.players[p3].has_lost);
    assert_eq!(
        engine.state.players[p3].counters.get(&CounterKind::Poison),
        Some(&10)
    );
    for player_id in 0..3 {
        assert!(!engine.state.players[engine.state.player_idx(player_id).unwrap()].has_lost);
    }
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .filter(|player| !player.has_lost)
            .count(),
        3
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(p0_mana_total(&engine), mana_after_payment);
    assert_eq!(
        engine.state.players[engine.state.player_idx(0).unwrap()]
            .hand
            .len(),
        hand_at_first_prompt
    );

    let trigger_order =
        engine.state.pending_trigger_order.as_ref().expect(
            "all four Proliferated events wait for their Ezuri triggers after the parent ends",
        );
    assert_eq!(trigger_order.deciding_player, 0);
    assert_eq!(trigger_order.candidates.len(), 4);
    assert!(trigger_order.candidates.iter().all(|trigger| {
        trigger.source_permanent_id == ezuri
            && trigger.controller == 0
            && trigger.card_id == "ezuri,_stalker_of_spheres"
            && trigger.ability_index == 1
    }));
    assert!(answer_trigger_order_in_engine_order(&mut engine));
    assert!(engine.state.pending_trigger_order.is_none());
    assert_eq!(engine.state.stack.len(), 4);
    assert!(engine.state.stack.iter().all(|item| {
        item.is_triggered
            && item.source_permanent_id == Some(ezuri)
            && item.controller == 0
            && item.card_id == "ezuri,_stalker_of_spheres"
            && item.ability_index == Some(1)
    }));

    for expected_remaining in (1..=4).rev() {
        assert_eq!(engine.state.stack.len(), expected_remaining);
        pass_live_players_with_batch(&mut engine);
        assert_eq!(engine.state.stack.len(), expected_remaining - 1);
        assert!(engine.state.pending_resolution.is_none());
    }
    assert_eq!(
        engine.state.players[engine.state.player_idx(0).unwrap()]
            .hand
            .len(),
        hand_at_first_prompt + 4,
        "the four actual Ezuri triggered_02 abilities each draw one card after the parent"
    );
    assert_eq!(p0_mana_total(&engine), mana_after_payment);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_trigger_order.is_none());
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.stack.is_empty());

    let mut no_counters = runtime_four_player_engine(810_113);
    assert_eq!(no_counters.state.players.len(), 4);
    assert!(no_counters
        .state
        .players
        .iter()
        .all(|player| player.counters.is_empty()));
    assert!(no_counters
        .state
        .objects
        .values()
        .all(|object| object.counters.is_empty()));
    let no_counters_fixture = raw_card_fixture(STATIC_FIXTURE_RON);
    assert_eq!(no_counters_fixture.static_abilities.len(), 1);
    assert!(matches!(
        no_counters_fixture.static_abilities[0].definition,
        StaticAbilityDef::DoubleControllerProliferations
    ));
    let no_counters_doubler =
        inject_permanent_on_battlefield(&mut no_counters, 0, STATIC_FIXTURE_ID);
    let no_counters_doubler_generation = no_counters
        .state
        .zone_change_generation
        .get(&no_counters_doubler)
        .copied()
        .unwrap_or(0);
    let no_counters_fixture_sources = no_counters
        .state
        .players
        .iter()
        .flat_map(|player| player.battlefield.iter())
        .filter(|object_id| no_counters.state.objects[object_id].card_id == STATIC_FIXTURE_ID)
        .count();
    assert_eq!(no_counters_fixture_sources, 1);
    assert!(no_counters.state.players.iter().all(|player| {
        player
            .battlefield
            .iter()
            .all(|object_id| no_counters.state.objects[object_id].card_id != "doubling_season")
    }));

    let no_counters_ezuri = inject_card_into_hand(&mut no_counters, 0, "ezuri,_stalker_of_spheres");
    grant_pool(&mut no_counters, 0);
    let no_counters_ezuri_slot = hand_index_for_card(&no_counters, 0, "ezuri,_stalker_of_spheres");
    no_counters
        .apply_scenario_command(0, &cast_spell(no_counters_ezuri_slot, vec![]))
        .expect("cast the actual unchanged Ezuri card in the fresh no-counters row");
    pass_live_players_with_batch(&mut no_counters);
    assert_eq!(
        no_counters.state.objects[&no_counters_ezuri].zone,
        Zone::Battlefield
    );
    let no_counters_ezuri_generation = no_counters
        .state
        .zone_change_generation
        .get(&no_counters_ezuri)
        .copied()
        .unwrap_or(0);
    let no_counters_etb = no_counters
        .state
        .stack
        .last()
        .expect("the fresh row has Ezuri's actual ETB trigger");
    assert!(no_counters_etb.is_triggered);
    assert_eq!(no_counters_etb.source_permanent_id, Some(no_counters_ezuri));
    assert_eq!(no_counters_etb.ability_index, Some(0));
    let no_counters_etb_stack_item_id = no_counters_etb.id;
    let no_counters_etb_batch = pass_live_players_with_batch(&mut no_counters);
    let mut no_counters_etb_parent_resolutions =
        stack_resolved_count(&no_counters_etb_batch, no_counters_etb_stack_item_id);
    let no_counters_branch = find_resolution_choice(&no_counters_etb_batch)
        .expect("the fresh row resolves the actual Ezuri ETB branch");
    assert_eq!(
        no_counters_branch.choice_kind,
        ChoiceKind::ResolutionBranch as i32
    );
    assert_ne!(no_counters_branch.resolution_prompt_id, 0);
    let no_counters_branch_batch = no_counters
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: no_counters_branch.resolution_prompt_id,
                    decision: ResolutionChoiceDecision::SelectBranch as i32,
                    selected_branch_index: 0,
                    ..Default::default()
                })),
            },
        )
        .expect("select the current real Ezuri branch in the no-counters row");
    no_counters_etb_parent_resolutions +=
        stack_resolved_count(&no_counters_branch_batch, no_counters_etb_stack_item_id);
    let no_counters_payment = find_resolution_choice(&no_counters_branch_batch)
        .expect("the selected actual branch opens its one real {3} payment");
    assert_eq!(
        no_counters_payment.choice_kind,
        ChoiceKind::ManaPayment as i32
    );
    assert_ne!(no_counters_payment.resolution_prompt_id, 0);
    let no_counters_p0_mana_total = |engine: &GameEngine| {
        let pool = &engine.state.players[engine.state.player_idx(0).unwrap()].mana_pool;
        pool.white + pool.blue + pool.black + pool.red + pool.green + pool.colorless
    };
    let no_counters_hand_before_payment = no_counters.state.players
        [no_counters.state.player_idx(0).unwrap()]
    .hand
    .len();
    let no_counters_mana_before_payment = no_counters_p0_mana_total(&no_counters);
    let no_counters_payment_batch =
        submit_mana_resolution_decision(&mut no_counters, 0, ResolutionChoiceDecision::PayMana)
            .expect("pay the actual Ezuri optional cost once in the fresh no-counters row");
    no_counters_etb_parent_resolutions +=
        stack_resolved_count(&no_counters_payment_batch, no_counters_etb_stack_item_id);
    assert_eq!(
        no_counters_etb_parent_resolutions, 1,
        "the captured actual no-counters Ezuri ETB stack item resolves exactly once inline"
    );
    let no_counters_mana_after_payment = no_counters_p0_mana_total(&no_counters);
    assert_eq!(
        no_counters_mana_before_payment - no_counters_mana_after_payment,
        3
    );
    assert_eq!(
        no_counters.state.players[no_counters.state.player_idx(0).unwrap()]
            .hand
            .len(),
        no_counters_hand_before_payment
    );
    assert!(no_counters.state.pending_resolution.is_none());
    assert!(find_resolution_choice(&no_counters_payment_batch).is_none());
    assert!(no_counters_payment_batch
        .events
        .iter()
        .all(|event| !matches!(
            event.ev.as_ref(),
            Some(ruled_event::Ev::ResolutionChoiceRequired(_))
        )));
    assert!(no_counters.state.stack.is_empty());

    let no_counters_trigger_order = no_counters
        .state
        .pending_trigger_order
        .as_ref()
        .expect("all four inline Proliferate leaves finish before Ezuri triggers await order");
    assert_eq!(no_counters_trigger_order.deciding_player, 0);
    assert_eq!(no_counters_trigger_order.candidates.len(), 4);
    assert!(no_counters_trigger_order.candidates.iter().all(|trigger| {
        trigger.source_permanent_id == no_counters_ezuri
            && trigger.controller == 0
            && trigger.card_id == "ezuri,_stalker_of_spheres"
            && trigger.ability_index == 1
    }));
    assert!(answer_trigger_order_in_engine_order(&mut no_counters));
    assert!(no_counters.state.pending_trigger_order.is_none());
    assert_eq!(no_counters.state.stack.len(), 4);
    assert!(no_counters.state.stack.iter().all(|item| {
        item.is_triggered
            && item.source_permanent_id == Some(no_counters_ezuri)
            && item.controller == 0
            && item.card_id == "ezuri,_stalker_of_spheres"
            && item.ability_index == Some(1)
    }));
    for expected_remaining in (1..=4).rev() {
        assert_eq!(no_counters.state.stack.len(), expected_remaining);
        pass_live_players_with_batch(&mut no_counters);
        assert_eq!(no_counters.state.stack.len(), expected_remaining - 1);
        assert!(no_counters.state.pending_resolution.is_none());
    }
    assert_eq!(
        no_counters.state.players[no_counters.state.player_idx(0).unwrap()]
            .hand
            .len(),
        no_counters_hand_before_payment + 4,
        "the four actual Ezuri index-one triggers each draw after the all-inline parent completes"
    );
    assert_eq!(
        no_counters_p0_mana_total(&no_counters),
        no_counters_mana_after_payment
    );
    assert_eq!(
        no_counters
            .state
            .zone_change_generation
            .get(&no_counters_doubler)
            .copied()
            .unwrap_or(0),
        no_counters_doubler_generation
    );
    assert_eq!(
        no_counters
            .state
            .zone_change_generation
            .get(&no_counters_ezuri)
            .copied()
            .unwrap_or(0),
        no_counters_ezuri_generation
    );
    assert!(no_counters
        .state
        .players
        .iter()
        .all(|player| player.counters.is_empty()));
    assert!(no_counters
        .state
        .objects
        .values()
        .all(|object| object.counters.is_empty()));
    assert!(no_counters.state.pending_resolution.is_none());
    assert!(no_counters.state.pending_trigger_order.is_none());
    assert!(no_counters.state.pending_triggers.is_empty());
    assert!(no_counters.state.stack.is_empty());
}
