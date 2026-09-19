//! Issue #414 focused scenarios for the five retained enters-with-minus-counter identities.
//!
//! CR 614.1c / 122.6 entry replacements start the creature with exactly its printed -1/-1
//! counters; CR 602.2b / 601.2h / 122.1 removal is an atomic activation cost paid from the exact
//! source generation before the effect resolves; an activation with too few counters is rejected
//! without partial payment; and CR 307.5 / 602.5d sorcery-speed timing rejects activation while
//! the stack is not empty. Hovel Hurler's printed "another" excludes the source from its own
//! target group. "Remove two counters from this creature" is paid as two any-one-counter cost
//! components, so Gnarlbark Elm is exercised with a mixed-kind pair (one -1/-1 plus one stun) and
//! Reaping Willow returns only a creature card with mana value 3 or less from its controller's
//! graveyard.

use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, CostChoiceKind, CostSelection, CounterRemovalSelection,
    LegalCostChoice, TargetRef, TargetRefKind,
};

fn engine_with(seed: u64, own: &[&str]) -> GameEngine {
    let decks = Some(vec![deck_with("forest", own), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn counters(engine: &GameEngine, oid: u32, kind: CounterKind) -> u32 {
    engine.state.objects[&oid].counter_count(kind)
}

fn stats(engine: &GameEngine, oid: u32) -> (u32, u32) {
    let characteristics = engine.characteristics(oid).expect("characteristics");
    (
        characteristics.power.expect("power"),
        characteristics.toughness.expect("toughness"),
    )
}

/// The first published counter-removal choice for `oid`'s ability 0, asserted to be the
/// source-paid any-one-counter form.
fn published_counter_selection(engine: &mut GameEngine, oid: u32) -> CostSelection {
    let legal = engine.initial_response_batch();
    let key = u64::from(oid) << 32;
    let choices = &legal.legal_by_player[&0].cost_choices_by_ability[&key];
    assert!(
        choices.non_mana_costs_payable,
        "counter cost must be payable"
    );
    let choice = choices
        .choices
        .iter()
        .find(|choice| choice.kind() == CostChoiceKind::RemoveCounters)
        .expect("a counter-removal cost choice is published");
    let removal = choice
        .counter_removal
        .as_ref()
        .expect("counter removal choices");
    assert_eq!(removal.count, 1);
    assert!(
        removal
            .options
            .iter()
            .any(|option| option.available_count >= 1),
        "at least one counter kind must be available"
    );
    let source = removal.source.expect("source payment carries its object");
    assert_eq!(source.object_id, oid);
    CostSelection {
        cost_index: choice.cost_index,
        selection: Some(Selection::CounterRemoval(CounterRemovalSelection {
            source: Some(source),
            option_id: removal.options[0].option_id,
        })),
    }
}

/// Activate ability 0 with the generation-bound source identity and the paid counter selection.
fn activate_with_counter_cost(
    engine: &GameEngine,
    oid: u32,
    targets: Vec<TargetRef>,
    selection: CostSelection,
) -> RuledCommand {
    let mut command = activate_ability_for(engine, oid, 0, targets);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!("activate_ability_for always builds an ActivateAbility command");
    };
    activation.cost_selections.push(selection);
    command
}

/// The two published counter-removal components of `oid`'s ability 0, plus the engine's joint
/// payability flag. "Remove two counters" is two any-one-counter components so mixed kinds work.
fn published_counter_components(engine: &mut GameEngine, oid: u32) -> (bool, Vec<LegalCostChoice>) {
    let legal = engine.initial_response_batch();
    let key = u64::from(oid) << 32;
    let choices = &legal.legal_by_player[&0].cost_choices_by_ability[&key];
    let removals = choices
        .choices
        .iter()
        .filter(|choice| choice.kind() == CostChoiceKind::RemoveCounters)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        removals.len(),
        2,
        "a remove-two-counters ability publishes two one-counter components"
    );
    (choices.non_mana_costs_payable, removals)
}

/// Build one generation-bound selection for `choice`, paying exactly `kind`.
fn counter_selection_for(choice: &LegalCostChoice, kind: CounterKind) -> CostSelection {
    let removal = choice
        .counter_removal
        .as_ref()
        .expect("counter removal choices");
    assert_eq!(removal.count, 1, "each component removes one counter");
    let option = removal
        .options
        .iter()
        .find(|option| option.label == kind.label())
        .unwrap_or_else(|| panic!("{} is not an available counter kind", kind.label()));
    CostSelection {
        cost_index: choice.cost_index,
        selection: Some(Selection::CounterRemoval(CounterRemovalSelection {
            source: removal.source,
            option_id: option.option_id,
        })),
    }
}

/// Activate ability 0 paying a vector of counter-component selections.
fn activate_with_counter_costs(
    engine: &GameEngine,
    oid: u32,
    targets: Vec<TargetRef>,
    selections: Vec<CostSelection>,
) -> RuledCommand {
    let mut command = activate_ability_for(engine, oid, 0, targets);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!("activate_ability_for always builds an ActivateAbility command");
    };
    activation.cost_selections.extend(selections);
    command
}

