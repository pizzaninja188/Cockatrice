//! Issue #116: live conditions on creature cohorts and automatic resolution branches.

use super::helpers::*;
use tricerules_cards::primitives::{CounterKind, Keyword};
use tricerules_proto::ruled::v1::dev_command::Dev;
use tricerules_proto::ruled::v1::{
    DevCommand, DevMoveCard, DevPutCardInZone, DevZone, RuledCommand,
};

fn dev(target: i32, payload: Dev) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: target,
            dev: Some(payload),
        })),
    }
}

fn put_ready(target: i32, card_name: &str) -> RuledCommand {
    dev(
        target,
        Dev::PutCardInZone(DevPutCardInZone {
            card_name: card_name.to_string(),
            zone: DevZone::Battlefield as i32,
            ready: true,
        }),
    )
}

fn move_to_hand(target: i32, card_name: &str) -> RuledCommand {
    dev(
        target,
        Dev::MoveCard(DevMoveCard {
            card_name: card_name.to_string(),
            zone: DevZone::Hand as i32,
            ready: false,
        }),
    )
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![vec!["plains".into(); 12], vec!["forest".into(); 12]]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new engine");
    engine.enable_dev_commands();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn put(engine: &mut GameEngine, player: i32, card_name: &str, card_id: &str) -> u32 {
    engine
        .apply_command(player, &put_ready(player, card_name))
        .unwrap_or_else(|error| panic!("put {card_name}: {error:?}"));
    battlefield_object_for_card(engine, player as usize, card_id)
}

fn cast_trade_route_envoy(engine: &mut GameEngine) -> u32 {
    let object_id = inject_card_into_hand(engine, 0, "trade_route_envoy");
    grant_pool(engine, 0);
    let hand_index = hand_index_for_card(engine, 0, "trade_route_envoy");
    engine
        .apply_command(0, &cast_spell(hand_index, Vec::new()))
        .expect("cast Trade Route Envoy");
    pass_both_players(engine);
    assert_eq!(
        engine.state.objects.get(&object_id).expect("envoy").zone,
        tricerules_core::Zone::Battlefield
    );
    object_id
}

#[test]
fn inspiring_paladin_conditions_its_self_and_countered_creature_first_strike() {
    let mut engine = engine(116_001);
    let paladin = put(&mut engine, 0, "Inspiring Paladin", "inspiring_paladin");
    let countered = put(&mut engine, 0, "Grizzly Bears", "grizzly_bears");
    let uncountered = put(&mut engine, 0, "Air Elemental", "air_elemental");
    engine
        .state
        .objects
        .get_mut(&countered)
        .expect("countered creature")
        .set_counter(CounterKind::PlusOnePlusOne, 1);

    assert!(engine.effective_has_keyword(paladin, Keyword::FirstStrike));
    assert!(engine.effective_has_keyword(countered, Keyword::FirstStrike));
    assert!(!engine.effective_has_keyword(uncountered, Keyword::FirstStrike));

    engine.state.active_player_idx = 1;
    assert!(!engine.effective_has_keyword(paladin, Keyword::FirstStrike));
    assert!(!engine.effective_has_keyword(countered, Keyword::FirstStrike));

    engine.state.active_player_idx = 0;
    engine
        .apply_command(0, &move_to_hand(0, "Inspiring Paladin"))
        .expect("remove anthem source");
    assert!(!engine.effective_has_keyword(countered, Keyword::FirstStrike));
}

/// CR 510.4: a creature with first strike deals its combat damage in the first combat damage
/// step and does not deal damage again in the regular combat damage step.
#[test]
fn inspiring_paladin_grant_changes_the_combat_damage_step() {
    let mut engine = engine(116_002);
    put(&mut engine, 0, "Inspiring Paladin", "inspiring_paladin");
    let attacker = put(&mut engine, 0, "Grizzly Bears", "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&attacker)
        .expect("attacker")
        .set_counter(CounterKind::PlusOnePlusOne, 1);

    engine
        .apply_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare countered attacker");
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.turn_step,
        tricerules_core::TurnStep::FirstStrikeDamage
    );
    assert_eq!(engine.state.players[1].life, 17);

    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.players[1].life, 17,
        "the first-strike attacker must not deal damage again in the regular step"
    );
}

#[test]
fn trade_route_envoy_uses_live_counter_state_without_prompting() {
    let mut draw_engine = engine(116_010);
    let qualifier = put(&mut draw_engine, 0, "Grizzly Bears", "grizzly_bears");
    let envoy = cast_trade_route_envoy(&mut draw_engine);
    draw_engine
        .state
        .objects
        .get_mut(&qualifier)
        .expect("qualifying creature")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    let hand_before = draw_engine.state.players[0].hand.len();

    pass_both_players(&mut draw_engine);

    assert!(draw_engine.state.pending_resolution.is_none());
    assert_eq!(draw_engine.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(
        draw_engine
            .state
            .objects
            .get(&envoy)
            .expect("envoy")
            .counter_count(CounterKind::PlusOnePlusOne),
        0
    );

    let mut fallback_engine = engine(116_011);
    let qualifier = put(&mut fallback_engine, 0, "Grizzly Bears", "grizzly_bears");
    fallback_engine
        .state
        .objects
        .get_mut(&qualifier)
        .expect("qualifying creature")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    let envoy = cast_trade_route_envoy(&mut fallback_engine);
    fallback_engine
        .state
        .objects
        .get_mut(&qualifier)
        .expect("qualifying creature")
        .set_counter(CounterKind::PlusOnePlusOne, 0);
    let hand_before = fallback_engine.state.players[0].hand.len();

    pass_both_players(&mut fallback_engine);

    assert!(fallback_engine.state.pending_resolution.is_none());
    assert_eq!(fallback_engine.state.players[0].hand.len(), hand_before);
    assert_eq!(
        fallback_engine
            .state
            .objects
            .get(&envoy)
            .expect("envoy")
            .counter_count(CounterKind::PlusOnePlusOne),
        1
    );
}
