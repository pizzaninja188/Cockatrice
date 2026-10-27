//! Actual Ajani Steadfast cast and +1 commands, with labeled synthetic setup cards and mana.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::{CounterKind, Keyword};
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

const AJANI: &str = "ajani_steadfast";

fn game(seed: u64) -> GameEngine {
    let registry = tricerules_cards::registry::global();
    assert!(
        registry.get(AJANI).is_some(),
        "Ajani Steadfast must be registered"
    );
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("plains", &[]), deck_with("forest", &[])]),
        true,
    )
    .expect("create two-player game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_ajani(engine: &mut GameEngine) -> u32 {
    let ajani = inject_card_into_hand(engine, 0, AJANI);
    give_mana(
        engine,
        0,
        ManaGift {
            c: 3,
            w: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, AJANI);
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast actual Ajani for {3}{W}");
    assert_eq!(
        (
            engine.state.players[0].mana_pool.colorless,
            engine.state.players[0].mana_pool.white
        ),
        (0, 0)
    );
    pass_both_players(engine);
    assert_eq!(engine.state.objects[&ajani].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&ajani].counter_count(CounterKind::Loyalty),
        4
    );
    ajani
}

fn cast_fixture_permanent(engine: &mut GameEngine, card_id: &str, mana: ManaGift) -> u32 {
    // The card and mana are synthetic setup; its cast, payment and entry are real commands.
    let object = inject_card_into_hand(engine, 0, card_id);
    give_mana(engine, 0, mana);
    let slot = hand_index_for_card(engine, 0, card_id);
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast the fixture permanent");
    pass_both_players(engine);
    assert_eq!(engine.state.objects[&object].zone, Zone::Battlefield);
    object
}

fn animate_planeswalkers_with_lattice_and_bello(engine: &mut GameEngine) {
    cast_fixture_permanent(
        engine,
        "mycosynth_lattice",
        ManaGift {
            c: 6,
            ..Default::default()
        },
    );
    cast_fixture_permanent(
        engine,
        "bello,_bard_of_the_brambles",
        ManaGift {
            c: 1,
            r: 1,
            g: 1,
            ..Default::default()
        },
    );
}

fn assert_creature_planeswalker(engine: &GameEngine, object: u32) {
    let characteristics = engine
        .characteristics(object)
        .expect("permanent characteristics");
    assert!(characteristics.is_creature());
    assert!(characteristics.has_type("Planeswalker"));
}

fn has_ajani_bonus(engine: &GameEngine, creature: u32) -> bool {
    let Some(stats) = engine.characteristics(creature) else {
        return false;
    };
    stats.power == Some(3)
        && stats.toughness == Some(3)
        && [Keyword::FirstStrike, Keyword::Vigilance, Keyword::Lifelink]
            .into_iter()
            .all(|keyword| stats.has_keyword(keyword))
}

fn assert_vanilla_bears(engine: &GameEngine, creature: u32) {
    let stats = engine
        .characteristics(creature)
        .expect("the creature remains present");
    assert_eq!((stats.power, stats.toughness), (Some(2), Some(2)));
    assert!(
        [Keyword::FirstStrike, Keyword::Vigilance, Keyword::Lifelink]
            .into_iter()
            .all(|keyword| !stats.has_keyword(keyword))
    );
}