fn graveyard_object(oid: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id: oid,
        kind: TargetRefKind::Graveyard as i32,
        ..Default::default()
    }]
}

#[test]
fn issue_414_entry_replacements_place_the_printed_minus_counters() {
    let mut engine = engine_with(
        414_001,
        &["burdened_stoneback", "moonlit_lamenter", "hovel_hurler"],
    );
    let burdened = move_ready_to_battlefield(&mut engine, 0, "burdened_stoneback");
    assert_eq!(
        counters(&engine, burdened, CounterKind::MinusOneMinusOne),
        2,
        "Burdened Stoneback enters with exactly two -1/-1 counters"
    );
    assert_eq!(stats(&engine, burdened), (2, 2));

    let moonlit = move_ready_to_battlefield(&mut engine, 0, "moonlit_lamenter");
    assert_eq!(
        counters(&engine, moonlit, CounterKind::MinusOneMinusOne),
        1,
        "Moonlit Lamenter enters with exactly one -1/-1 counter"
    );
    assert_eq!(stats(&engine, moonlit), (1, 4));

    let hovel = move_ready_to_battlefield(&mut engine, 0, "hovel_hurler");
    assert_eq!(
        counters(&engine, hovel, CounterKind::MinusOneMinusOne),
        2,
        "Hovel Hurler enters with exactly two -1/-1 counters"
    );
    assert_eq!(stats(&engine, hovel), (4, 5));
}

#[test]
fn issue_414_counter_removal_is_paid_before_the_effect_resolves() {
    let mut engine = engine_with(414_010, &["burdened_stoneback"]);
    let burdened = move_ready_to_battlefield(&mut engine, 0, "burdened_stoneback");
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );

    let selection = published_counter_selection(&mut engine, burdened);
    engine
        .apply_command(
            0,
            &activate_with_counter_cost(&engine, burdened, target_object(target), selection),
        )
        .expect("pay mana and one counter as the activation cost");
    assert_eq!(
        counters(&engine, burdened, CounterKind::MinusOneMinusOne),
        1,
        "the counter is removed as part of the cost"
    );
    assert!(
        !engine.effective_has_keyword(target, Keyword::Indestructible),
        "the effect has not resolved yet"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert!(
        engine.effective_has_keyword(target, Keyword::Indestructible),
        "the target gains indestructible until end of turn"
    );
    assert!(
        !engine.effective_has_keyword(burdened, Keyword::Indestructible),
        "only the chosen target gains indestructible"
    );
}

#[test]
fn issue_414_activation_is_illegal_with_too_few_counters_and_pays_nothing() {
    let mut engine = engine_with(414_020, &["moonlit_lamenter"]);
    let moonlit = move_ready_to_battlefield(&mut engine, 0, "moonlit_lamenter");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let removal = published_counter_selection(&mut engine, moonlit);
    engine
        .apply_command(
            0,
            &activate_with_counter_cost(&engine, moonlit, vec![], removal.clone()),
        )
        .expect("the first activation removes the only counter");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(counters(&engine, moonlit, CounterKind::MinusOneMinusOne), 0);

    // With no counters left there is no legal payment; the stale selection is rejected and the
    // mana pool is untouched.
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let mana_before = engine.state.players[0].mana_pool.white;
    engine
        .apply_command(
            0,
            &activate_with_counter_cost(&engine, moonlit, vec![], removal),
        )
        .expect_err("a counter cost cannot be paid with no counters");
    assert_eq!(engine.state.players[0].mana_pool.white, mana_before);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_414_sorcery_speed_activation_is_rejected_while_the_stack_is_not_empty() {
    let mut engine = engine_with(414_030, &["burdened_stoneback", "lightning_bolt"]);
    let burdened = move_ready_to_battlefield(&mut engine, 0, "burdened_stoneback");
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            r: 1,
            ..Default::default()
        },
    );
    let selection = published_counter_selection(&mut engine, burdened);
    let slot = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Lightning Bolt");
    assert_eq!(engine.state.stack.len(), 1);

    engine
        .apply_command(
            0,
            &activate_with_counter_cost(&engine, burdened, target_object(target), selection),
        )
        .expect_err("an Activate only as a sorcery ability needs an empty stack");
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(
        counters(&engine, burdened, CounterKind::MinusOneMinusOne),
        2
    );
}

