//! Actual Garruk casts and -3 activations with labeled synthetic hand, mana and targets.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::{CounterKind, Keyword};
use tricerules_cards::Color;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

const GARRUK: &str = "garruk,_cursed_huntsman";
const WOLF: &str = "wolf_bg_2_2_garruk_loyalty";

fn setup(seed: u64) -> (GameEngine, u32) {
    let registry = tricerules_cards::registry::global();
    assert!(
        registry.get(GARRUK).is_some(),
        "Garruk, Cursed Huntsman must be registered"
    );
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("swamp", &[]), deck_with("forest", &[])]),
        true,
    )
    .expect("create two-player game");
    advance_to_main1_from_game_start(&mut engine);
    // The hand card and mana are synthetic setup; the cast, payment and entry are real commands.
    let garruk = inject_card_into_hand(&mut engine, 0, GARRUK);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 4,
            b: 1,
            g: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, GARRUK);
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast actual Garruk for {4}{B}{G}");
    assert_eq!(
        (
            engine.state.players[0].mana_pool.colorless,
            engine.state.players[0].mana_pool.black,
            engine.state.players[0].mana_pool.green
        ),
        (0, 0, 0)
    );
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&garruk].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&garruk].counter_count(CounterKind::Loyalty),
        5
    );
    (engine, garruk)
}

fn ability_index(id: &str) -> u32 {
    let face = tricerules_cards::registry::global()
        .get(GARRUK)
        .unwrap()
        .primary_face();
    face.activated_abilities
        .iter()
        .position(|ability| ability.ability_id.as_str() == id)
        .unwrap_or_else(|| panic!("Garruk ability {id} is present")) as u32
}

fn activate_minus_three(
    engine: &mut GameEngine,
    garruk: u32,
    targets: Vec<TargetRef>,
) -> Result<RuledEventBatch, tricerules_core::EngineError> {
    apply_ability(engine, 0, garruk, ability_index("activated_02"), targets)
}

fn dev_move(engine: &mut GameEngine, owner: i32, name: &str, zone: DevZone, ready: bool) {
    engine.enable_dev_commands();
    let actor = engine.state.priority_player_id();
    engine
        .apply_scenario_command(
            actor,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: owner,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: name.into(),
                        zone: zone as i32,
                        ready,
                    })),
                })),
            },
        )
        .expect("move the fixture through the engine's normal zone-change path");
}

