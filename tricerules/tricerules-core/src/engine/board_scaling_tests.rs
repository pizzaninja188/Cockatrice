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
    engine
        .state
        .continuous_effects
        .push(crate::state::ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(thopters[0]),
            kind: ContinuousEffectKind::Layer6AddKeyword(Keyword::Vigilance),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 1,
        });
    for name in [
        "Ajani, Sleeper Agent",
        "Ornithopter",
        "Ajani, Sleeper Agent",
    ] {
        put(&mut engine, 0, name, rv1::DevZone::Hand);
    }
    engine.reconcile_activated_ability_slots();
    (engine, plains[0])
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
    // About 80 per permanent with the memo, 870 without; the cap leaves room for new rules.
    let permanents = 4 * 32;
    assert!(
        large <= 200 * permanents,
        "{large} derived evaluations for {permanents} permanents"
    );
}