#[test]
fn issue_414_moonlit_lamenter_activation_draws_only_on_resolution() {
    let mut engine = engine_with(414_040, &["moonlit_lamenter"]);
    let moonlit = move_ready_to_battlefield(&mut engine, 0, "moonlit_lamenter");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let before = engine.state.players[0].hand.len();
    let selection = published_counter_selection(&mut engine, moonlit);
    engine
        .apply_command(
            0,
            &activate_with_counter_cost(&engine, moonlit, vec![], selection),
        )
        .expect("activate Moonlit Lamenter");
    assert_eq!(
        engine.state.players[0].hand.len(),
        before,
        "the card is not drawn during activation"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), before + 1);
    assert_eq!(counters(&engine, moonlit, CounterKind::MinusOneMinusOne), 0);
}

#[test]
fn issue_414_hovel_hurler_excludes_itself_and_pumps_the_other_creature() {
    let mut engine = engine_with(414_050, &["hovel_hurler"]);
    let hovel = move_ready_to_battlefield(&mut engine, 0, "hovel_hurler");
    let other = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 2,
            ..Default::default()
        },
    );

    // The printed "another" excludes the source: targeting the Hurler itself is rejected without
    // paying the cost.
    let selection = published_counter_selection(&mut engine, hovel);
    engine
        .apply_command(
            0,
            &activate_with_counter_cost(&engine, hovel, target_object(hovel), selection.clone()),
        )
        .expect_err("the source is not another creature");
    assert_eq!(
        engine.state.objects[&hovel].counter_count(CounterKind::MinusOneMinusOne),
        2
    );
    assert_eq!(engine.state.players[0].mana_pool.red, 2);
    assert!(engine.state.stack.is_empty());

    engine
        .apply_command(
            0,
            &activate_with_counter_cost(&engine, hovel, target_object(other), selection),
        )
        .expect("target the other creature");
    assert_eq!(
        engine.state.objects[&hovel].counter_count(CounterKind::MinusOneMinusOne),
        1
    );
    assert!(!engine.effective_has_keyword(other, Keyword::Flying));
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(stats(&engine, other).0, 3, "Grizzly Bears gets +1/+0");
    assert!(engine.effective_has_keyword(other, Keyword::Flying));
    assert!(
        !engine.effective_has_keyword(hovel, Keyword::Flying),
        "the Hurler itself does not gain flying"
    );
}

#[test]
fn issue_414_blocked_identities_stay_unregistered() {
    let registry = tricerules_cards::CardRegistry::global();
    for name in [
        "Flitterwing Nuisance",
        "Glen Elendra Guardian",
        "Slumbering Walker",
    ] {
        assert!(
            registry.id_for_name(name).is_none(),
            "{name} must stay fail-closed until its missing capability lands"
        );
    }
}

#[test]
fn issue_414_gnarlbark_elm_pays_one_minus_one_plus_one_stun_for_minus_two_minus_two() {
    let mut engine = engine_with(414_060, &["gnarlbark_elm"]);
    let gnarlbark = move_ready_to_battlefield(&mut engine, 0, "gnarlbark_elm");
    assert_eq!(
        counters(&engine, gnarlbark, CounterKind::MinusOneMinusOne),
        2,
        "Gnarlbark Elm enters with exactly two -1/-1 counters"
    );
    engine
        .state
        .objects
        .get_mut(&gnarlbark)
        .expect("gnarlbark")
        .add_counters(CounterKind::Stun, 1, 0);

    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    {
        let target_object_state = engine.state.objects.get_mut(&target).expect("target");
        target_object_state.power = Some(5);
        target_object_state.toughness = Some(5);
    }
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );

    let (payable, components) = published_counter_components(&mut engine, gnarlbark);
    assert!(payable, "two counters are available");
    let selections = vec![
        counter_selection_for(&components[0], CounterKind::MinusOneMinusOne),
        counter_selection_for(&components[1], CounterKind::Stun),
    ];
    engine
        .apply_command(
            0,
            &activate_with_counter_costs(&engine, gnarlbark, target_object(target), selections),
        )
        .expect("pay {2}{B} plus one -1/-1 counter and one stun counter");
    assert_eq!(
        counters(&engine, gnarlbark, CounterKind::MinusOneMinusOne),
        1,
        "one -1/-1 counter was removed as cost"
    );
    assert_eq!(
        counters(&engine, gnarlbark, CounterKind::Stun),
        0,
        "the stun counter was removed as cost"
    );
    assert_eq!(
        stats(&engine, target),
        (5, 5),
        "the pump has not resolved yet"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(stats(&engine, target), (3, 3), "the target gets -2/-2");
}