#[test]
fn garruk_minus_three_destroys_either_players_creature_then_draws() {
    for owner in [0, 1] {
        let (mut engine, garruk) = setup(261_027_900 + owner as u64);
        let target = inject_creature_on_battlefield(&mut engine, owner, "grizzly_bears");
        let top = *engine.state.players[0].library.front().unwrap();
        let draws = engine.state.turn_history.current.player(0).cards_drawn;
        activate_minus_three(&mut engine, garruk, target_object(target))
            .expect("activate -3 targeting a creature");
        assert_eq!(
            engine.state.objects[&garruk].counter_count(CounterKind::Loyalty),
            2
        );
        let stack_id = engine.state.stack.last().unwrap().id;
        let batch = pass_both_players_with_batch(&mut engine);
        assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
        assert!(engine.state.players[0].hand.contains(&top));
        assert_eq!(
            engine.state.turn_history.current.player(0).cards_drawn,
            draws + 1
        );
        assert_eq!(
            batch
                .events
                .iter()
                .filter(|event| matches!(&event.ev,
            Some(Ev::StackResolved(resolved)) if resolved.object_id == stack_id))
                .count(),
            1
        );
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn garruk_minus_three_draws_even_if_the_legal_creature_is_indestructible() {
    let (mut engine, garruk) = setup(261_027_902);
    let target = inject_permanent_on_battlefield(&mut engine, 1, "darksteel_myr");
    assert!(engine
        .characteristics(target)
        .unwrap()
        .has_keyword(Keyword::Indestructible));
    let top = *engine.state.players[0].library.front().unwrap();
    let draws = engine.state.turn_history.current.player(0).cards_drawn;
    activate_minus_three(&mut engine, garruk, target_object(target))
        .expect("an indestructible creature is a legal target");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert!(engine.state.players[0].hand.contains(&top));
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws + 1
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn garruk_minus_three_rejects_noncreatures_and_does_not_draw_for_stale_targets() {
    for return_target in [false, true] {
        let (mut engine, garruk) = setup(261_027_903 + u64::from(return_target));
        let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        let land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
        let top = *engine.state.players[0].library.front().unwrap();
        let draws = engine.state.turn_history.current.player(0).cards_drawn;
        activate_minus_three(&mut engine, garruk, target_object(land))
            .expect_err("a land is not a legal creature target");
        assert_eq!(
            engine.state.objects[&garruk].counter_count(CounterKind::Loyalty),
            5
        );
        activate_minus_three(&mut engine, garruk, target_object(target))
            .expect("choose a legal target and pay -3");
        let old_generation = engine
            .state
            .zone_change_generation
            .get(&target)
            .copied()
            .unwrap_or(0);
        dev_move(&mut engine, 1, "Grizzly Bears", DevZone::Hand, false);
        if return_target {
            dev_move(&mut engine, 1, "Grizzly Bears", DevZone::Battlefield, true);
            assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
        } else {
            assert_eq!(engine.state.objects[&target].zone, Zone::Hand);
        }
        assert!(
            engine
                .state
                .zone_change_generation
                .get(&target)
                .copied()
                .unwrap_or(0)
                > old_generation
        );
        pass_both_players(&mut engine);
        assert_eq!(
            engine.state.turn_history.current.player(0).cards_drawn,
            draws
        );
        assert_eq!(engine.state.players[0].library.front().copied(), Some(top));
        assert!(!engine.state.players[0].hand.contains(&top));
        assert_eq!(
            engine.state.objects[&garruk].counter_count(CounterKind::Loyalty),
            2,
            "an illegal target does not refund the paid loyalty"
        );
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn garruk_minus_three_resolves_after_its_source_leaves() {
    let (mut engine, garruk) = setup(261_027_905);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let top = *engine.state.players[0].library.front().unwrap();
    let draws = engine.state.turn_history.current.player(0).cards_drawn;
    activate_minus_three(&mut engine, garruk, target_object(target)).expect("activate -3");
    dev_move(
        &mut engine,
        0,
        "Garruk, Cursed Huntsman",
        DevZone::Graveyard,
        false,
    );
    assert_eq!(engine.state.objects[&garruk].zone, Zone::Graveyard);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert!(engine.state.players[0].hand.contains(&top));
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws + 1
    );
    assert!(engine.state.stack.is_empty());
}

fn assert_wolf_identity(identity: &tricerules_proto::ruled::v1::TokenIdentity) {
    assert_eq!(identity.name, "Wolf");
    assert_eq!(identity.pt, "2/2");
    assert_eq!(identity.color, "bg");
    assert!(identity.is_creature);
    assert!(identity.types.iter().any(|kind| kind == "Wolf"));
    assert_eq!(identity.ability_texts.len(), 1);
    assert!(identity.ability_texts[0].contains("triggered ability"));
    assert!(identity.ability_texts[0].contains("triggered_01"));
}

fn create_wolves(engine: &mut GameEngine, garruk: u32) -> Vec<u32> {
    apply_ability(engine, 0, garruk, ability_index("activated_01"), vec![])
        .expect("activate Garruk's actual 0 ability");
    assert_eq!(
        engine.state.objects[&garruk].counter_count(CounterKind::Loyalty),
        5
    );
    let batch = pass_both_players_with_batch(engine);
    let wolves = battlefield_token_oids(engine, 0, WOLF);
    assert_eq!(wolves.len(), 2);
    assert_ne!(wolves[0], wolves[1]);
    for &wolf in &wolves {
        assert!(engine.state.objects[&wolf].is_token());
        let characteristics = engine.characteristics(wolf).unwrap();
        assert!(characteristics.is_creature() && characteristics.has_type("Wolf"));
        assert_eq!(characteristics.colors, [Color::Black, Color::Green]);
        assert_eq!(
            (characteristics.power, characteristics.toughness),
            (Some(2), Some(2))
        );
        let created = batch
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(Ev::TokenCreated(created)) if created.object_id == wolf => Some(created),
                _ => None,
            })
            .expect("each actual token creation has a public identity");
        assert_eq!(created.controller_player_id, 0);
        assert_eq!(created.card_id, WOLF);
        assert_wolf_identity(created.identity.as_ref().expect("created token identity"));
    }
    let snapshot = engine.initial_response_batch();
    let identities: Vec<_> = snapshot
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .flat_map(|view| &view.per_player)
        .flat_map(|player| &player.battlefield_objects)
        .filter(|object| wolves.contains(&object.object_id))
        .collect();
    assert_eq!(identities.len(), 2);
    for object in identities {
        assert_wolf_identity(
            object
                .token_identity
                .as_ref()
                .expect("snapshot token identity"),
        );
    }
    wolves
}

#[test]
fn garruk_zero_creates_two_black_green_wolves_with_public_identity() {
    let (mut engine, garruk) = setup(261_027_910);
    create_wolves(&mut engine, garruk);
}

#[test]
fn garruk_wolf_death_retains_its_trigger_and_counts_current_garruk_subtypes() {
    let (mut engine, garruk) = setup(261_027_911);
    let wolves = create_wolves(&mut engine, garruk);
    // The other-name Garruk subtype, ordinary Jace and opposing Garruk are synthetic placements.
    let other = inject_permanent_on_battlefield(&mut engine, 0, "jace,_unraveler_of_secrets");
    let mut face = tricerules_cards::registry::global()
        .get("jace,_unraveler_of_secrets")
        .unwrap()
        .primary_face()
        .clone();
    face.name = "Synthetic Other Garruk".into();
    face.types = vec!["Planeswalker".into(), "Garruk".into()];
    engine
        .state
        .objects
        .get_mut(&other)
        .unwrap()
        .copiable_values = Some(tricerules_core::state::CopiableValues {
        prototyped: false,
        source_card_id: "jace,_unraveler_of_secrets".into(),
        source_face_index: 0,
        face,
        room_faces: None,
        display_name: "Synthetic Other Garruk".into(),
    });
    engine
        .state
        .objects
        .get_mut(&other)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 5);
    let jace = inject_permanent_on_battlefield(&mut engine, 0, "jace,_unraveler_of_secrets");
    engine
        .state
        .objects
        .get_mut(&jace)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 5);
    let opposing = inject_permanent_on_battlefield(&mut engine, 1, GARRUK);
    engine
        .state
        .objects
        .get_mut(&opposing)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 5);
    assert!(engine.characteristics(other).unwrap().has_type("Garruk"));
    assert!(!engine.characteristics(jace).unwrap().has_type("Garruk"));

    // Synthetic hand and mana; an actual Bolt causes the Wolf's death.
    inject_card_into_hand(&mut engine, 0, "lightning_bolt");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(wolves[0])))
        .expect("cast Bolt on a Wolf");
    let batch = pass_both_players_with_batch(&mut engine);
    assert!(
        !engine.state.objects.contains_key(&wolves[0]),
        "the dead token has ceased to exist"
    );
    let trigger = engine
        .state
        .stack
        .last()
        .expect("the captured Wolf trigger is on the stack");
    assert!(trigger.is_triggered);
    assert_eq!(trigger.source_permanent_id, Some(wolves[0]));
    assert_eq!(trigger.controller, 0);
    assert!(trigger.triggered_ability.is_some());
    let pushed = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackPushed(pushed)) if pushed.object_id == trigger.id => Some(pushed),
            _ => None,
        })
        .expect("the ceased token's trigger has a public stack event");
    let presentation = pushed
        .primary_presentation
        .as_ref()
        .expect("captured token presentation");
    assert_eq!(presentation.card_id, WOLF);
    assert_eq!(presentation.face_id, "wolf");
    assert_eq!(
        presentation
            .path
            .iter()
            .map(|part| part.id.as_str())
            .collect::<Vec<_>>(),
        ["triggered_01"]
    );
    let text = presentation.fallback_text.to_ascii_lowercase();
    assert!(
        text.contains("loyalty counter") && text.contains("each garruk planeswalker you control"),
        "the emitted fallback describes the actual token ability: {text}"
    );
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&garruk].counter_count(CounterKind::Loyalty),
        6
    );
    assert_eq!(
        engine.state.objects[&other].counter_count(CounterKind::Loyalty),
        6
    );
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        5
    );
    assert_eq!(
        engine.state.objects[&opposing].counter_count(CounterKind::Loyalty),
        5
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn garruk_two_wolves_dying_together_create_separate_loyalty_triggers() {
    let (mut engine, garruk) = setup(261_027_912);
    let wolves = create_wolves(&mut engine, garruk);
    // Synthetic hand and mana; real Wrath resolves and destroys both Wolves simultaneously.
    inject_card_into_hand(&mut engine, 0, "wrath_of_god");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            w: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "wrath_of_god");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast actual Wrath");
    pass_both_players(&mut engine);
    assert!(wolves
        .iter()
        .all(|wolf| !engine.state.objects.contains_key(wolf)));
    answer_trigger_order_in_engine_order(&mut engine);
    let triggers: Vec<_> = engine
        .state
        .stack
        .iter()
        .filter(|item| item.is_triggered)
        .collect();
    assert_eq!(triggers.len(), 2);
    assert!(wolves.iter().all(|wolf| triggers
        .iter()
        .any(|item| item.source_permanent_id == Some(*wolf))));
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&garruk].counter_count(CounterKind::Loyalty),
        7
    );
    assert!(engine.state.stack.is_empty());
}

fn bolt_wolf(engine: &mut GameEngine, actor: usize, wolf: u32) -> RuledEventBatch {
    // Hand and mana are synthetic; the spell, target, payment and damage are real.
    let player = engine.state.players[actor].id;
    inject_card_into_hand(engine, actor, "lightning_bolt");
    give_mana(
        engine,
        player,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, actor, "lightning_bolt");
    engine
        .apply_scenario_command(player, &cast_spell(slot, target_object(wolf)))
        .expect("cast Bolt at the Wolf");
    pass_both_players_with_batch(engine)
}

