//! Board-scaling guard for one ordinary command. Counts full derived-query evaluations rather
//! than wall time, so it is deterministic and also holds in the debug-assertion test profile.
use super::*;

fn put(engine: &mut GameEngine, player: PlayerId, name: &str, zone: rv1::DevZone) -> ObjectId {
    let before = engine
        .state
        .objects
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    engine
        .apply_dev_command(
            player,
            &rv1::DevCommand {
                target_player_id: player,
                dev: Some(rv1::dev_command::Dev::PutCardInZone(
                    rv1::DevPutCardInZone {
                        card_name: name.into(),
                        zone: zone as i32,
                        ready: true,
                    },
                )),
            },
        )
        .expect("dev put");
    let created = engine
        .state
        .objects
        .keys()
        .copied()
        .filter(|oid| !before.contains(oid))
        .collect::<Vec<_>>();
    assert_eq!(created.len(), 1, "dev put creates one object");
    created[0]
}

/// A long-game board: `n` each of Plains, Forest and Ornithopter for the active player, `n`
/// Plains for the opponent, and castable spells in hand so legal-action generation runs both
/// the per-ability and per-spell payment witnesses. A live keyword grant on one Ornithopter
/// stands in for the effects an Ajani turn leaves behind, as in the long game this models.
fn long_game_board(n: usize) -> (GameEngine, ObjectId) {
    let (engine, plains, _) = board_with(
        n,
        Some(ContinuousEffectKind::Layer6AddKeyword(Keyword::Vigilance)),
        &[
            "Ajani, Sleeper Agent",
            "Ornithopter",
            "Ajani, Sleeper Agent",
        ],
    );
    (engine, plains)
}

/// The long-game board with an optional effect on the first Ornithopter and a chosen hand.
/// Returns the engine, the first Plains and the hand objects in order.
fn board_with(
    n: usize,
    effect: Option<ContinuousEffectKind>,
    hand: &[&str],
) -> (GameEngine, ObjectId, Vec<ObjectId>) {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        22_112_026,
        &[0, 1],
        20,
        // Basic-land decks keep the opening hands free of spells this board cannot pay for.
        Some(vec![vec!["plains".to_string(); 40]; 2]),
        true,
    )
    .unwrap();
    engine.state.opening = None;
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = 0;
    let mut plains = Vec::new();
    let mut thopters = Vec::new();
    for _ in 0..n {
        plains.push(put(&mut engine, 0, "Plains", rv1::DevZone::Battlefield));
        put(&mut engine, 0, "Forest", rv1::DevZone::Battlefield);
        thopters.push(put(
            &mut engine,
            0,
            "Ornithopter",
            rv1::DevZone::Battlefield,
        ));
        put(&mut engine, 1, "Plains", rv1::DevZone::Battlefield);
    }
    if let Some(kind) = effect {
        engine
            .state
            .continuous_effects
            .push(crate::state::ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: AffectedScope::Single(thopters[0]),
                kind,
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: 1,
            });
    }
    let hand = hand
        .iter()
        .map(|name| put(&mut engine, 0, name, rv1::DevZone::Hand))
        .collect();
    engine.reconcile_activated_ability_slots();
    (engine, plains[0], hand)
}

/// Legal actions for player 0, as the client receives them after a command.
fn legal(engine: &GameEngine) -> rv1::LegalActions {
    let mut batch = RuledEventBatch::default();
    super::legal_actions::fill_legal(&mut batch, engine);
    batch
        .legal_by_player
        .remove(&0)
        .expect("player 0 legal actions")
}

fn affordable(engine: &GameEngine, actions: &rv1::LegalActions, oid: ObjectId) -> bool {
    actions
        .affordable_hand_indices
        .iter()
        .any(|&slot| engine.state.players[0].hand[slot as usize] == oid)
}

fn probes_during<T>(probe: super::read_memo::Probe, run: impl FnOnce() -> T) -> (T, u64) {
    let before = super::read_memo::probes(probe);
    let value = run();
    (value, super::read_memo::probes(probe) - before)
}

fn evaluations_for_one_mana_tap(n: usize) -> u64 {
    let (mut engine, plains) = long_game_board(n);
    let ability_index = engine.effective_activated_abilities(plains)[0].slot;
    let command = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
            rv1::ActivateAbility {
                source_object_id: plains,
                expected_zone_change_generation: engine
                    .state
                    .zone_change_generation
                    .get(&plains)
                    .copied()
                    .unwrap_or(0),
                ability_index,
                source_zone: rv1::AbilitySourceZone::Battlefield as i32,
                ..Default::default()
            },
        )),
    };
    let before = super::read_memo::evaluations();
    engine
        .apply_command(0, &command)
        .expect("tap Plains for mana");
    super::read_memo::evaluations() - before
}