fn move_ajani_to_graveyard(engine: &mut GameEngine) {
    engine.enable_dev_commands();
    let actor = engine.state.priority_player_id();
    engine
        .apply_scenario_command(
            actor,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Ajani Steadfast".into(),
                        zone: DevZone::Graveyard as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move the physical Ajani through the engine");
}

#[test]
fn ajani_plus_one_accepts_zero_targets_and_pays_loyalty() {
    let mut engine = game(261_027_800);
    let creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let ajani = cast_ajani(&mut engine);
    apply_ability(&mut engine, 0, ajani, 0, vec![]).expect("choose zero targets");
    assert_eq!(
        engine.state.objects[&ajani].counter_count(CounterKind::Loyalty),
        5
    );
    assert!(engine.state.stack.last().unwrap().targets.is_empty());
    pass_both_players(&mut engine);
    assert_vanilla_bears(&engine, creature);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn ajani_plus_one_buff_applies_to_either_players_creature_and_survives_source_departure() {
    for owner in [0, 1] {
        let mut engine = game(261_027_801 + owner as u64);
        // Synthetic placement supplies a vanilla 2/2 without asserting a creature cast.
        let creature = inject_creature_on_battlefield(&mut engine, owner, "grizzly_bears");
        let ajani = cast_ajani(&mut engine);
        apply_ability(&mut engine, 0, ajani, 0, target_object(creature))
            .expect("target the creature");
        assert_eq!(
            engine.state.objects[&ajani].counter_count(CounterKind::Loyalty),
            5
        );
        if owner == 1 {
            move_ajani_to_graveyard(&mut engine);
        }
        pass_both_players(&mut engine);
        assert!(has_ajani_bonus(&engine, creature));
        if owner == 1 {
            assert_eq!(engine.state.objects[&ajani].zone, Zone::Graveyard);
        }
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn ajani_plus_one_rejects_noncreature_and_does_not_buff_a_stale_incarnation() {
    let mut engine = game(261_027_803);
    let creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    let ajani = cast_ajani(&mut engine);
    apply_ability(&mut engine, 0, ajani, 0, target_object(land))
        .expect_err("a land is not a legal creature target");
    assert_eq!(
        engine.state.objects[&ajani].counter_count(CounterKind::Loyalty),
        4
    );
    apply_ability(&mut engine, 0, ajani, 0, target_object(creature))
        .expect("choose a legal target");
    // Synthetic generation change models the creature leaving and returning before resolution.
    *engine
        .state
        .zone_change_generation
        .entry(creature)
        .or_default() += 2;
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Battlefield);
    assert_vanilla_bears(&engine, creature);
    assert_eq!(
        engine.state.objects[&ajani].counter_count(CounterKind::Loyalty),
        5
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn ajani_plus_one_keywords_affect_combat_then_expire_at_cleanup() {
    let mut engine = game(261_027_804);
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let ajani = cast_ajani(&mut engine);
    apply_ability(&mut engine, 0, ajani, 0, target_object(attacker)).expect("target own creature");
    pass_both_players(&mut engine);
    assert!(has_ajani_bonus(&engine, attacker));
    let starting_turn = engine.state.turn_instance;
    let life = engine.state.players[0].life;

    engine
        .apply_scenario_command(0, &primitive_yield())
        .expect("advance to beginning of combat");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
    engine
        .apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("attack with vigilance");
    assert!(
        !engine.state.objects[&attacker].tapped,
        "vigilance prevents tapping to attack"
    );
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .expect("block the attacker");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::FirstStrikeDamage);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&blocker].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&attacker].damage, 0,
        "first strike removes the blocker first"
    );
    assert_eq!(
        engine.state.players[0].life,
        life + 3,
        "lifelink follows the actual damage dealt"
    );

    for _ in 0..100 {
        if engine.state.turn_instance > starting_turn {
            break;
        }
        let (actor, command) = match engine.state.cleanup_discard_player {
            Some(player) => {
                let index = engine.state.player_idx(player).unwrap();
                let excess = engine.state.players[index].hand.len() - 7;
                (player, discard_cleanup_batch((0..excess as u32).collect()))
            }
            None => (engine.state.priority_player_id(), pass()),
        };
        engine
            .apply_scenario_command(actor, &command)
            .expect("pass through actual cleanup");
    }
    assert!(engine.state.turn_instance > starting_turn);
    assert_vanilla_bears(&engine, attacker);
}

#[test]
fn ajani_minus_two_uses_current_control_and_excludes_opponents() {
    let mut engine = game(261_027_810);
    let ajani = cast_ajani(&mut engine);
    let jace = cast_fixture_permanent(
        &mut engine,
        "jace,_unraveler_of_secrets",
        ManaGift {
            c: 3,
            u: 2,
            ..Default::default()
        },
    );
    // These synthetic placements distinguish ownership from current control.
    let borrowed = inject_creature_under_foreign_control(&mut engine, 1, 0, "grizzly_bears");
    let lent = inject_creature_under_foreign_control(&mut engine, 0, 1, "grizzly_bears");
    let opposing_walker =
        inject_permanent_on_battlefield(&mut engine, 1, "jace,_unraveler_of_secrets");
    engine
        .state
        .objects
        .get_mut(&opposing_walker)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 5);

    apply_ability(&mut engine, 0, ajani, 1, vec![]).expect("activate the actual -2 ability");
    assert_eq!(
        engine.state.objects[&ajani].counter_count(CounterKind::Loyalty),
        2
    );
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&borrowed].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(
        engine.state.objects[&lent].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        6
    );
    assert_eq!(
        engine.state.objects[&opposing_walker].counter_count(CounterKind::Loyalty),
        5
    );
    assert_eq!(
        engine.state.objects[&ajani].counter_count(CounterKind::Loyalty),
        2
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn ajani_minus_two_combines_both_counter_kinds_before_replacement_ordering() {
    for laezel_first in [true, false] {
        let mut engine = game(261_027_811 + u64::from(laezel_first));
        let ajani = cast_ajani(&mut engine);
        animate_planeswalkers_with_lattice_and_bello(&mut engine);
        let jace = cast_fixture_permanent(
            &mut engine,
            "jace,_unraveler_of_secrets",
            ManaGift {
                c: 3,
                u: 2,
                ..Default::default()
            },
        );
        assert_creature_planeswalker(&engine, ajani);
        assert_creature_planeswalker(&engine, jace);
        assert_eq!(
            engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
            5
        );

        // Enter these real replacement sources after the walkers, so their entry loyalty stays 4/5.
        let laezel = cast_fixture_permanent(
            &mut engine,
            "laezel,_vlaakiths_champion",
            ManaGift {
                c: 2,
                w: 1,
                ..Default::default()
            },
        );
        let season = cast_fixture_permanent(
            &mut engine,
            "doubling_season",
            ManaGift {
                c: 4,
                g: 1,
                ..Default::default()
            },
        );
        let desired_source = if laezel_first { laezel } else { season };
        let expected = if laezel_first { 4 } else { 3 };

        apply_ability(&mut engine, 0, ajani, 1, vec![]).expect("activate the actual -2 ability");
        assert_eq!(
            engine.state.objects[&ajani].counter_count(CounterKind::Loyalty),
            2
        );
        let stack_id = engine.state.stack.last().expect("-2 is on the stack").id;
        let mut batch = pass_both_players_with_batch(&mut engine);
        let mut completed = 0;
        let mut choices = 0;
        loop {
            completed += batch
                .events
                .iter()
                .filter(|event| {
                    matches!(&event.ev,
                Some(Ev::StackResolved(resolved)) if resolved.object_id == stack_id)
                })
                .count();
            if engine.state.pending_resolution.is_none() {
                break;
            }
            choices += 1;
            assert!(
                choices <= 6,
                "each affected permanent has one replacement-order choice"
            );
            assert_eq!(
                engine.state.objects[&ajani].counter_count(CounterKind::PlusOnePlusOne),
                0
            );
            assert_eq!(
                engine.state.objects[&jace].counter_count(CounterKind::PlusOnePlusOne),
                0
            );
            assert_eq!(
                engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
                5,
                "the entire simultaneous placement remains uncommitted while choices are pending"
            );
            let choice = find_resolution_choice(&batch).expect("published replacement choice");
            assert_eq!(choice.choice_kind(), ChoiceKind::ReplacementEffect);
            assert_eq!(choice.deciding_player_id, 0);
            assert_eq!(choice.replacement_options.len(), 2);
            let selected = choice
                .replacement_options
                .iter()
                .find(|option| option.source_object_id == desired_source)
                .expect("the selected real replacement source is offered")
                .application_id;
            batch = engine
                .apply_scenario_command(0, &submit_resolution_choice(vec![selected]))
                .expect("choose the next replacement effect");
        }
        assert_eq!(
            choices, 6,
            "Ajani, Jace, Bello, Lattice, Lae'zel and Season are creatures"
        );
        assert_eq!(completed, 1);
        assert_eq!(
            engine.state.objects[&ajani].counter_count(CounterKind::PlusOnePlusOne),
            expected
        );
        assert_eq!(
            engine.state.objects[&ajani].counter_count(CounterKind::Loyalty),
            2,
            "animated Ajani receives a creature counter, but no loyalty counter for itself"
        );
        assert_eq!(
            engine.state.objects[&jace].counter_count(CounterKind::PlusOnePlusOne),
            expected
        );
        assert_eq!(
            engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
            5 + expected,
            "both kinds on this dual permanent use the same replacement order"
        );
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn ajani_minus_two_excludes_only_its_exact_source_incarnation() {
    for return_ajani in [false, true] {
        let mut engine = game(261_027_813 + u64::from(return_ajani));
        let ajani = cast_ajani(&mut engine);
        animate_planeswalkers_with_lattice_and_bello(&mut engine);
        let jace = cast_fixture_permanent(
            &mut engine,
            "jace,_unraveler_of_secrets",
            ManaGift {
                c: 3,
                u: 2,
                ..Default::default()
            },
        );
        assert_creature_planeswalker(&engine, ajani);
        assert_creature_planeswalker(&engine, jace);
        let original_generation = engine
            .state
            .zone_change_generation
            .get(&ajani)
            .copied()
            .unwrap_or(0);
        apply_ability(&mut engine, 0, ajani, 1, vec![]).expect("activate the actual -2 ability");
        move_ajani_to_graveyard(&mut engine);
        if return_ajani {
            let returned = move_ready_to_battlefield(&mut engine, 0, AJANI);
            assert_eq!(returned, ajani, "the same physical card returned");
            assert!(engine.state.zone_change_generation[&ajani] > original_generation);
            assert_creature_planeswalker(&engine, ajani);
            assert_eq!(
                engine.state.objects[&ajani].counter_count(CounterKind::Loyalty),
                4
            );
        }
        pass_both_players(&mut engine);
        assert_eq!(
            engine.state.objects[&jace].counter_count(CounterKind::PlusOnePlusOne),
            1
        );
        assert_eq!(
            engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
            6
        );
        if return_ajani {
            assert_eq!(
                engine.state.objects[&ajani].counter_count(CounterKind::PlusOnePlusOne),
                1
            );
            assert_eq!(
                engine.state.objects[&ajani].counter_count(CounterKind::Loyalty),
                5,
                "the new incarnation is another planeswalker relative to the captured source"
            );
        } else {
            assert_eq!(engine.state.objects[&ajani].zone, Zone::Graveyard);
        }
        assert!(engine.state.stack.is_empty());
    }
}

fn parked_ajani_counter_batch(
    seed: u64,
    with_observer: bool,
) -> (GameEngine, u32, u32, u32, u32, u32, Option<u32>) {
    let registry = tricerules_cards::registry::global();
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[0, 1, 2],
        20,
        Some(vec![
            deck_with("plains", &[]),
            deck_with("forest", &[]),
            deck_with("island", &[]),
        ]),
        true,
    )
    .expect("create three-player game");
    advance_to_main1_from_game_start(&mut engine);
    // Only the bear's initial placement and mana are synthetic. Act of Treason establishes a real
    // temporary control effect, which ends if its controller leaves the multiplayer game.
    let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let cast = |engine: &mut GameEngine, id: &str, mana: ManaGift, targets: Vec<TargetRef>| {
        let object = inject_card_into_hand(engine, 0, id);
        give_mana(engine, 0, mana);
        let slot = hand_index_for_card(engine, 0, id);
        engine
            .apply_scenario_command(0, &cast_spell(slot, targets))
            .expect("cast fixture card");
        pass_priority_round(engine);
        object
    };
    let aura = if with_observer {
        use tricerules_core::AttachmentRecipient;
        // Initial Aura placement/attachment is synthetic. The actual Light cast, target and ETB
        // establish the independent return observer that will interrupt the counter batch.
        let aura = inject_permanent_on_battlefield(&mut engine, 1, "pacifism");
        engine.state.objects.get_mut(&aura).unwrap().attached_to =
            Some(AttachmentRecipient::Object(bear));
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
            .expect("choose the foreign Aura for the real Banishing Light trigger");
        pass_priority_round(&mut engine);
        assert_eq!(engine.state.objects[&aura].zone, Zone::Exile);
        assert_eq!(engine.state.active_event_observers.len(), 1);
        Some(aura)
    } else {
        None
    };
    let ajani = cast(
        &mut engine,
        AJANI,
        ManaGift {
            c: 3,
            w: 1,
            ..Default::default()
        },
        vec![],
    );
    assert_eq!(
        engine.state.objects[&ajani].counter_count(CounterKind::Loyalty),
        4
    );
    cast(
        &mut engine,
        "act_of_treason",
        ManaGift {
            c: 2,
            r: 1,
            ..Default::default()
        },
        target_object(bear),
    );
    assert_eq!(engine.characteristics(bear).unwrap().controller, 0);
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
    for id in [ajani, bear, laezel, first_season, second_season] {
        assert_eq!(engine.state.objects[&id].zone, Zone::Battlefield);
    }

    apply_ability(&mut engine, 0, ajani, 1, vec![]).expect("activate Ajani's actual -2");
    let stack_id = engine.state.stack.last().expect("ability on the stack").id;
    let mut batch = None;
    for _ in 0..3 {
        let actor = engine.state.priority_player_id();
        batch = Some(
            engine
                .apply_scenario_command(actor, &pass())
                .expect("pass priority"),
        );
    }
    let choice = find_resolution_choice(&batch.unwrap()).expect("first replacement-order choice");
    assert!(
        choice.prompt_text.contains("Grizzly Bears"),
        "the borrowed creature is first"
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
        .expect("apply Lae'zel before the two Seasons");
    let remaining = find_resolution_choice(&next).expect("the two Seasons still need ordering");
    assert_eq!(remaining.replacement_options.len(), 2);
    assert!(remaining
        .replacement_options
        .iter()
        .all(|option| option.source_object_id == first_season
            || option.source_object_id == second_season));
    assert_eq!(
        engine.state.objects[&bear].counter_count(CounterKind::PlusOnePlusOne),
        0,
        "no counter placement has committed yet"
    );
    (
        engine,
        bear,
        laezel,
        first_season,
        second_season,
        stack_id,
        aura,
    )
}

#[test]
fn ajani_minus_two_begun_counter_batch_survives_controller_departure() {
    let (mut engine, bear, laezel, first_season, second_season, stack_id, _) =
        parked_ajani_counter_batch(261_027_824, false);
    let generation = engine
        .state
        .zone_change_generation
        .get(&bear)
        .copied()
        .unwrap_or(0);
    let batch = engine
        .apply_scenario_command(0, &concede())
        .expect("ability controller leaves");
    assert!(engine.state.players[0].has_lost);
    assert!([laezel, first_season, second_season]
        .into_iter()
        .all(|id| !engine.state.objects.contains_key(&id)));
    assert_eq!(engine.state.objects[&bear].zone, Zone::Battlefield);
    assert_eq!(engine.characteristics(bear).unwrap().controller, 1);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&bear)
            .copied()
            .unwrap_or(0),
        generation
    );
    assert_eq!(
        engine.state.objects[&bear].counter_count(CounterKind::PlusOnePlusOne),
        2,
        "Lae'zel's accepted replacement remains applied after its source leaves"
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
    let next_player = engine.state.priority_player_id();
    assert_ne!(next_player, 0, "a departed player cannot retain priority");
    engine
        .apply_scenario_command(next_player, &pass())
        .expect("a surviving player can continue");
}

#[test]
fn ajani_minus_two_prunes_departed_recipient_and_refreshes_remaining_batch() {
    let (mut engine, bear, laezel, first_season, _, stack_id, _) =
        parked_ajani_counter_batch(261_027_825, false);
    let mut batch = engine
        .apply_scenario_command(1, &concede())
        .expect("borrowed creature's owner leaves");
    assert!(!engine.state.objects.contains_key(&bear));
    let mut completed = 0;
    let mut choices = 0;
    loop {
        completed += batch
            .events
            .iter()
            .filter(|event| {
                matches!(&event.ev,
            Some(Ev::StackResolved(resolved)) if resolved.object_id == stack_id)
            })
            .count();
        if engine.state.pending_resolution.is_none() {
            break;
        }
        choices += 1;
        assert!(
            choices <= 2,
            "the remaining Lae'zel needs at most two ordering choices"
        );
        let choice = find_resolution_choice(&batch).expect("refreshed live-recipient choice");
        assert!(choice.prompt_text.contains("Lae'zel"));
        let source = if choices == 1 { laezel } else { first_season };
        let selected = choice
            .replacement_options
            .iter()
            .find(|option| option.source_object_id == source)
            .expect("remaining replacement source is offered")
            .application_id;
        batch = engine
            .apply_scenario_command(0, &submit_resolution_choice(vec![selected]))
            .expect("order the surviving recipient's replacements");
    }
    assert_eq!(choices, 2);
    assert_eq!(
        engine.state.objects[&laezel].counter_count(CounterKind::PlusOnePlusOne),
        8
    );
    assert_eq!(completed, 1);
    assert!(engine.diagnostic_snapshot().unwrap()["state"]["pending_replacement_event"].is_null());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn ajani_minus_two_restores_begun_counters_after_independent_aura_return() {
    use tricerules_core::AttachmentRecipient;
    let (mut engine, bear, _, _, _, stack_id, aura) = parked_ajani_counter_batch(261_027_826, true);
    let aura = aura.expect("real Banishing Light established the Aura observer");
    let departure = engine
        .apply_scenario_command(0, &concede())
        .expect("ability controller leaves");
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Aura return is independently parked");
    assert_eq!(pending.deciding_player, 1);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::AuraPermanent);
    assert!(pending.presentation.candidates.contains(&bear));
    assert_eq!(engine.state.objects[&aura].zone, Zone::Exile);
    assert_eq!(
        engine.state.objects[&bear].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    assert!(
        engine.diagnostic_snapshot().unwrap()["state"]["pending_observer_return_batch"].is_object(),
        "the already-begun counter resolution remains suspended beneath the Aura choice"
    );
    assert!(!departure.events.iter().any(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == stack_id)));

    let returned = engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![bear]))
        .expect("the Aura owner attaches it and restores the suspended counter batch");
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(bear))
    );
    assert_eq!(
        engine.state.objects[&bear].counter_count(CounterKind::PlusOnePlusOne),
        2,
        "the restored batch retains Lae'zel's accepted replacement and finishes on the same bear"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.diagnostic_snapshot().unwrap()["state"]["pending_replacement_event"].is_null());
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