#[test]
fn garruk_zero_consumes_the_ordinary_loyalty_activation_allowance() {
    let (mut engine, garruk) = setup(261_027_913);
    let wolves = create_wolves(&mut engine, garruk);
    apply_ability(
        &mut engine,
        0,
        garruk,
        ability_index("activated_01"),
        vec![],
    )
    .expect_err("the zero-cost ability is still a loyalty activation");
    activate_minus_three(&mut engine, garruk, target_object(wolves[0]))
        .expect_err("a different loyalty ability also cannot be activated again this turn");
    assert_eq!(
        engine.state.objects[&garruk].counter_count(CounterKind::Loyalty),
        5
    );
    assert_eq!(battlefield_token_oids(&engine, 0, WOLF).len(), 2);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn garruk_stolen_wolf_death_uses_its_controller_at_death() {
    use tricerules_core::TurnStep;
    let (mut engine, garruk) = setup(261_027_914);
    let wolf = create_wolves(&mut engine, garruk)[0];
    // The opposing Garruk and its counters are synthetic; the control change below is real.
    let opposing = inject_permanent_on_battlefield(&mut engine, 1, GARRUK);
    engine
        .state
        .objects
        .get_mut(&opposing)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 5);
    let first_turn = engine.state.turn_instance;
    for _ in 0..100 {
        if engine.state.turn_instance > first_turn
            && engine.state.active_player_id() == 1
            && engine.state.turn_step == TurnStep::Main1
        {
            break;
        }
        let (actor, command) = match engine.state.cleanup_discard_player {
            Some(player) => {
                let seat = engine.state.player_idx(player).unwrap();
                let excess = engine.state.players[seat].hand.len() - 7;
                (player, discard_cleanup_batch((0..excess as u32).collect()))
            }
            None => (engine.state.priority_player_id(), pass()),
        };
        engine
            .apply_scenario_command(actor, &command)
            .expect("advance through the real turn");
    }
    assert_eq!(engine.state.active_player_id(), 1);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    // Synthetic hand and mana; player 1 actually casts Act of Treason on player 0's Wolf.
    inject_card_into_hand(&mut engine, 1, "act_of_treason");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            c: 2,
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 1, "act_of_treason");
    engine
        .apply_scenario_command(1, &cast_spell(slot, target_object(wolf)))
        .expect("cast Act of Treason targeting the opposing Wolf");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&wolf].owner, 0);
    assert_eq!(engine.characteristics(wolf).unwrap().controller, 1);

    bolt_wolf(&mut engine, 1, wolf);
    assert!(!engine.state.objects.contains_key(&wolf));
    let trigger = engine
        .state
        .stack
        .last()
        .expect("stolen Wolf's death trigger");
    assert!(trigger.is_triggered);
    assert_eq!(trigger.source_permanent_id, Some(wolf));
    assert_eq!(trigger.controller, 1);
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&garruk].counter_count(CounterKind::Loyalty),
        5
    );
    assert_eq!(
        engine.state.objects[&opposing].counter_count(CounterKind::Loyalty),
        6
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn garruk_copied_wolf_retains_its_death_trigger_after_cessation() {
    let (mut engine, garruk) = setup(261_027_915);
    let wolves = create_wolves(&mut engine, garruk);
    // Synthetic hand and mana; Cackling Counterpart copies an actual Wolf token.
    inject_card_into_hand(&mut engine, 0, "cackling_counterpart");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            u: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "cackling_counterpart");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(wolves[0])))
        .expect("cast Cackling Counterpart on the Wolf");
    pass_both_players(&mut engine);
    let copies: Vec<_> = engine
        .state
        .objects
        .values()
        .filter(|object| {
            object.zone == Zone::Battlefield && object.is_token() && !wolves.contains(&object.id)
        })
        .map(|object| object.id)
        .collect();
    let [copy] = copies.as_slice() else {
        panic!("one copied Wolf token")
    };
    let copy = *copy;
    assert!(engine.characteristics(copy).unwrap().has_type("Wolf"));
    bolt_wolf(&mut engine, 0, copy);
    assert!(!engine.state.objects.contains_key(&copy));
    let trigger = engine
        .state
        .stack
        .last()
        .expect("copied Wolf's death trigger");
    assert!(trigger.is_triggered && trigger.triggered_ability.is_some());
    assert_eq!(trigger.source_permanent_id, Some(copy));
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&garruk].counter_count(CounterKind::Loyalty),
        6
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn garruk_wolf_suppressed_at_death_has_no_trigger() {
    use tricerules_cards::{ContinuousEffectKind, EffectDuration};
    use tricerules_core::{AffectedScope, ContinuousEffect};
    let (mut engine, garruk) = setup(261_027_916);
    let wolf = create_wolves(&mut engine, garruk)[0];
    // This synthetic layer-6 effect isolates loss of the actual token's death ability.
    engine.state.continuous_effects.push(ContinuousEffect {
        source_id: None,
        affected: AffectedScope::Single(wolf),
        kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index + 1,
        trigger_grant_origin: None,
    });
    bolt_wolf(&mut engine, 0, wolf);
    assert!(!engine.state.objects.contains_key(&wolf));
    assert!(
        engine.state.stack.is_empty(),
        "the suppressed death ability cannot trigger"
    );
    assert_eq!(
        engine.state.objects[&garruk].counter_count(CounterKind::Loyalty),
        5
    );
}

#[test]
fn garruk_exiled_wolf_does_not_die_or_trigger() {
    let (mut engine, garruk) = setup(261_027_917);
    let wolf = create_wolves(&mut engine, garruk)[0];
    // Synthetic hand and mana; Swords actually exiles the token instead of destroying it.
    inject_card_into_hand(&mut engine, 0, "swords_to_plowshares");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "swords_to_plowshares");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(wolf)))
        .expect("cast Swords to Plowshares on the Wolf");
    pass_both_players(&mut engine);
    assert!(!engine.state.objects.contains_key(&wolf));
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.objects[&garruk].counter_count(CounterKind::Loyalty),
        5
    );
    assert_eq!(
        engine.state.players[0].life, 22,
        "the actual exile instruction completed"
    );
}

