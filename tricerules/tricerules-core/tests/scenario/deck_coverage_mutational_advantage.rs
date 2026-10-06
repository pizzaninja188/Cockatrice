//! Exact Mutational Advantage identity and rules behavior.

use super::helpers::*;
use tricerules_cards::primitives::SpellEffectKind;
use tricerules_cards::{CardRegistry, CounterKind, Keyword, ManaCost};
use tricerules_core::state::DamagePreventionScope;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{ruled_command::Cmd, ChoiceKind, SubmitResolutionChoice};

const MUTATIONAL_ADVANTAGE: &str = "mutational_advantage";

fn choice(objects: Vec<u32>, players: Vec<i32>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            chosen_object_ids: objects,
            chosen_player_ids: players,
            ..Default::default()
        })),
    }
}

fn set_fixture_controller(engine: &mut GameEngine, object_id: u32, from: usize, to: usize) {
    engine.state.players[from]
        .battlefield
        .retain(|candidate| *candidate != object_id);
    engine.state.players[to].battlefield.push(object_id);
    let controller = engine.state.players[to].id;
    let object = engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("fixture permanent");
    object.base_controller = controller;
    object.controller = controller;
}

#[test]
fn mutational_advantage_has_a_complete_card_definition() {
    let card = CardRegistry::global()
        .get(MUTATIONAL_ADVANTAGE)
        .expect("Mutational Advantage needs a complete definition");
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.mana_cost, ManaCost::parse("{1}{G}{U}").unwrap());
    assert_eq!(face.spell_effect.len(), 2);
    assert_eq!(
        face.spell_effect[0],
        SpellEffectKind::ProtectAllPermanentsYouControlWithCounters
    );
    assert_eq!(face.spell_effect[1], SpellEffectKind::Proliferate);
}