fn create_ajani_emblem(engine: &mut GameEngine, ajani: u32) -> u32 {
    // Synthetic loyalty setup avoids advancing several turns; the -7 cost and ability are real.
    engine
        .state
        .objects
        .get_mut(&ajani)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 7);
    apply_ability(engine, 0, ajani, 2, vec![]).expect("activate the actual -7 ability");
    assert_eq!(
        engine.state.objects[&ajani].zone,
        Zone::Graveyard,
        "state-based actions move Ajani after the last seven loyalty are paid"
    );
    assert_eq!(engine.state.stack.len(), 1);
    pass_both_players(engine);
    assert!(engine.state.stack.is_empty());
    engine
        .state
        .static_emblems
        .iter()
        .find(|emblem| emblem.controller == 0)
        .expect("the ability creates an emblem after Ajani has left")
        .object_id
}

fn bolt(engine: &mut GameEngine, targets: Vec<TargetRef>) -> u32 {
    // Synthetic hand/mana setup; the actual instant, targeting, payment and damage are accepted.
    let spell = inject_card_into_hand(engine, 0, "lightning_bolt");
    give_mana(
        engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "lightning_bolt");
    engine
        .apply_scenario_command(0, &cast_spell(slot, targets))
        .expect("cast Lightning Bolt");
    pass_both_players(engine);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    spell
}