#[test]
fn garruk_wolf_trigger_checks_live_recipient_type_subtype_and_controller() {
    use tricerules_cards::{
        ContinuousEffectKind, ControllerReference, EffectDuration, PermanentTypeFilter,
        TypeLineReplacement,
    };
    use tricerules_core::{AffectedScope, ContinuousEffect};
    fn fixture_walker(engine: &mut GameEngine, owner: usize, name: &str, subtype: &str) -> u32 {
        let id = inject_permanent_on_battlefield(engine, owner, "jace,_unraveler_of_secrets");
        let mut face = tricerules_cards::registry::global()
            .get("jace,_unraveler_of_secrets")
            .unwrap()
            .primary_face()
            .clone();
        face.name = name.into();
        face.types = vec!["Planeswalker".into(), subtype.into()];
        let object = engine.state.objects.get_mut(&id).unwrap();
        object.copiable_values = Some(tricerules_core::state::CopiableValues {
            prototyped: false,
            source_card_id: "jace,_unraveler_of_secrets".into(),
            source_face_index: 0,
            face,
            room_faces: None,
            display_name: name.into(),
        });
        object.set_counter(CounterKind::Loyalty, 5);
        id
    }
    fn effect(engine: &mut GameEngine, object: u32, kind: ContinuousEffectKind) {
        engine.state.continuous_effects.push(ContinuousEffect {
            source_id: None,
            affected: AffectedScope::Single(object),
            kind,
            condition: None,
            duration: EffectDuration::Indefinite,
            timestamp: engine.state.command_index + 1,
            trigger_grant_origin: None,
        });
    }
    let (mut engine, garruk) = setup(261_027_918);
    let wolf = create_wolves(&mut engine, garruk)[0];
    // Distinct names, source faces and counters are synthetic, to isolate current characteristics.
    let loses_subtype = fixture_walker(&mut engine, 0, "Subtype loser", "Garruk");
    let loses_type = fixture_walker(&mut engine, 0, "Type loser", "Garruk");
    let gains_subtype = fixture_walker(&mut engine, 0, "Subtype gainer", "Jace");
    let gains_control = fixture_walker(&mut engine, 1, "Control gainer", "Garruk");
    bolt_wolf(&mut engine, 0, wolf);
    assert!(engine.state.stack.last().unwrap().is_triggered);

    // Changes happen after the actual death trigger is stacked and before it resolves.
    effect(
        &mut engine,
        garruk,
        ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(1),
        },
    );
    effect(
        &mut engine,
        gains_control,
        ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(0),
        },
    );
    effect(
        &mut engine,
        loses_type,
        ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
            card_types: vec![PermanentTypeFilter::Artifact],
            creature_types: vec![],
            land_types: vec![],
        }),
    );
    engine
        .state
        .objects
        .get_mut(&loses_subtype)
        .unwrap()
        .copiable_values
        .as_mut()
        .unwrap()
        .face
        .types = vec!["Planeswalker".into(), "Jace".into()];
    engine
        .state
        .objects
        .get_mut(&gains_subtype)
        .unwrap()
        .copiable_values
        .as_mut()
        .unwrap()
        .face
        .types = vec!["Planeswalker".into(), "Garruk".into()];
    assert_eq!(engine.characteristics(garruk).unwrap().controller, 1);
    assert_eq!(engine.characteristics(gains_control).unwrap().controller, 0);
    assert!(!engine
        .characteristics(loses_type)
        .unwrap()
        .has_type("Planeswalker"));
    assert!(!engine
        .characteristics(loses_subtype)
        .unwrap()
        .has_type("Garruk"));
    assert!(engine
        .characteristics(gains_subtype)
        .unwrap()
        .has_type("Garruk"));
    pass_both_players(&mut engine);
    for id in [garruk, loses_subtype, loses_type] {
        assert_eq!(
            engine.state.objects[&id].counter_count(CounterKind::Loyalty),
            5
        );
    }
    for id in [gains_control, gains_subtype] {
        assert_eq!(
            engine.state.objects[&id].counter_count(CounterKind::Loyalty),
            6
        );
    }
    assert!(engine.state.stack.is_empty());
}

#[test]
fn garruk_cannot_be_saved_by_a_simultaneous_wolf_death_trigger() {
    let (mut engine, garruk) = setup(261_027_919);
    let wolf = create_wolves(&mut engine, garruk)[0];
    // These synthetic marks cause the real next state-based action to see both events together.
    engine
        .state
        .objects
        .get_mut(&garruk)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 0);
    engine.state.objects.get_mut(&wolf).unwrap().damage = 2;
    let actor = engine.state.priority_player_id();
    engine
        .apply_scenario_command(actor, &pass())
        .expect("run the simultaneous state-based actions");
    assert_eq!(engine.state.objects[&garruk].zone, Zone::Graveyard);
    assert!(!engine.state.objects.contains_key(&wolf));
    assert!(engine
        .state
        .stack
        .last()
        .is_some_and(|item| item.is_triggered && item.source_permanent_id == Some(wolf)));
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&garruk].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&garruk].counter_count(CounterKind::Loyalty),
        0
    );
    assert!(engine.state.stack.is_empty());
}

type ParkedWolfBatch = (GameEngine, u32, u32, u32, Option<u32>);
type ParkedWolfBatchWithSecond = (GameEngine, u32, u32, u32, Option<u32>, Option<u32>);

fn parked_wolf_counter_batch(seed: u64, with_observer: bool) -> ParkedWolfBatch {
    let (engine, walker, wolf, stack_id, aura, _) =
        parked_wolf_counter_batch_with_second(seed, with_observer, false);
    (engine, walker, wolf, stack_id, aura)
}

