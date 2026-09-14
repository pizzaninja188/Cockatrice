//! Issue #282: generated self-tap loot and rummage triggers observe only real
//! untapped-to-tapped transitions and keep their resolution choices private.

use crate::helpers::*;
use tricerules_cards::primitives::{ContinuousEffectKind, EffectDuration};
use tricerules_cards::CardRegistry;
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};

fn grant_llanowar_tap_ability(engine: &mut GameEngine, source: u32) {
    let ability = CardRegistry::global()
        .get("llanowar_elves")
        .expect("Llanowar Elves definition")
        .primary_face()
        .activated_abilities[0]
        .clone();
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::GrantActivatedAbility(Box::new(ability)),
        condition: None,
        duration: EffectDuration::WhileSourceOnBattlefield,
        timestamp: engine.state.command_index,
    });
}

fn move_hand_to_library(engine: &mut GameEngine, player: usize) {
    let moved: Vec<_> = engine.state.players[player].hand.drain(..).collect();
    for object_id in &moved {
        engine
            .state
            .objects
            .get_mut(object_id)
            .expect("hand object")
            .zone = Zone::Library;
    }
    engine.state.players[player].library.extend(moved);
}

#[test]
fn issue_282_attack_taps_the_source_but_entering_tapped_does_not() {
    let decks = Some(vec![
        deck_with("forest", &["mechan_navigator"]),
        forest_only_deck(),
    ]);
    let mut entering_tapped = GameEngine::new(282_001, &[0, 1], 20, decks.clone(), true)
        .expect("new entering-tapped engine");
    advance_to_main1_from_game_start(&mut entering_tapped);
    let entered_tapped = relocate_to_battlefield(&mut entering_tapped, 0, "mechan_navigator", true);
    assert!(entering_tapped.state.objects[&entered_tapped].tapped);
    assert!(
        entering_tapped.state.stack.is_empty(),
        "entering tapped does not create a self-becomes-tapped trigger"
    );

    let mut attacking =
        GameEngine::new(282_002, &[0, 1], 20, decks, true).expect("new attacking engine");
    advance_to_declare_attackers(&mut attacking);
    let attacker = inject_creature_on_battlefield(&mut attacking, 0, "mechan_navigator");
    attacking
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare Mechan Navigator as an attacker");
    assert!(attacking.state.objects[&attacker].tapped);
    assert_eq!(
        attacking.state.stack.len(),
        1,
        "attacking creates the self-becomes-tapped trigger"
    );
}