#[test]
fn ajani_emblem_is_created_after_its_source_dies_to_the_loyalty_cost() {
    let mut engine = game(261_027_820);
    let ajani = cast_ajani(&mut engine);
    let marker = create_ajani_emblem(&mut engine, ajani);
    assert_eq!(engine.state.objects[&ajani].zone, Zone::Graveyard);
    assert!(engine.initial_response_batch().events.iter().any(|event| {
        matches!(&event.ev, Some(Ev::ZoneView(view)) if view.per_player.iter().any(|player| {
            player.player_id == 0 && player.static_emblems.iter().any(|emblem| emblem.object_id == marker)
        }))
    }), "the persistent emblem has a public command-zone marker");
}

#[test]
fn ajani_emblem_protects_its_controller_and_controlled_planeswalkers_only() {
    let mut engine = game(261_027_821);
    let ajani = cast_ajani(&mut engine);
    let own_walker = cast_fixture_permanent(
        &mut engine,
        "jace,_unraveler_of_secrets",
        ManaGift {
            c: 3,
            u: 2,
            ..Default::default()
        },
    );
    // Synthetic opposing walker and creature placement supplies unrelated damage recipients.
    let opposing_walker =
        inject_permanent_on_battlefield(&mut engine, 1, "jace,_unraveler_of_secrets");
    engine
        .state
        .objects
        .get_mut(&opposing_walker)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 5);
    let own_creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    create_ajani_emblem(&mut engine, ajani);

    bolt(&mut engine, target_player(0));
    assert_eq!(engine.state.players[0].life, 19);
    bolt(&mut engine, target_object(own_walker));
    assert_eq!(
        engine.state.objects[&own_walker].counter_count(CounterKind::Loyalty),
        4
    );
    bolt(&mut engine, target_player(1));
    assert_eq!(engine.state.players[1].life, 17);
    bolt(&mut engine, target_object(opposing_walker));
    assert_eq!(
        engine.state.objects[&opposing_walker].counter_count(CounterKind::Loyalty),
        2
    );
    bolt(&mut engine, target_object(own_creature));
    assert_eq!(engine.state.objects[&own_creature].zone, Zone::Graveyard);
}