fn parked_wolf_counter_batch_with_second(
    seed: u64,
    with_observer: bool,
    with_second: bool,
) -> ParkedWolfBatchWithSecond {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1, 2],
        20,
        Some(vec![
            deck_with("swamp", &[]),
            deck_with("forest", &[]),
            deck_with("island", &[]),
        ]),
        true,
    )
    .expect("create a three-player game");
    advance_to_main1_from_game_start(&mut engine);
    // Fixture cards and mana enter hands synthetically; every cast and its payment are real.
    fn cast(engine: &mut GameEngine, card: &str, mana: ManaGift, targets: Vec<TargetRef>) -> u32 {
        let id = inject_card_into_hand(engine, 0, card);
        give_mana(engine, 0, mana);
        let slot = hand_index_for_card(engine, 0, card);
        engine
            .apply_scenario_command(0, &cast_spell(slot, targets))
            .expect("cast fixture card");
        pass_priority_round(engine);
        id
    }
    let garruk = cast(
        &mut engine,
        GARRUK,
        ManaGift {
            c: 4,
            b: 1,
            g: 1,
            ..Default::default()
        },
        vec![],
    );
    apply_ability(
        &mut engine,
        0,
        garruk,
        ability_index("activated_01"),
        vec![],
    )
    .expect("activate actual Garruk's zero ability");
    pass_priority_round(&mut engine);
    let wolves = battlefield_token_oids(&engine, 0, WOLF);
    assert_eq!(wolves.len(), 2);
    let wolf = wolves[0];
    dev_move(
        &mut engine,
        0,
        "Garruk, Cursed Huntsman",
        DevZone::Graveyard,
        false,
    );
    assert_eq!(engine.state.objects[&garruk].zone, Zone::Graveyard);

    // The foreign permanent's placement, counters, distinct name and creature/planeswalker
    // characteristics are synthetic; they make real Act of Treason a legal way to borrow it.
    let walker = inject_permanent_on_battlefield(&mut engine, 1, "jace,_unraveler_of_secrets");
    let mut face = tricerules_cards::registry::global()
        .get("jace,_unraveler_of_secrets")
        .unwrap()
        .primary_face()
        .clone();
    face.name = "Borrowed Garruk Creature".into();
    face.types = vec!["Creature".into(), "Planeswalker".into(), "Garruk".into()];
    face.power = Some(2);
    face.toughness = Some(2);
    let object = engine.state.objects.get_mut(&walker).unwrap();
    object.copiable_values = Some(tricerules_core::state::CopiableValues {
        prototyped: false,
        source_card_id: "jace,_unraveler_of_secrets".into(),
        source_face_index: 0,
        face,
        room_faces: None,
        display_name: "Borrowed Garruk Creature".into(),
    });
    object.set_counter(CounterKind::Loyalty, 5);
    assert!(engine.characteristics(walker).unwrap().has_type("Garruk"));
    assert!(engine.characteristics(walker).unwrap().has_type("Creature"));
    let second = if with_second {
        // A second foreign-owned creature/planeswalker has the same labeled synthetic setup,
        // with a distinct name to avoid the legend rule after both are borrowed.
        let second = inject_permanent_on_battlefield(&mut engine, 2, "jace,_unraveler_of_secrets");
        let mut values = engine.state.objects[&walker]
            .copiable_values
            .clone()
            .unwrap();
        values.face.name = "Second Borrowed Garruk".into();
        values.display_name = "Second Borrowed Garruk".into();
        let object = engine.state.objects.get_mut(&second).unwrap();
        object.copiable_values = Some(values);
        object.set_counter(CounterKind::Loyalty, 5);
        Some(second)
    } else {
        None
    };

    let aura = if with_observer {
        use tricerules_core::AttachmentRecipient;
        // Initial Aura placement/attachment is synthetic. The real Banishing Light ETB creates
        // the independent observer that later suspends the already-begun counter resolution.
        let aura = inject_permanent_on_battlefield(&mut engine, 1, "pacifism");
        engine.state.objects.get_mut(&aura).unwrap().attached_to =
            Some(AttachmentRecipient::Object(walker));
        let light = cast(
            &mut engine,
            "banishing_light",
            ManaGift {
                c: 2,
                w: 1,
                ..Default::default()
            },
            vec![],
        );
        assert_eq!(engine.state.objects[&light].zone, Zone::Battlefield);
        engine
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                        decline: false,
                        selected_modes: Vec::new(),
                        targets: target_object(aura),
                    })),
                },
            )
            .expect("choose the foreign Aura for Banishing Light");
        pass_priority_round(&mut engine);
        assert_eq!(engine.state.objects[&aura].zone, Zone::Exile);
        assert_eq!(engine.state.active_event_observers.len(), 1);
        Some(aura)
    } else {
        None
    };
    cast(
        &mut engine,
        "act_of_treason",
        ManaGift {
            c: 2,
            r: 1,
            ..Default::default()
        },
        target_object(walker),
    );
    assert_eq!(engine.characteristics(walker).unwrap().controller, 0);
    if let Some(second) = second {
        cast(
            &mut engine,
            "act_of_treason",
            ManaGift {
                c: 2,
                r: 1,
                ..Default::default()
            },
            target_object(second),
        );
        assert_eq!(engine.characteristics(second).unwrap().controller, 0);
    }
    let laezel = cast(
        &mut engine,
        "laezel,_vlaakiths_champion",
        ManaGift {
            c: 2,
            w: 1,
            ..Default::default()
        },
        vec![],
    );
    let first_season = cast(
        &mut engine,
        "doubling_season",
        ManaGift {
            c: 4,
            g: 1,
            ..Default::default()
        },
        vec![],
    );
    let second_season = cast(
        &mut engine,
        "doubling_season",
        ManaGift {
            c: 4,
            g: 1,
            ..Default::default()
        },
        vec![],
    );
    cast(
        &mut engine,
        "lightning_bolt",
        ManaGift {
            r: 1,
            ..Default::default()
        },
        target_object(wolf),
    );
    assert!(
        !engine.state.objects.contains_key(&wolf),
        "the dying Wolf ceased to exist"
    );
    let trigger = engine.state.stack.last().expect("captured Wolf trigger");
    assert!(trigger.is_triggered && trigger.triggered_ability.is_some());
    assert_eq!(
        (trigger.source_permanent_id, trigger.controller),
        (Some(wolf), 0)
    );
    let stack_id = trigger.id;
    let mut batch = None;
    for _ in 0..3 {
        let actor = engine.state.priority_player_id();
        batch = Some(
            engine
                .apply_scenario_command(actor, &pass())
                .expect("pass priority"),
        );
    }
    let choice = find_resolution_choice(&batch.unwrap()).expect("replacement order for the walker");
    let snapshot = engine.diagnostic_snapshot().unwrap();
    let counter_batch = &snapshot["state"]["pending_replacement_event"];
    assert_eq!(counter_batch["variant"], "Counters");
    assert_eq!(counter_batch["value"]["event_index"].as_str(), Some("0"));
    assert_eq!(
        counter_batch["value"]["events"][0]["recipient"]["variant"],
        "Object"
    );
    assert_eq!(
        counter_batch["value"]["events"][0]["recipient"]["value"]["object_id"].as_u64(),
        Some(walker as u64),
        "the first replacement belongs to the first foreign recipient"
    );
    assert_eq!(choice.replacement_options.len(), 3);
    let selected = choice
        .replacement_options
        .iter()
        .find(|option| option.source_object_id == laezel)
        .expect("Lae'zel is offered")
        .application_id;
    let next = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![selected]))
        .expect("apply Lae'zel before both Seasons");
    let remaining = find_resolution_choice(&next).expect("the Seasons still need ordering");
    assert_eq!(remaining.replacement_options.len(), 2);
    assert!(remaining
        .replacement_options
        .iter()
        .all(|option| option.source_object_id == first_season
            || option.source_object_id == second_season));
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        5
    );
    (engine, walker, wolf, stack_id, aura, second)
}

#[test]
fn garruk_wolf_begun_counters_survive_trigger_controller_departure() {
    let (mut engine, walker, wolf, stack_id, _) = parked_wolf_counter_batch(261_027_920, false);
    let generation = engine
        .state
        .zone_change_generation
        .get(&walker)
        .copied()
        .unwrap_or(0);
    let batch = engine
        .apply_scenario_command(0, &concede())
        .expect("trigger controller leaves");
    assert!(engine.state.players[0].has_lost);
    assert!(!engine.state.objects.contains_key(&wolf));
    assert_eq!(engine.state.objects[&walker].owner, 1);
    assert_eq!(engine.state.objects[&walker].zone, Zone::Battlefield);
    assert_eq!(engine.characteristics(walker).unwrap().controller, 1);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&walker)
            .copied()
            .unwrap_or(0),
        generation
    );
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        7,
        "the accepted Lae'zel adjustment survives departure"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.diagnostic_snapshot().unwrap()["state"]["pending_replacement_event"].is_null());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == stack_id))
            .count(),
        1
    );
}

#[test]
fn garruk_wolf_restores_begun_counters_after_independent_aura_return() {
    use tricerules_core::AttachmentRecipient;
    let (mut engine, walker, wolf, stack_id, aura) = parked_wolf_counter_batch(261_027_921, true);
    let aura = aura.expect("Banishing Light's actual ETB established the Aura observer");
    let generation = engine
        .state
        .zone_change_generation
        .get(&walker)
        .copied()
        .unwrap_or(0);
    let departure = engine
        .apply_scenario_command(0, &concede())
        .expect("trigger controller leaves");
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Aura return is parked");
    assert_eq!(pending.deciding_player, 1);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::AuraPermanent);
    assert!(pending.presentation.candidates.contains(&walker));
    assert_eq!(engine.state.objects[&aura].zone, Zone::Exile);
    assert!(!engine.state.objects.contains_key(&wolf));
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        5
    );
    assert!(
        engine.diagnostic_snapshot().unwrap()["state"]["pending_observer_return_batch"].is_object(),
        "the begun Wolf trigger is suspended beneath the independent Aura choice"
    );
    assert!(!departure.events.iter().any(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == stack_id)));

    let returned = engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![walker]))
        .expect("the owner attaches the Aura and restores the Wolf trigger");
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(walker))
    );
    assert_eq!(engine.state.objects[&walker].owner, 1);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&walker)
            .copied()
            .unwrap_or(0),
        generation
    );
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        7,
        "the restored event retains the accepted Lae'zel adjustment"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.diagnostic_snapshot().unwrap()["state"]["pending_replacement_event"].is_null());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        returned
            .events
            .iter()
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == stack_id))
            .count(),
        1
    );
}

