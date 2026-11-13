//! Actual-card scenarios for the reviewed Treasure-on-entry creature family.
//!
//! Expectations are printed Oracle values. CR 603.6a stages the enters-the-battlefield
//! trigger; the canonical Treasure token creates one artifact and its mana ability adds
//! one mana of any color then ceases to exist (CR 111.1, CR 605).
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

fn cases() -> [(&'static str, (u32, u32)); 3] {
    [
        ("brazen_freebooter", (3, 3)),
        ("sailor_of_means", (1, 4)),
        ("wily_goblin", (1, 1)),
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[card, card]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    e
}

fn resolve_top_stack(engine: &mut GameEngine) -> tricerules_proto::ruled::v1::RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    engine
        .apply_scenario_command(first, &pass())
        .expect("first pass");
    engine
        .apply_scenario_command(second, &pass())
        .expect("second pass resolves stack item")
}

#[test]
fn brazen_freebooter_batch_cards_create_one_treasure_on_entry() {
    for (index, (id, body)) in cases().into_iter().enumerate() {
        let mut e = engine(880_000 + index as u64, id);
        let entered = move_ready_to_battlefield(&mut e, 0, id);
        assert_eq!(
            (e.effective_power(entered), e.effective_toughness(entered)),
            (Some(body.0), Some(body.1)),
            "{id}: printed body"
        );
        assert_eq!(e.state.stack.len(), 1, "{id}: one entry trigger");
        let resolved = resolve_top_stack(&mut e);
        let treasures = battlefield_token_oids(&e, 0, "treasure");
        assert_eq!(treasures.len(), 1, "{id}: exactly one Treasure");
        let created = token_created_events(&resolved);
        assert_eq!(created.len(), 1, "{id}");
        assert_eq!(created[0].card_id, "treasure", "{id}");
        let identity = created[0].identity.as_ref().expect("public token identity");
        assert_eq!(identity.name, "Treasure", "{id}");
        assert!(!identity.is_creature, "{id}");

        let mut activate = activate_ability_for(&e, treasures[0], 0, vec![]);
        let Some(Cmd::ActivateAbility(ability)) = activate.cmd.as_mut() else {
            unreachable!()
        };
        ability.mana_option_index = 3;
        e.apply_scenario_command(0, &activate)
            .expect("Treasure mana ability");
        assert_eq!(e.state.players[0].mana_pool.red, 1, "{id}");
        assert!(
            !e.state.objects.contains_key(&treasures[0]),
            "{id}: the sacrificed Treasure ceases to exist"
        );
        assert!(e.state.stack.is_empty(), "{id}");
    }
}

#[test]
fn brazen_freebooter_batch_cards_create_one_treasure_per_entry() {
    for (index, (id, _)) in cases().into_iter().enumerate() {
        let mut e = engine(881_000 + index as u64, id);
        move_ready_to_battlefield(&mut e, 0, id);
        resolve_top_stack(&mut e);
        assert_eq!(battlefield_token_oids(&e, 0, "treasure").len(), 1, "{id}");
        move_ready_to_battlefield(&mut e, 0, id);
        assert_eq!(e.state.stack.len(), 1, "{id}: the second entry triggers");
        resolve_top_stack(&mut e);
        assert_eq!(
            battlefield_token_oids(&e, 0, "treasure").len(),
            2,
            "{id}: one Treasure per entry, no more"
        );
        assert!(e.state.stack.is_empty(), "{id}");
    }
}
