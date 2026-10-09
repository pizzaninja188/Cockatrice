//! Actual-card coverage for Gratuitous Violence's creature-only damage replacement.

use super::helpers::*;
use tricerules_core::{GameEngine, TurnStep, Zone};

const CARD: &str = "gratuitous_violence";

fn setup(seed: u64) -> GameEngine {
    let deck = deck_with("mountain", &["grizzly_bears"]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("new two-player Gratuitous Violence engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn paid_gratuitous_violence(engine: &mut GameEngine) -> u32 {
    let object = inject_card_into_hand(engine, 0, CARD);
    give_mana(
        engine,
        0,
        ManaGift {
            c: 2,
            r: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, CARD);
    semantic::accepted(engine, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(engine);
    assert_eq!(engine.state.objects[&object].zone, Zone::Battlefield);
    object
}

fn advance_to_declare_attackers(engine: &mut GameEngine) {
    semantic::accepted(engine, 0, &primitive_yield());
    for _ in 0..12 {
        if engine.state.turn_step == TurnStep::DeclareAttackers {
            return;
        }
        pass_priority_round(engine);
    }
    panic!("active player did not reach Declare Attackers");
}

#[test]
fn gratuitous_violence_doubles_only_its_controllers_creature_damage() {
    let mut engine = setup(202_610_909);
    let violence = paid_gratuitous_violence(&mut engine);
    assert_eq!(
        engine.state.objects[&violence].card_id, CARD,
        "the actual permanent is the registered card"
    );

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
    semantic::accepted(&mut engine, 0, &cast_spell(slot, target_player(1)));
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&bolt].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.players[1].life, 17,
        "Lightning Bolt is an instant source, so the creature-only replacement does not apply"
    );

    let attacker = deploy_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    advance_to_declare_attackers(&mut engine);
    semantic::accepted(&mut engine, 0, &declare_attackers(vec![attacker]));
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareBlockers);
    semantic::accepted(&mut engine, 1, &declare_blockers(vec![]));
    pass_priority_round(&mut engine);

    assert_eq!(
        engine.state.players[1].life, 13,
        "the controlled 2/2 creature deals 4 combat damage"
    );
    assert_eq!(engine.state.objects[&attacker].damage, 0);
}