#[test]
fn ajani_emblem_leaves_one_damage_for_each_separate_source() {
    let mut engine = game(261_027_822);
    let ajani = cast_ajani(&mut engine);
    let marker = create_ajani_emblem(&mut engine, ajani);
    let first = bolt(&mut engine, target_player(0));
    assert_eq!(engine.state.players[0].life, 19);
    let second = bolt(&mut engine, target_player(0));
    assert_ne!(first, second, "the two spells are separate damage sources");
    assert_eq!(engine.state.players[0].life, 18);
    assert!(engine
        .state
        .static_emblems
        .iter()
        .any(|emblem| emblem.object_id == marker));
}

#[test]
fn ajani_emblem_does_not_prevent_damage_when_stomp_prohibits_prevention() {
    let mut engine = game(261_027_823);
    let ajani = cast_ajani(&mut engine);
    create_ajani_emblem(&mut engine, ajani);
    // Synthetic hand/mana setup; Stomp's prohibition and damage use its actual Adventure face.
    inject_card_into_hand(&mut engine, 0, "bonecrusher_giant_stomp");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "bonecrusher_giant_stomp");
    engine
        .apply_scenario_command(0, &cast_spell_face(slot, target_player(0), 1))
        .expect("cast Stomp");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.players[0].life, 18,
        "Stomp's damage is unpreventable"
    );
    bolt(&mut engine, target_player(0));
    assert_eq!(
        engine.state.players[0].life, 15,
        "the prohibition lasts for the turn"
    );
}