#[test]
fn issue_282_mandatory_loot_draws_before_the_private_discard_choice() {
    let mut engine = GameEngine::new(
        282_003,
        &[0, 1],
        20,
        Some(vec![forest_only_deck(), forest_only_deck()]),
        true,
    )
    .expect("new mandatory-loot engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_creature_on_battlefield(&mut engine, 0, "silvergill_peddler");
    grant_llanowar_tap_ability(&mut engine, source);
    let discard = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    let hand_before = engine.state.players[0].hand.clone();
    let library_before = engine.state.players[0].library.len();

    engine
        .apply_command(0, &activate_ability(source, 0, vec![]))
        .expect("activate the granted tap ability");
    assert!(engine.state.objects[&source].tapped);
    assert_eq!(engine.state.players[0].mana_pool.green, 1);
    assert_eq!(engine.state.stack.len(), 1, "loot trigger is on the stack");
    engine
        .apply_command(0, &pass())
        .expect("active player passes");
    let parked = engine
        .apply_command(1, &pass())
        .expect("trigger resolves to the discard choice");

    let choice = find_resolution_choice(&parked).expect("mandatory discard choice");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!((choice.min, choice.max), (1, 1));
    assert_eq!(
        engine.state.players[0].library.len(),
        library_before - 1,
        "the mandatory draw happens before the discard choice"
    );
    assert_eq!(engine.state.players[0].hand.len(), hand_before.len() + 1);
    let drawn = engine.state.players[0]
        .hand
        .iter()
        .find(|object_id| !hand_before.contains(object_id))
        .copied()
        .expect("drawn card in hand");
    assert!(
        choice.candidate_object_ids.contains(&drawn),
        "the newly drawn card is a legal discard candidate"
    );
    assert!(choice.candidate_object_ids.contains(&discard));

    engine
        .apply_command(0, &submit_resolution_choice(vec![discard]))
        .expect("discard the known card");
    assert!(engine.state.players[0].graveyard.contains(&discard));
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_282_other_effect_taps_optional_source_and_choice_is_controller_private() {
    let mut engine = GameEngine::new(
        282_004,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["cryptic_command"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("new other-effect engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_creature_on_battlefield(&mut engine, 1, "rescue_leopard");
    let bounced = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "cryptic_command");
    let cryptic = hand_index_for_card(&engine, 0, "cryptic_command");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 4,
            ..Default::default()
        },
    );

    engine
        .apply_command(
            0,
            &cast_modal_spell(cryptic, vec![(1, target_object(bounced)), (2, vec![])]),
        )
        .expect("cast Cryptic Command bounce-and-tap modes");
    pass_both_players(&mut engine);
    assert!(engine.state.objects[&source].tapped);
    assert_eq!(engine.state.objects[&bounced].zone, Zone::Hand);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "tap effect created the trigger"
    );

    let opponent_view = engine
        .apply_command(0, &pass())
        .expect("non-controller passes on the trigger");
    assert!(
        find_resolution_choice(&opponent_view).is_none(),
        "the non-controller must not receive a private hand choice"
    );
    let controller_view = engine
        .apply_command(1, &pass())
        .expect("controller receives the trigger choice");
    let choice = find_resolution_choice(&controller_view).expect("optional discard choice");
    assert_eq!(choice.deciding_player_id, 1);
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!((choice.min, choice.max), (0, 1));
    let discarded = choice
        .candidate_object_ids
        .first()
        .copied()
        .expect("controller has a discard candidate");
    let hand_before = engine.state.players[1].hand.clone();
    let library_before = engine.state.players[1].library.len();
    let opponent_attempt = engine.apply_command(0, &submit_resolution_choice(vec![discarded]));
    assert!(
        matches!(
            opponent_attempt,
            Err(tricerules_core::EngineError::Illegal(_))
        ),
        "only the trigger controller may submit its private choice"
    );
    assert_eq!(
        engine.state.players[1].library.len(),
        library_before,
        "optional discard does not draw before submission"
    );
    engine
        .apply_command(1, &submit_resolution_choice(vec![discarded]))
        .expect("controller accepts optional discard");
    assert!(engine.state.players[1].graveyard.contains(&discarded));
    assert_eq!(
        engine.state.players[1].library.len(),
        library_before - 1,
        "successful optional discard draws exactly one card"
    );
    assert_eq!(engine.state.players[1].hand.len(), hand_before.len());
    assert!(
        engine.state.players[1]
            .hand
            .iter()
            .any(|object_id| !hand_before.contains(object_id)),
        "successful optional discard leaves one newly drawn card in hand"
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_282_optional_decline_does_not_draw() {
    let mut engine = GameEngine::new(
        282_006,
        &[0, 1],
        20,
        Some(vec![forest_only_deck(), forest_only_deck()]),
        true,
    )
    .expect("new optional-decline engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_creature_on_battlefield(&mut engine, 0, "rescue_leopard");
    grant_llanowar_tap_ability(&mut engine, source);
    let library_before = engine.state.players[0].library.len();

    engine
        .apply_command(0, &activate_ability(source, 0, vec![]))
        .expect("activate the granted tap ability");
    engine
        .apply_command(0, &pass())
        .expect("active player passes");
    let resolved = engine
        .apply_command(1, &pass())
        .expect("resolve the optional trigger");
    let choice = find_resolution_choice(&resolved).expect("optional discard choice");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!((choice.min, choice.max), (0, 1));
    engine
        .apply_command(0, &submit_resolution_choice(vec![]))
        .expect("decline optional discard");
    assert_eq!(engine.state.players[0].library.len(), library_before);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_282_optional_empty_hand_cannot_pay_and_does_not_draw() {
    let mut engine = GameEngine::new(
        282_005,
        &[0, 1],
        20,
        Some(vec![forest_only_deck(), forest_only_deck()]),
        true,
    )
    .expect("new empty-hand engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_creature_on_battlefield(&mut engine, 0, "volatile_wanderglyph");
    grant_llanowar_tap_ability(&mut engine, source);
    move_hand_to_library(&mut engine, 0);
    let library_before = engine.state.players[0].library.len();

    engine
        .apply_command(0, &activate_ability(source, 0, vec![]))
        .expect("activate the granted tap ability");
    engine
        .apply_command(0, &pass())
        .expect("active player passes");
    let resolved = engine
        .apply_command(1, &pass())
        .expect("resolve the optional trigger");
    assert!(find_resolution_choice(&resolved).is_none());
    assert_eq!(engine.state.players[0].library.len(), library_before);
    assert!(engine.state.stack.is_empty());
}