#[test]
fn garruk_wolf_begun_counters_retire_when_the_recipient_owner_leaves() {
    let (mut engine, walker, _, stack_id, _) = parked_wolf_counter_batch(261_027_922, false);
    let batch = engine
        .apply_scenario_command(1, &concede())
        .expect("recipient owner leaves");
    assert!(engine.state.players[1].has_lost);
    assert!(!engine.state.objects.contains_key(&walker));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.diagnostic_snapshot().unwrap()["state"]["pending_replacement_event"].is_null());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == stack_id))
            .count(),
        1
    );
    let survivor = engine.state.priority_player_id();
    assert_ne!(survivor, 1);
    engine
        .apply_scenario_command(survivor, &pass())
        .expect("a surviving player can continue");
}

#[test]
fn garruk_wolf_suspended_counters_prune_a_stale_incarnation_but_finish_the_other() {
    let (mut engine, stale, _, stack_id, aura, second) =
        parked_wolf_counter_batch_with_second(261_027_923, true, true);
    let aura = aura.unwrap();
    let second = second.unwrap();
    let old_generation = engine
        .state
        .zone_change_generation
        .get(&stale)
        .copied()
        .unwrap_or(0);
    let other_generation = engine
        .state
        .zone_change_generation
        .get(&second)
        .copied()
        .unwrap_or(0);
    let departure = engine
        .apply_scenario_command(0, &concede())
        .expect("trigger controller leaves");
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Aura return is parked");
    assert_eq!(pending.deciding_player, 1);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::AuraPermanent);
    assert!(pending.presentation.candidates.contains(&second));
    assert!(
        engine.diagnostic_snapshot().unwrap()["state"]["pending_observer_return_batch"].is_object()
    );
    assert!(!departure.events.iter().any(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == stack_id)));

    // Dev zone moves are rejected during a parked choice. This synthetic same-OID generation
    // change models a new incarnation while preserving another exact original batch member.
    *engine
        .state
        .zone_change_generation
        .entry(stale)
        .or_default() += 2;
    engine
        .state
        .objects
        .get_mut(&stale)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 5);
    assert!(engine.state.zone_change_generation[&stale] > old_generation);
    let completed = engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![second]))
        .expect("the Aura owner chooses the surviving other creature");
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&stale].counter_count(CounterKind::Loyalty),
        5,
        "the saved proposal cannot affect a newer incarnation with the same object ID"
    );
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&second)
            .copied()
            .unwrap_or(0),
        other_generation
    );
    assert_eq!(
        engine.state.objects[&second].counter_count(CounterKind::Loyalty),
        6,
        "the other original recipient receives its one unmodified loyalty counter"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.diagnostic_snapshot().unwrap()["state"]["pending_replacement_event"].is_null());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        completed
            .events
            .iter()
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == stack_id))
            .count(),
        1
    );
}

fn create_garruk_emblem(engine: &mut GameEngine, garruk: u32) -> u32 {
    let previous: Vec<_> = engine
        .state
        .static_emblems
        .iter()
        .map(|emblem| emblem.object_id)
        .collect();
    // Synthetic loyalty avoids waiting multiple turns; the -6 cost and ultimate are real.
    engine
        .state
        .objects
        .get_mut(&garruk)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 6);
    apply_ability(engine, 0, garruk, ability_index("activated_03"), vec![])
        .expect("activate Garruk's actual -6 ability");
    assert_eq!(
        engine.state.objects[&garruk].zone,
        Zone::Graveyard,
        "zero loyalty moves Garruk before his ability resolves"
    );
    pass_priority_round(engine);
    assert!(engine.state.stack.is_empty());
    engine
        .state
        .static_emblems
        .iter()
        .find(|emblem| emblem.controller == 0 && !previous.contains(&emblem.object_id))
        .expect("the resolved ability creates an emblem")
        .object_id
}

#[test]
fn garruk_emblem_survives_its_source_and_affects_current_and_later_creatures() {
    let (mut engine, garruk) = setup(261_027_930);
    // Initial creatures are synthetic placements; the later creature uses an actual cast.
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let marker = create_garruk_emblem(&mut engine, garruk);
    assert!(engine.initial_response_batch().events.iter().any(|event| {
        matches!(&event.ev, Some(Ev::ZoneView(view)) if view.per_player.iter().any(|player|
            player.player_id == 0 && player.static_emblems.iter().any(|emblem| emblem.object_id == marker)))
    }), "the source-independent emblem has a public marker");
    let own_characteristics = engine.characteristics(own).unwrap();
    assert_eq!(
        (own_characteristics.power, own_characteristics.toughness),
        (Some(5), Some(5))
    );
    assert!(own_characteristics.has_keyword(Keyword::Trample));
    let other_characteristics = engine.characteristics(opponent).unwrap();
    assert_eq!(
        (other_characteristics.power, other_characteristics.toughness),
        (Some(2), Some(2))
    );
    assert!(!other_characteristics.has_keyword(Keyword::Trample));

    inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            g: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "grizzly_bears");
    let later = engine.state.players[0].hand[slot];
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast later Bears");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&later].zone, Zone::Battlefield);
    let characteristics = engine.characteristics(later).unwrap();
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(5), Some(5))
    );
    assert!(characteristics.has_keyword(Keyword::Trample));
}

#[test]
fn garruk_emblem_tracks_current_control_type_and_layer_six_order() {
    use tricerules_cards::{
        ContinuousEffectKind, ControllerReference, EffectDuration, PermanentTypeFilter,
        TypeLineReplacement,
    };
    use tricerules_core::{AffectedScope, ContinuousEffect};
    fn effect(engine: &mut GameEngine, object: u32, kind: ContinuousEffectKind) {
        engine.state.continuous_effects.push(ContinuousEffect {
            source_id: None,
            affected: AffectedScope::Single(object),
            kind,
            condition: None,
            duration: EffectDuration::Indefinite,
            timestamp: engine.state.command_index + 1,
            trigger_grant_origin: None,
        });
    }
    let (mut engine, garruk) = setup(261_027_931);
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let other = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    create_garruk_emblem(&mut engine, garruk);
    // These synthetic layer effects isolate the emblem's live scope and timestamp ordering.
    effect(
        &mut engine,
        other,
        ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(0),
        },
    );
    let controlled = engine.characteristics(other).unwrap();
    assert_eq!(controlled.controller, 0);
    assert_eq!((controlled.power, controlled.toughness), (Some(5), Some(5)));
    assert!(controlled.has_keyword(Keyword::Trample));
    engine.state.continuous_effects.retain(|entry| {
        !matches!(
            entry.kind,
            ContinuousEffectKind::Layer2Control {
                controller: ControllerReference::Fixed(0)
            }
        )
    });
    assert_eq!(engine.characteristics(other).unwrap().controller, 1);
    assert!(!engine
        .characteristics(other)
        .unwrap()
        .has_keyword(Keyword::Trample));

    effect(
        &mut engine,
        own,
        ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
            card_types: vec![PermanentTypeFilter::Artifact],
            creature_types: vec![],
            land_types: vec![],
        }),
    );
    let noncreature = engine.characteristics(own).unwrap();
    assert!(!noncreature.is_creature());
    assert_eq!((noncreature.power, noncreature.toughness), (None, None));
    assert!(!noncreature.has_keyword(Keyword::Trample));
    engine
        .state
        .continuous_effects
        .retain(|entry| !matches!(entry.kind, ContinuousEffectKind::Layer4SetTypeLine(_)));
    assert!(engine
        .characteristics(own)
        .unwrap()
        .has_keyword(Keyword::Trample));

    effect(
        &mut engine,
        own,
        ContinuousEffectKind::Layer6RemoveAllAbilities,
    );
    let suppressed = engine.characteristics(own).unwrap();
    assert_eq!((suppressed.power, suppressed.toughness), (Some(5), Some(5)));
    assert!(!suppressed.has_keyword(Keyword::Trample));
    engine
        .state
        .continuous_effects
        .retain(|entry| !matches!(entry.kind, ContinuousEffectKind::Layer6RemoveAllAbilities));
    assert!(engine
        .characteristics(own)
        .unwrap()
        .has_keyword(Keyword::Trample));
}