#[test]
fn ajani_emblem_and_gratuitous_violence_obey_both_replacement_orders() {
    for emblem_first in [true, false] {
        let mut engine = game(261_027_830 + u64::from(emblem_first));
        let ajani = cast_ajani(&mut engine);
        create_ajani_emblem(&mut engine, ajani);
        let violence = cast_fixture_permanent(
            &mut engine,
            "gratuitous_violence",
            ManaGift {
                c: 2,
                r: 3,
                ..Default::default()
            },
        );
        // Card and mana are synthetic setup. The Dragon is really cast and its actual ETB
        // ability is targeted; the creature itself is the three-damage source.
        let dragon = cast_fixture_permanent(
            &mut engine,
            "nebula_dragon",
            ManaGift {
                c: 6,
                r: 1,
                ..Default::default()
            },
        );
        assert_eq!(engine.state.pending_triggers.len(), 1);
        engine
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                        decline: false,
                        selected_modes: Vec::new(),
                        targets: target_player(0),
                    })),
                },
            )
            .expect("target the emblem controller with the actual Dragon trigger");
        let stack_id = engine
            .state
            .stack
            .last()
            .expect("Dragon trigger on stack")
            .id;
        let batch = pass_both_players_with_batch(&mut engine);
        let choice = find_resolution_choice(&batch).expect("damage replacements need ordering");
        assert_eq!(choice.deciding_player_id, 0);
        assert_eq!(choice.replacement_options.len(), 2);
        let emblem = choice
            .replacement_options
            .iter()
            .find(|option| {
                option
                    .effect_summary
                    .contains("Prevent all but 1 of this damage")
            })
            .expect("the emblem prevention is offered");
        let doubling = choice
            .replacement_options
            .iter()
            .find(|option| option.effect_summary.contains("Double this damage"))
            .expect("the actual enchantment's replacement is offered");
        assert!(emblem.source_card_name.is_empty());
        assert_eq!(
            emblem.source_object_id, 0,
            "an emblem is not a battlefield card"
        );
        assert_eq!(doubling.source_card_name, "Gratuitous Violence");
        assert_eq!(doubling.source_object_id, violence);
        assert_ne!(emblem.application_id, doubling.application_id);
        assert!(choice.replacement_options.iter().all(|option| option
            .effect_summary
            .contains("3 damage from Nebula Dragon")));
        assert_eq!(engine.state.objects[&dragon].zone, Zone::Battlefield);
        let selected = if emblem_first {
            emblem.application_id
        } else {
            doubling.application_id
        };
        let settled = engine
            .apply_scenario_command(0, &submit_resolution_choice(vec![selected]))
            .expect("apply the selected replacement and settle the other");
        assert_eq!(
            engine.state.players[0].life,
            if emblem_first { 18 } else { 19 },
            "3→1→2 differs from 3→6→1"
        );
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
        assert_eq!(
            settled
                .events
                .iter()
                .filter(|event| matches!(&event.ev,
            Some(Ev::StackResolved(resolved)) if resolved.object_id == stack_id))
                .count(),
            1
        );
    }
}

#[test]
fn ajani_emblems_are_independent_applications_for_each_damage_event() {
    let mut engine = game(261_027_832);
    let first_ajani = cast_ajani(&mut engine);
    let first_marker = create_ajani_emblem(&mut engine, first_ajani);
    let second_ajani = cast_ajani(&mut engine);
    create_ajani_emblem(&mut engine, second_ajani);
    let markers: Vec<_> = engine
        .state
        .static_emblems
        .iter()
        .filter(|emblem| emblem.controller == 0)
        .map(|emblem| emblem.object_id)
        .collect();
    assert_eq!(markers.len(), 2);
    assert!(markers.contains(&first_marker));
    assert_ne!(markers[0], markers[1]);
    assert!(markers.iter().all(|marker| engine
        .state
        .damage_prevention_effects
        .iter()
        .any(|effect| effect.source_id == Some(*marker))));

    let mut prior_applications = Vec::new();
    for expected_life in [19, 18] {
        let spell = inject_card_into_hand(&mut engine, 0, "lightning_bolt");
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
            .apply_scenario_command(0, &cast_spell(slot, target_player(0)))
            .expect("cast a separate actual Lightning Bolt");
        let stack_id = engine.state.stack.last().unwrap().id;
        let batch = pass_both_players_with_batch(&mut engine);
        let choice = find_resolution_choice(&batch).expect("two emblems need an order");
        assert_eq!(choice.replacement_options.len(), 2);
        let ids: Vec<_> = choice
            .replacement_options
            .iter()
            .map(|option| {
                assert!(option.source_card_name.is_empty());
                assert_eq!(option.source_object_id, 0);
                assert!(option
                    .effect_summary
                    .contains("Prevent all but 1 of this damage"));
                option.application_id
            })
            .collect();
        assert_ne!(ids[0], ids[1]);
        assert!(
            ids.iter().all(|id| !prior_applications.contains(id)),
            "each event gets distinct opaque application IDs"
        );
        assert!(engine
            .apply_scenario_command(1, &submit_resolution_choice(vec![ids[0]]))
            .is_err());
        assert!(engine
            .apply_scenario_command(0, &submit_resolution_choice(vec![u32::MAX]))
            .is_err());
        assert!(engine.state.pending_resolution.is_some());
        let settled = engine
            .apply_scenario_command(0, &submit_resolution_choice(vec![ids[0]]))
            .expect("choose one emblem; the other applies once to the remaining one damage");
        assert_eq!(engine.state.players[0].life, expected_life);
        assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(
            settled
                .events
                .iter()
                .filter(|event| matches!(&event.ev,
            Some(Ev::StackResolved(resolved)) if resolved.object_id == stack_id))
                .count(),
            1
        );
        prior_applications = ids;
    }
}