#[test]
fn one_command_on_a_long_game_board_scales_linearly_with_permanents() {
    // Permanents: 4n, so the board grows 4x from n = 8 to n = 32 and linear work grows ~4x.
    // Without the read-scope memo, per-permanent queries re-derived every permanent's
    // characteristics and statics: 13,790 -> 111,518 evaluations (8.1x). With it: ~3.9x.
    let small = evaluations_for_one_mana_tap(8);
    let large = evaluations_for_one_mana_tap(32);
    let ratio = large as f64 / small as f64;
    assert!(
        ratio <= 5.0,
        "derived evaluations grew {ratio:.2}x for a 4x board ({small} -> {large}); \
         expected roughly linear"
    );
    // About 55 per permanent now (80 with the memo alone, 870 without); the cap leaves room
    // for new rules.
    let permanents = 4 * 32;
    assert!(
        large <= 120 * permanents,
        "{large} derived evaluations for {permanents} permanents"
    );
}

#[test]
fn tap_mana_checks_do_not_clone_the_engine_without_tap_observers() {
    // No effect, no triggered abilities: tapping any land can only add mana. Proving that
    // used to clone the whole engine once per land, so the check alone was O(boardÂ²).
    let (engine, _, _) = board_with(16, None, &["Lightning Bolt"]);
    let (_, clones) = probes_during(super::read_memo::Probe::TapCheckClone, || legal(&engine));
    assert_eq!(clones, 0, "tap checks cloned the engine {clones} times");
}

#[test]
fn unpayable_spell_beside_a_mana_inert_effect_is_unavailable_without_a_search() {
    // A Vigilance grant cannot change what any land produces, so Lightning Bolt with only
    // Plains and Forests is provably unpayable. It used to run the 512-node search instead.
    let (engine, _, hand) = board_with(
        16,
        Some(ContinuousEffectKind::Layer6AddKeyword(Keyword::Vigilance)),
        &["Lightning Bolt"],
    );
    let (actions, clones) = probes_during(super::read_memo::Probe::PaymentSearchClone, || {
        legal(&engine)
    });
    assert!(!affordable(&engine, &actions, hand[0]));
    assert_eq!(clones, 0, "payment search cloned the engine {clones} times");
    assert_ne!(
        actions.automation.as_ref().map(|a| a.stop_reason),
        Some(rv1::AutomationStopReason::Uncertain as i32),
        "an unpayable Bolt must be a proof, not an uncertain stop"
    );
}

#[test]
fn payment_search_beside_an_ability_removing_effect_fits_a_two_megabyte_stack() {
    // Ability removal is not mana-inert, so the bounded search still runs. With 96 lands it
    // reaches the full activation depth, which used to overflow a default 2 MB test-thread
    // stack (about 52 KB per level in debug builds). Bolt is the only searched cost here, so
    // its clones stay within one search budget instead of re-cloning at the depth cap.
    std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(|| {
            let (engine, _, hand) = board_with(
                48,
                Some(ContinuousEffectKind::Layer6RemoveAllAbilities),
                &["Lightning Bolt"],
            );
            let (actions, clones) =
                probes_during(super::read_memo::Probe::PaymentSearchClone, || {
                    legal(&engine)
                });
            assert!(!affordable(&engine, &actions, hand[0]));
            assert!(clones <= 512, "{clones} search clones");
        })
        .unwrap()
        .join()
        .expect("payment search overflowed or panicked on a 2 MB stack");
}

fn snapshots_for_one_mana_tap(n: usize) -> u64 {
    let (mut engine, plains) = long_game_board(n);
    let ability_index = engine.effective_activated_abilities(plains)[0].slot;
    let command = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
            rv1::ActivateAbility {
                source_object_id: plains,
                expected_zone_change_generation: engine
                    .state
                    .zone_change_generation
                    .get(&plains)
                    .copied()
                    .unwrap_or(0),
                ability_index,
                source_zone: rv1::AbilitySourceZone::Battlefield as i32,
                ..Default::default()
            },
        )),
    };
    probes_during(super::read_memo::Probe::TriggerSourceSnapshot, || {
        engine
            .apply_command(0, &command)
            .expect("tap Plains for mana")
    })
    .1
}

#[test]
fn one_command_snapshots_only_permanents_that_can_trigger() {
    // Every permanent on this board is a land or a vanilla Ornithopter. None has a triggered
    // ability, so trigger collection, tap snapshots and SBA zone snapshots need none of them.
    let snapshots = snapshots_for_one_mana_tap(32);
    assert!(
        snapshots <= 16,
        "{snapshots} trigger-source snapshots for one mana tap on a 128-permanent board"
    );
}