#[test]
fn garruk_emblems_stack_their_modifiers_without_duplicate_trample() {
    let (mut engine, first) = setup(261_027_932);
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let first_marker = create_garruk_emblem(&mut engine, first);
    // The second hand card and mana are synthetic; its actual cast and -6 are accepted.
    let second = inject_card_into_hand(&mut engine, 0, GARRUK);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 4,
            b: 1,
            g: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, GARRUK);
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast a second Garruk");
    pass_both_players(&mut engine);
    let second_marker = create_garruk_emblem(&mut engine, second);
    assert_ne!(first_marker, second_marker);
    let characteristics = engine.characteristics(bear).unwrap();
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(8), Some(8))
    );
    assert_eq!(
        characteristics
            .keywords
            .iter()
            .filter(|keyword| **keyword == Keyword::Trample)
            .count(),
        1
    );
    assert_eq!(
        engine
            .state
            .static_emblems
            .iter()
            .filter(|emblem| emblem.controller == 0)
            .count(),
        2
    );
}

#[test]
fn garruk_emblem_and_its_effects_leave_with_their_controller() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        261_027_933,
        &[0, 1, 2],
        20,
        Some(vec![
            deck_with("swamp", &[]),
            deck_with("forest", &[]),
            deck_with("island", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let garruk = inject_card_into_hand(&mut engine, 0, GARRUK);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 4,
            b: 1,
            g: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, GARRUK);
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Garruk");
    pass_priority_round(&mut engine);
    let marker = create_garruk_emblem(&mut engine, garruk);
    let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    // The opponent's creature, hand card and mana are synthetic; Act of Treason is real.
    inject_card_into_hand(&mut engine, 0, "act_of_treason");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "act_of_treason");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(bear)))
        .expect("borrow the opposing creature");
    pass_priority_round(&mut engine);
    assert_eq!(engine.characteristics(bear).unwrap().controller, 0);
    assert_eq!(engine.characteristics(bear).unwrap().power, Some(5));
    assert!(engine
        .characteristics(bear)
        .unwrap()
        .has_keyword(Keyword::Trample));
    engine
        .apply_scenario_command(0, &concede())
        .expect("emblem controller leaves");
    assert!(engine.state.players[0].has_lost);
    assert!(!engine
        .state
        .static_emblems
        .iter()
        .any(|emblem| emblem.object_id == marker));
    assert!(!engine
        .state
        .continuous_effects
        .iter()
        .any(|effect| effect.source_id == Some(marker)));
    assert_eq!(engine.characteristics(bear).unwrap().controller, 1);
    assert_eq!(
        (
            engine.characteristics(bear).unwrap().power,
            engine.characteristics(bear).unwrap().toughness
        ),
        (Some(2), Some(2))
    );
    assert!(!engine
        .characteristics(bear)
        .unwrap()
        .has_keyword(Keyword::Trample));
}

#[test]
fn garruk_emblem_grants_trample_after_an_older_ability_removal() {
    use tricerules_cards::{ContinuousEffectKind, EffectDuration};
    use tricerules_core::{AffectedScope, ContinuousEffect};
    let (mut engine, garruk) = setup(261_027_934);
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    // This older synthetic layer-6 removal remains active as the real emblem is created later.
    engine.state.continuous_effects.push(ContinuousEffect {
        source_id: None,
        affected: AffectedScope::Single(bear),
        kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
        trigger_grant_origin: None,
    });
    create_garruk_emblem(&mut engine, garruk);
    assert!(engine
        .state
        .continuous_effects
        .iter()
        .any(|effect| matches!(effect.kind, ContinuousEffectKind::Layer6RemoveAllAbilities)));
    let characteristics = engine.characteristics(bear).unwrap();
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(5), Some(5))
    );
    assert!(
        characteristics.has_keyword(Keyword::Trample),
        "the newer emblem grants trample after the older removal"
    );
}

#[test]
fn garruk_emblem_grants_real_blocked_trample_damage() {
    use tricerules_core::TurnStep;
    let (mut engine, garruk) = setup(261_027_935);
    // Both Bears are synthetic placements; combat commands and damage are actual game actions.
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    create_garruk_emblem(&mut engine, garruk);
    assert_eq!(engine.characteristics(attacker).unwrap().power, Some(5));
    assert!(engine
        .characteristics(attacker)
        .unwrap()
        .has_keyword(Keyword::Trample));
    engine
        .apply_scenario_command(0, &primitive_yield())
        .expect("advance to combat");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
    engine
        .apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("attack");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .expect("block the creature");
    assert!(
        engine
            .state
            .combat
            .as_ref()
            .unwrap()
            .damage_assignment_needed
    );
    pass_both_players(&mut engine);
    assert!(
        engine
            .state
            .combat
            .as_ref()
            .unwrap()
            .assign_combat_damage_phase
    );
    engine
        .apply_scenario_command(
            0,
            &assign_combat_damage_cmd_with_player(attacker, vec![(blocker, 2)], 3),
        )
        .expect("assign lethal damage and trample excess");
    assert_eq!(engine.state.objects[&blocker].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[1].life, 17);
    assert_eq!(engine.state.objects[&attacker].damage, 2);
}

#[test]
fn garruk_minus_three_draw_updates_only_its_server_private_hand_projection() {
    let (mut engine, garruk) = setup(261_027_936);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let drawn = *engine.state.players[0].library.front().unwrap();
    let card_id = engine.state.objects[&drawn].card_id.clone();
    let card_name = tricerules_cards::registry::global()
        .get(&card_id)
        .unwrap()
        .name
        .clone();
    activate_minus_three(&mut engine, garruk, target_object(target)).expect("activate the real -3");
    let batch = pass_both_players_with_batch(&mut engine);
    let view = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("draw synchronizes server-only hand data");
    let recipient = view
        .per_player
        .iter()
        .find(|player| player.player_id == 0)
        .unwrap();
    assert!(recipient
        .hand_cards
        .iter()
        .any(|card| card.object_id == drawn && card.card_id == card_id));
    assert!(view
        .per_player
        .iter()
        .filter(|player| player.player_id != 0)
        .all(|player| player.hand_cards.iter().all(|card| card.object_id != drawn)));
    assert!(
        batch.events.iter().all(|event| match &event.ev {
            Some(Ev::PermanentMoved(moved)) => moved.object_id != drawn,
            Some(Ev::Log(log)) => !log.text.contains(&card_name),
            _ => true,
        }),
        "the engine does not publish the concealed draw in a movement or public log"
    );
}