#[test]
fn ajani_emblem_uses_current_planeswalker_type_and_control() {
    use tricerules_cards::{
        ContinuousEffectKind, ControllerReference, EffectDuration, PermanentTypeFilter,
        TypeLineReplacement,
    };
    use tricerules_core::{AffectedScope, ContinuousEffect};
    fn add_effect(engine: &mut GameEngine, object: u32, kind: ContinuousEffectKind) {
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(object),
            kind,
            condition: None,
            duration: EffectDuration::Indefinite,
            timestamp: engine.state.command_index + 1,
        });
    }
    let mut engine = game(261_027_833);
    let ajani = cast_ajani(&mut engine);
    create_ajani_emblem(&mut engine, ajani);
    // The initial foreign walker, its loyalty, and the two layer effects are synthetic setup.
    let walker = inject_permanent_on_battlefield(&mut engine, 1, "jace,_unraveler_of_secrets");
    engine
        .state
        .objects
        .get_mut(&walker)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 8);
    bolt(&mut engine, target_object(walker));
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        5
    );
    add_effect(
        &mut engine,
        walker,
        ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(0),
        },
    );
    assert_eq!(engine.characteristics(walker).unwrap().controller, 0);
    bolt(&mut engine, target_object(walker));
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        4
    );
    engine.state.continuous_effects.retain(|effect| {
        !matches!(
            effect.kind,
            ContinuousEffectKind::Layer2Control {
                controller: ControllerReference::Fixed(0)
            }
        )
    });
    assert_eq!(engine.characteristics(walker).unwrap().controller, 1);
    bolt(&mut engine, target_object(walker));
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        1
    );

    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&bear)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 4);
    add_effect(
        &mut engine,
        bear,
        ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
            card_types: vec![
                PermanentTypeFilter::Creature,
                PermanentTypeFilter::Planeswalker,
            ],
            creature_types: vec![],
            land_types: vec![],
        }),
    );
    assert!(engine
        .characteristics(bear)
        .unwrap()
        .has_type("Planeswalker"));
    bolt(&mut engine, target_object(bear));
    assert_eq!(engine.state.objects[&bear].damage, 1);
    assert_eq!(
        engine.state.objects[&bear].counter_count(CounterKind::Loyalty),
        3
    );
    engine
        .state
        .continuous_effects
        .retain(|effect| !matches!(effect.kind, ContinuousEffectKind::Layer4SetTypeLine(_)));
    assert!(!engine
        .characteristics(bear)
        .unwrap()
        .has_type("Planeswalker"));
    bolt(&mut engine, target_object(bear));
    assert_eq!(
        engine.state.objects[&bear].zone,
        Zone::Graveyard,
        "a creature that stopped being a planeswalker is no longer protected"
    );
}

#[test]
fn ajani_emblem_prevents_each_combat_source_and_lifelink_uses_damage_dealt() {
    let mut engine = game(261_027_834);
    let ajani = cast_ajani(&mut engine);
    create_ajani_emblem(&mut engine, ajani);
    let starting_turn = engine.state.turn_instance;
    for _ in 0..100 {
        if engine.state.turn_instance > starting_turn
            && engine.state.active_player_id() == 1
            && engine.state.turn_step == TurnStep::Main1
        {
            break;
        }
        let (actor, command) = match engine.state.cleanup_discard_player {
            Some(player) => {
                let index = engine.state.player_idx(player).unwrap();
                let excess = engine.state.players[index].hand.len() - 7;
                (player, discard_cleanup_batch((0..excess as u32).collect()))
            }
            None => (engine.state.priority_player_id(), pass()),
        };
        engine
            .apply_scenario_command(actor, &command)
            .expect("pass through the real turn and cleanup");
    }
    assert_eq!(engine.state.active_player_id(), 1);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    // Synthetic placement supplies two real printed 2-power creatures, one with lifelink.
    let lifelinker = inject_creature_on_battlefield(&mut engine, 1, "child_of_night");
    let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine
        .apply_scenario_command(1, &primitive_yield())
        .expect("begin combat");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
    engine
        .apply_scenario_command(1, &declare_attackers(vec![lifelinker, bear]))
        .expect("declare the two separate attacking sources");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareBlockers);
    engine
        .apply_scenario_command(0, &declare_blockers(vec![]))
        .expect("do not block");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.players[0].life, 18,
        "each attacking source deals one, rather than treating their damage as one event"
    );
    assert_eq!(
        engine.state.players[1].life, 21,
        "Child of Night gains life for the one damage actually dealt"
    );
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn ajani_emblem_departure_refreshes_a_borrowed_walkers_parked_damage_choice() {
    use tricerules_cards::{ContinuousEffectKind, ControllerReference, EffectDuration};
    use tricerules_core::{AffectedScope, ContinuousEffect};

    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        261_027_835,
        &[0, 1, 2],
        20,
        Some(vec![
            deck_with("plains", &[]),
            deck_with("island", &[]),
            deck_with("mountain", &[]),
        ]),
        true,
    )
    .expect("create three-player game");
    advance_to_main1_from_game_start(&mut engine);
    let ajani = inject_card_into_hand(&mut engine, 0, AJANI);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            w: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, AJANI);
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast actual Ajani");
    pass_priority_round(&mut engine);
    // The loyalty, foreign walker, control effect and two one-point shields are synthetic setup.
    // The ultimate, the Bolt, the departure and both ordering decisions are real commands.
    engine
        .state
        .objects
        .get_mut(&ajani)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 7);
    apply_ability(&mut engine, 0, ajani, 2, vec![]).expect("activate actual -7");
    pass_priority_round(&mut engine);
    let marker = engine
        .state
        .static_emblems
        .iter()
        .find(|emblem| emblem.controller == 0)
        .expect("emblem exists")
        .object_id;
    let walker = inject_permanent_on_battlefield(&mut engine, 1, "jace,_unraveler_of_secrets");
    engine
        .state
        .objects
        .get_mut(&walker)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 5);
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(walker),
        kind: ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(0),
        },
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index + 1,
    });
    engine.state.add_damage_prevention_shield(walker, 1);
    engine.state.add_damage_prevention_shield(walker, 1);
    assert_eq!(engine.characteristics(walker).unwrap().controller, 0);
    assert!(engine
        .state
        .damage_prevention_effects
        .iter()
        .any(|effect| effect.source_id == Some(marker)));
    let spell = inject_card_into_hand(&mut engine, 2, "lightning_bolt");
    give_mana(
        &mut engine,
        2,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(0, &pass())
        .expect("P0 passes to P1");
    engine
        .apply_scenario_command(1, &pass())
        .expect("P1 passes to P2");
    let slot = hand_index_for_card(&engine, 2, "lightning_bolt");
    engine
        .apply_scenario_command(2, &cast_spell(slot, target_object(walker)))
        .expect("P2 casts the actual Bolt on the borrowed walker");
    let stack_id = engine.state.stack.last().unwrap().id;
    let mut batch = None;
    for _ in 0..3 {
        let actor = engine.state.priority_player_id();
        batch = Some(
            engine
                .apply_scenario_command(actor, &pass())
                .expect("pass priority"),
        );
    }
    let old =
        find_resolution_choice(&batch.unwrap()).expect("the three replacements need an order");
    assert_eq!(old.deciding_player_id, 0);
    assert_eq!(old.replacement_options.len(), 3);
    let old_application = old
        .replacement_options
        .iter()
        .find(|option| {
            option
                .effect_summary
                .contains("Prevent all but 1 of this damage")
        })
        .expect("the original offer includes the emblem")
        .application_id;

    let departed = engine
        .apply_scenario_command(0, &concede())
        .expect("the emblem controller leaves during the parked damage event");
    assert!(engine.state.players[0].has_lost);
    assert_eq!(engine.characteristics(walker).unwrap().controller, 1);
    let diagnostic = engine.diagnostic_snapshot().unwrap();
    assert!(!diagnostic["state"]["static_emblems"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["object_id"].as_u64() == Some(u64::from(marker))));
    assert!(
        !diagnostic["state"]["damage_prevention_effects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["source_id"].as_u64() == Some(u64::from(marker))),
        "the departed marker's linked prevention was removed before refreshing the choice"
    );
    let refreshed =
        find_resolution_choice(&departed).expect("the surviving owner chooses the shields");
    assert_eq!(refreshed.deciding_player_id, 1);
    assert_eq!(refreshed.replacement_options.len(), 2);
    assert!(refreshed.replacement_options.iter().all(|option| !option
        .effect_summary
        .contains("Prevent all but 1 of this damage")));
    let mut stale = submit_resolution_choice(vec![old_application]);
    if let Some(Cmd::SubmitResolutionChoice(answer)) = stale.cmd.as_mut() {
        answer.resolution_prompt_id = old.resolution_prompt_id;
    }
    assert!(
        engine.apply_scenario_command(1, &stale).is_err(),
        "the old published answer is stale"
    );
    assert!(engine.state.pending_resolution.is_some());
    let selected = refreshed.replacement_options[0].application_id;
    let settled = engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![selected]))
        .expect("the surviving owner orders the two remaining shields");
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        4
    );
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.diagnostic_snapshot().unwrap()["state"]["pending_replacement_event"].is_null());
    assert_eq!(
        settled
            .events
            .iter()
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == stack_id))
            .count(),
        1
    );
    let actor = engine.state.priority_player_id();
    assert_ne!(actor, 0);
    engine
        .apply_scenario_command(actor, &pass())
        .expect("a surviving player can continue");
}