#[test]
fn protection_uses_one_resolution_snapshot_before_proliferate_then_prevents_later_damage() {
    let mut engine = GameEngine::new(
        20_261_006,
        &[0, 1],
        20,
        Some(vec![deck_with("forest", &[]), deck_with("mountain", &[])]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    let protected = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let unprotected = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let opponent_owned_controlled_by_us =
        inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    set_fixture_controller(&mut engine, opponent_owned_controlled_by_us, 1, 0);
    let us_owned_controlled_by_opponent =
        inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    set_fixture_controller(&mut engine, us_owned_controlled_by_opponent, 0, 1);
    let own_land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    engine
        .state
        .objects
        .get_mut(&protected)
        .unwrap()
        .set_counter(CounterKind::Charge, 1);
    engine
        .state
        .objects
        .get_mut(&opponent_owned_controlled_by_us)
        .unwrap()
        .set_counter(CounterKind::Charge, 1);
    engine
        .state
        .objects
        .get_mut(&us_owned_controlled_by_opponent)
        .unwrap()
        .set_counter(CounterKind::Charge, 1);
    engine
        .state
        .objects
        .get_mut(&opponent)
        .unwrap()
        .set_counter(CounterKind::Charge, 1);
    engine
        .state
        .objects
        .get_mut(&own_land)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 1);
    engine.state.players[1]
        .counters
        .insert(CounterKind::Poison, 1);

    inject_card_into_hand(&mut engine, 0, MUTATIONAL_ADVANTAGE);
    engine.state.players[0].mana_pool.colorless = 1;
    engine.state.players[0].mana_pool.green = 1;
    engine.state.players[0].mana_pool.blue = 1;
    let hand_index = hand_index_for_card(&engine, 0, MUTATIONAL_ADVANTAGE);
    engine
        .apply_command(0, &cast_spell(hand_index, vec![]))
        .expect("cast Mutational Advantage without targets");
    pass_both_players(&mut engine);

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the final Proliferate instruction parks for its existing choice");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::Proliferate);
    assert!(engine.effective_has_keyword(protected, Keyword::Hexproof));
    assert!(engine.effective_has_keyword(protected, Keyword::Indestructible));
    assert!(engine.effective_has_keyword(own_land, Keyword::Hexproof));
    assert!(engine.effective_has_keyword(own_land, Keyword::Indestructible));
    assert!(engine.effective_has_keyword(opponent_owned_controlled_by_us, Keyword::Hexproof));
    assert!(engine.effective_has_keyword(opponent_owned_controlled_by_us, Keyword::Indestructible));
    assert!(!engine.effective_has_keyword(unprotected, Keyword::Hexproof));
    assert!(!engine.effective_has_keyword(opponent, Keyword::Hexproof));
    assert!(!engine.effective_has_keyword(us_owned_controlled_by_opponent, Keyword::Hexproof));
    assert!(pending.presentation.candidates.contains(&protected));
    assert!(pending.presentation.candidates.contains(&own_land));
    assert!(pending.presentation.candidates.contains(&opponent));
    assert!(pending
        .presentation
        .candidates
        .contains(&opponent_owned_controlled_by_us));
    assert!(pending
        .presentation
        .candidates
        .contains(&us_owned_controlled_by_opponent));

    let before_rejection = format!("{:?}", engine.state);
    engine
        .apply_command(0, &choice(vec![unprotected], vec![]))
        .expect_err("Proliferate cannot select a permanent without a counter");
    assert_eq!(format!("{:?}", engine.state), before_rejection);
    engine
        .apply_command(
            0,
            &choice(
                vec![
                    protected,
                    own_land,
                    opponent,
                    opponent_owned_controlled_by_us,
                    us_owned_controlled_by_opponent,
                ],
                vec![1],
            ),
        )
        .expect("choose counter-bearing permanents and an opponent");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.objects[&protected].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(
        engine.state.objects[&own_land].counter_count(CounterKind::Loyalty),
        2
    );
    assert_eq!(
        engine.state.objects[&opponent].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(
        engine.state.objects[&opponent_owned_controlled_by_us].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(
        engine.state.objects[&us_owned_controlled_by_opponent].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(engine.state.players[1].counters[&CounterKind::Poison], 2);

    // Later counter changes must not re-evaluate the affected set from the card's ruling.
    engine
        .state
        .objects
        .get_mut(&protected)
        .unwrap()
        .set_counter(CounterKind::Charge, 0);
    engine
        .state
        .objects
        .get_mut(&unprotected)
        .unwrap()
        .set_counter(CounterKind::Charge, 1);
    assert!(engine.effective_has_keyword(protected, Keyword::Indestructible));
    assert!(!engine.effective_has_keyword(unprotected, Keyword::Indestructible));

    inject_card_into_hand(&mut engine, 0, "pyroclasm");
    engine.state.players[0].mana_pool.colorless = 1;
    engine.state.players[0].mana_pool.red = 1;
    let pyroclasm_hand_index = hand_index_for_card(&engine, 0, "pyroclasm");
    engine
        .apply_command(0, &cast_spell(pyroclasm_hand_index, vec![]))
        .expect("cast untargeted noncombat damage");
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&protected].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&protected].damage, 0);
    assert_eq!(engine.state.objects[&unprotected].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&opponent].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&us_owned_controlled_by_opponent].zone,
        Zone::Graveyard
    );
    assert_eq!(
        engine.state.objects[&opponent_owned_controlled_by_us].damage,
        0
    );
    assert!(engine.effective_has_keyword(protected, Keyword::Hexproof));
    assert!(!engine.effective_has_keyword(unprotected, Keyword::Hexproof));

    // A zone change ends both effects for this physical card, even if that card
    // later returns with the same ObjectId.
    let old_generation = engine
        .state
        .zone_change_generation
        .get(&protected)
        .copied()
        .unwrap_or_default();
    inject_card_into_hand(&mut engine, 0, "unsummon");
    engine.state.players[0].mana_pool.blue = 1;
    let unsummon_hand_index = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(
            0,
            &cast_spell(unsummon_hand_index, target_object(protected)),
        )
        .expect("target our own hexproof creature with Unsummon");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&protected].zone, Zone::Hand);
    assert!(!engine.effective_has_keyword(protected, Keyword::Hexproof));
    assert!(!engine.effective_has_keyword(protected, Keyword::Indestructible));
    assert!(!engine
        .state
        .damage_prevention_effects
        .iter()
        .any(|effect| { effect.scope == DamagePreventionScope::Recipient(protected) }));

    engine.state.players[0].mana_pool.colorless = 1;
    engine.state.players[0].mana_pool.green = 1;
    let returned_bear_hand_index = hand_index_for_card(&engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &cast_spell(returned_bear_hand_index, vec![]))
        .expect("recast the returned physical card");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&protected].zone, Zone::Battlefield);
    assert!(engine.state.zone_change_generation[&protected] > old_generation);
    assert!(!engine.effective_has_keyword(protected, Keyword::Hexproof));
    assert!(!engine.effective_has_keyword(protected, Keyword::Indestructible));
    assert!(!engine
        .state
        .damage_prevention_effects
        .iter()
        .any(|effect| { effect.scope == DamagePreventionScope::Recipient(protected) }));

    inject_card_into_hand(&mut engine, 0, "pyroclasm");
    engine.state.players[0].mana_pool.colorless = 1;
    engine.state.players[0].mana_pool.red = 1;
    let second_pyroclasm_hand_index = hand_index_for_card(&engine, 0, "pyroclasm");
    engine
        .apply_command(0, &cast_spell(second_pyroclasm_hand_index, vec![]))
        .expect("cast damage against the new incarnation");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&protected].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&opponent_owned_controlled_by_us].damage,
        0
    );

    end_active_turn(&mut engine, 0);
    assert!(!engine.effective_has_keyword(own_land, Keyword::Hexproof));
    assert!(!engine.effective_has_keyword(own_land, Keyword::Indestructible));
    assert!(!engine.effective_has_keyword(opponent_owned_controlled_by_us, Keyword::Hexproof));
    assert!(!engine.effective_has_keyword(opponent_owned_controlled_by_us, Keyword::Indestructible));
    assert!(
        !engine.state.damage_prevention_effects.iter().any(|effect| {
            matches!(
                effect.scope,
                DamagePreventionScope::Recipient(id)
                    if id == own_land || id == opponent_owned_controlled_by_us
            )
        })
    );
}