#[test]
fn garruk_all_three_abilities_replay_as_accepted_wire_commands() {
    use prost::Message;
    use tricerules_proto::ruled::v1::{DevAddMana, DevPutCardInZone};
    type ReplayLog = Vec<(i32, RuledCommand, RuledEventBatch)>;
    fn fresh() -> GameEngine {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            261_027_937,
            &[0, 1],
            20,
            Some(vec![deck_with("swamp", &[]), deck_with("forest", &[])]),
            true,
        )
        .expect("create replay fixture");
        advance_to_main1_from_game_start(&mut engine);
        engine.enable_dev_commands();
        engine
    }
    fn record(
        engine: &mut GameEngine,
        commands: &mut ReplayLog,
        actor: i32,
        command: RuledCommand,
    ) {
        let decoded = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
        let batch = engine
            .apply_scenario_command(actor, &decoded)
            .expect("accepted wire command");
        commands.push((actor, decoded, batch));
    }
    fn drain(engine: &mut GameEngine, commands: &mut ReplayLog) {
        for _ in 0..64 {
            if let Some(pending) = engine.state.pending_resolution.as_ref() {
                assert_eq!(
                    pending.presentation.choice_kind,
                    ChoiceKind::SimultaneousEntryOrder
                );
                let actor = pending.deciding_player;
                let choice = submit_resolution_choice(pending.presentation.candidates.clone());
                record(engine, commands, actor, choice);
            } else if let Some(order) = engine.state.pending_trigger_order.as_ref() {
                let actor = order.deciding_player;
                let choice = submit_trigger_order(order.candidates[0].object_id);
                record(engine, commands, actor, choice);
            } else if engine.state.stack.is_empty() && engine.state.blocking_choice().is_none() {
                return;
            } else {
                assert!(
                    engine.state.blocking_choice().is_none(),
                    "unexpected replay choice"
                );
                let actor = engine.state.priority_player_id();
                record(engine, commands, actor, pass());
            }
        }
        panic!("replay resolution exceeded 64 commands");
    }
    let dev = |target, payload| RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: target,
            dev: Some(payload),
        })),
    };
    let put = |target, name: &str, zone| {
        dev(
            target,
            dev_command::Dev::PutCardInZone(DevPutCardInZone {
                card_name: name.into(),
                zone: zone as i32,
                ready: true,
            }),
        )
    };
    let garruk_mana = || {
        dev(
            0,
            dev_command::Dev::AddMana(DevAddMana {
                c: 4,
                b: 1,
                g: 1,
                ..Default::default()
            }),
        )
    };
    let move_garruk = || {
        dev(
            0,
            dev_command::Dev::MoveCard(DevMoveCard {
                card_name: "Garruk, Cursed Huntsman".into(),
                zone: DevZone::Graveyard as i32,
                ready: false,
            }),
        )
    };
    let mut engine = fresh();
    let mut commands = Vec::new();

    // Dev commands provide cards and mana but are themselves recorded and replayed. Every
    // Garruk cast/activation, Wolf death, draw and emblem resolution is an actual command path.
    record(
        &mut engine,
        &mut commands,
        0,
        put(0, "Garruk, Cursed Huntsman", DevZone::Hand),
    );
    record(&mut engine, &mut commands, 0, garruk_mana());
    let slot = hand_index_for_card(&engine, 0, GARRUK);
    let first = engine.state.players[0].hand[slot];
    record(&mut engine, &mut commands, 0, cast_spell(slot, vec![]));
    drain(&mut engine, &mut commands);
    let zero = activate_ability_for(&engine, first, ability_index("activated_01"), vec![]);
    record(&mut engine, &mut commands, 0, zero);
    drain(&mut engine, &mut commands);
    let wolves = battlefield_token_oids(&engine, 0, WOLF);
    assert_eq!(wolves.len(), 2);
    record(&mut engine, &mut commands, 0, move_garruk());
    assert_eq!(engine.state.objects[&first].zone, Zone::Graveyard);

    record(
        &mut engine,
        &mut commands,
        0,
        put(0, "Garruk, Cursed Huntsman", DevZone::Hand),
    );
    record(&mut engine, &mut commands, 0, garruk_mana());
    let slot = hand_index_for_card(&engine, 0, GARRUK);
    let second = engine.state.players[0].hand[slot];
    record(&mut engine, &mut commands, 0, cast_spell(slot, vec![]));
    drain(&mut engine, &mut commands);
    record(
        &mut engine,
        &mut commands,
        0,
        put(1, "Grizzly Bears", DevZone::Battlefield),
    );
    let target = *engine.state.players[1].battlefield.last().unwrap();
    let top = *engine.state.players[0].library.front().unwrap();
    let minus_three = activate_ability_for(
        &engine,
        second,
        ability_index("activated_02"),
        target_object(target),
    );
    record(&mut engine, &mut commands, 0, minus_three);
    drain(&mut engine, &mut commands);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert!(engine.state.players[0].hand.contains(&top));
    record(&mut engine, &mut commands, 0, move_garruk());
    assert_eq!(engine.state.objects[&second].zone, Zone::Graveyard);

    record(
        &mut engine,
        &mut commands,
        0,
        put(0, "Garruk, Cursed Huntsman", DevZone::Hand),
    );
    record(&mut engine, &mut commands, 0, garruk_mana());
    let slot = hand_index_for_card(&engine, 0, GARRUK);
    let third = engine.state.players[0].hand[slot];
    record(&mut engine, &mut commands, 0, cast_spell(slot, vec![]));
    drain(&mut engine, &mut commands);
    record(
        &mut engine,
        &mut commands,
        0,
        put(0, "Lightning Bolt", DevZone::Hand),
    );
    record(
        &mut engine,
        &mut commands,
        0,
        dev(
            0,
            dev_command::Dev::AddMana(DevAddMana {
                r: 1,
                ..Default::default()
            }),
        ),
    );
    let slot = hand_index_for_card(&engine, 0, "lightning_bolt");
    record(
        &mut engine,
        &mut commands,
        0,
        cast_spell(slot, target_object(wolves[0])),
    );
    drain(&mut engine, &mut commands);
    assert_eq!(
        engine.state.objects[&third].counter_count(CounterKind::Loyalty),
        6
    );
    let minus_six = activate_ability_for(&engine, third, ability_index("activated_03"), vec![]);
    record(&mut engine, &mut commands, 0, minus_six);
    drain(&mut engine, &mut commands);
    assert_eq!(engine.state.objects[&third].zone, Zone::Graveyard);
    assert_eq!(
        engine
            .state
            .static_emblems
            .iter()
            .filter(|emblem| emblem.controller == 0)
            .count(),
        1
    );
    assert_eq!(engine.characteristics(wolves[1]).unwrap().power, Some(5));
    assert!(engine
        .characteristics(wolves[1])
        .unwrap()
        .has_keyword(Keyword::Trample));

    let mut replay = fresh();
    for (actor, command, expected) in commands {
        let decoded = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
        assert_eq!(
            replay.apply_scenario_command(actor, &decoded).unwrap(),
            expected
        );
    }
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
    let actor = engine.state.priority_player_id();
    let next = pass();
    let decoded = RuledCommand::decode(next.encode_to_vec().as_slice()).unwrap();
    assert_eq!(
        engine.apply_scenario_command(actor, &next).unwrap(),
        replay.apply_scenario_command(actor, &decoded).unwrap()
    );
}