#[test]
fn ajani_emblem_creation_and_replacement_choice_replay_as_accepted_wire_commands() {
    use prost::Message;
    fn fixture() -> (GameEngine, u32, u32, usize) {
        let mut engine = game(261_027_836);
        let first = cast_ajani(&mut engine);
        create_ajani_emblem(&mut engine, first);
        let second = cast_ajani(&mut engine);
        // Synthetic loyalty, hand card and mana; all subsequent commands are recorded below.
        engine
            .state
            .objects
            .get_mut(&second)
            .unwrap()
            .set_counter(CounterKind::Loyalty, 7);
        let bolt = inject_card_into_hand(&mut engine, 0, "lightning_bolt");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                r: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&engine, 0, "lightning_bolt");
        (engine, second, bolt, slot)
    }
    fn apply_wire(
        original: &mut GameEngine,
        replay: &mut GameEngine,
        actor: i32,
        command: RuledCommand,
    ) -> RuledEventBatch {
        let decoded = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
        let actual = original
            .apply_scenario_command(actor, &command)
            .expect("accepted original");
        assert_eq!(
            actual,
            replay
                .apply_scenario_command(actor, &decoded)
                .expect("accepted replay")
        );
        actual
    }
    let (mut engine, ajani, bolt, slot) = fixture();
    let (mut replay, replay_ajani, replay_bolt, replay_slot) = fixture();
    assert_eq!(
        (ajani, bolt, slot),
        (replay_ajani, replay_bolt, replay_slot)
    );
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
    let ultimate = activate_ability_for(&engine, ajani, 2, vec![]);
    apply_wire(&mut engine, &mut replay, 0, ultimate);
    for _ in 0..2 {
        let actor = engine.state.priority_player_id();
        apply_wire(&mut engine, &mut replay, actor, pass());
    }
    assert_eq!(engine.state.static_emblems.len(), 2);
    apply_wire(
        &mut engine,
        &mut replay,
        0,
        cast_spell(slot, target_player(0)),
    );
    let stack_id = engine.state.stack.last().unwrap().id;
    let mut batch = None;
    for _ in 0..2 {
        let actor = engine.state.priority_player_id();
        batch = Some(apply_wire(&mut engine, &mut replay, actor, pass()));
    }
    let choice = find_resolution_choice(&batch.unwrap()).expect("two emblems offer an order");
    let mut answer = submit_resolution_choice(vec![choice.replacement_options[0].application_id]);
    if let Some(Cmd::SubmitResolutionChoice(submission)) = answer.cmd.as_mut() {
        submission.resolution_prompt_id = choice.resolution_prompt_id;
    }
    let settled = apply_wire(&mut engine, &mut replay, 0, answer);
    assert_eq!(engine.state.players[0].life, 19);
    assert_eq!(engine.state.objects[&bolt].zone, Zone::Graveyard);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        settled
            .events
            .iter()
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == stack_id))
            .count(),
        1
    );
    let actor = engine.state.priority_player_id();
    apply_wire(&mut engine, &mut replay, actor, pass());
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}