#[test]
fn issue_414_two_counter_cost_is_atomic_with_too_few_counters() {
    let mut engine = engine_with(414_070, &["gnarlbark_elm"]);
    let gnarlbark = move_ready_to_battlefield(&mut engine, 0, "gnarlbark_elm");
    engine
        .state
        .objects
        .get_mut(&gnarlbark)
        .expect("gnarlbark")
        .set_counter(CounterKind::MinusOneMinusOne, 1);
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );

    let (payable, components) = published_counter_components(&mut engine, gnarlbark);
    assert!(
        !payable,
        "one counter cannot satisfy two one-counter components"
    );
    let mana_before = engine.state.players[0].mana_pool.black;
    let selections = vec![
        counter_selection_for(&components[0], CounterKind::MinusOneMinusOne),
        counter_selection_for(&components[1], CounterKind::MinusOneMinusOne),
    ];
    engine
        .apply_command(
            0,
            &activate_with_counter_costs(&engine, gnarlbark, target_object(target), selections),
        )
        .expect_err("the joint counter assignment must fail closed");
    assert_eq!(
        counters(&engine, gnarlbark, CounterKind::MinusOneMinusOne),
        1,
        "no partial payment"
    );
    assert_eq!(engine.state.players[0].mana_pool.black, mana_before);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_414_reaping_willow_returns_a_small_creature_card_for_two_counters() {
    let mut engine = engine_with(414_080, &["reaping_willow"]);
    let reaping = move_ready_to_battlefield(&mut engine, 0, "reaping_willow");
    assert_eq!(
        counters(&engine, reaping, CounterKind::MinusOneMinusOne),
        2,
        "Reaping Willow enters with exactly two -1/-1 counters"
    );
    let visionary = inject_graveyard_card(&mut engine, 0, "elvish_visionary");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );

    let (payable, components) = published_counter_components(&mut engine, reaping);
    assert!(payable, "two counters are available");
    let selections = vec![
        counter_selection_for(&components[0], CounterKind::MinusOneMinusOne),
        counter_selection_for(&components[1], CounterKind::MinusOneMinusOne),
    ];
    engine
        .apply_command(
            0,
            &activate_with_counter_costs(&engine, reaping, graveyard_object(visionary), selections),
        )
        .expect("pay {1}{W/B} plus two -1/-1 counters");
    assert_eq!(
        counters(&engine, reaping, CounterKind::MinusOneMinusOne),
        0,
        "both counters are removed as cost"
    );
    assert!(
        engine.state.players[0].graveyard.contains(&visionary),
        "the card is still in the graveyard before resolution"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert!(
        engine.state.players[0].battlefield.contains(&visionary),
        "the creature card returns to the battlefield"
    );
    assert!(!engine.state.players[0].graveyard.contains(&visionary));
}

#[test]
fn issue_414_reaping_willow_rejects_illegal_graveyard_targets_without_paying() {
    let mut engine = engine_with(414_090, &["reaping_willow"]);
    let reaping = move_ready_to_battlefield(&mut engine, 0, "reaping_willow");
    let big = inject_graveyard_card(&mut engine, 0, "hill_giant");
    let spell = inject_graveyard_card(&mut engine, 0, "lightning_bolt");
    let small = inject_graveyard_card(&mut engine, 0, "elvish_visionary");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );

    for illegal in [big, spell] {
        let (_, components) = published_counter_components(&mut engine, reaping);
        let selections = vec![
            counter_selection_for(&components[0], CounterKind::MinusOneMinusOne),
            counter_selection_for(&components[1], CounterKind::MinusOneMinusOne),
        ];
        engine
            .apply_command(
                0,
                &activate_with_counter_costs(
                    &engine,
                    reaping,
                    graveyard_object(illegal),
                    selections,
                ),
            )
            .expect_err("only your creature cards with mana value 3 or less are legal");
        assert_eq!(
            counters(&engine, reaping, CounterKind::MinusOneMinusOne),
            2,
            "an illegal target pays nothing"
        );
        assert_eq!(engine.state.players[0].mana_pool.white, 1);
        assert!(engine.state.stack.is_empty());
    }

    let (_, components) = published_counter_components(&mut engine, reaping);
    let selections = vec![
        counter_selection_for(&components[0], CounterKind::MinusOneMinusOne),
        counter_selection_for(&components[1], CounterKind::MinusOneMinusOne),
    ];
    engine
        .apply_command(
            0,
            &activate_with_counter_costs(&engine, reaping, graveyard_object(small), selections),
        )
        .expect("mana value 2 is a legal target");
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.players[0].battlefield.contains(&small));
}
