use crate::helpers::*;
use tricerules_cards::primitives::{
    Amount, CardResultAction, CardResultFilter, CardResultSource, CountExpression, DiscardQuantity,
    LibraryDrawReplacement, PlayerRecipient, RelativePlayerSet, SpellEffectKind, StaticAbilityDef,
};
use tricerules_cards::CardRegistry;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ChoiceKind, ResolutionChoiceDecision, SubmitResolutionChoice};

fn set_library_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let object_ids = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect::<Vec<_>>();
    engine.state.players[player]
        .library
        .retain(|object_id| !object_ids.contains(object_id));
    for object_id in object_ids.iter().rev() {
        engine.state.players[player].library.push_front(*object_id);
    }
    object_ids
}

fn choose_branch(
    engine: &mut GameEngine,
    player: i32,
    index: u32,
) -> tricerules_proto::ruled::v1::RuledEventBatch {
    engine
        .apply_command(
            player,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    decision: ResolutionChoiceDecision::SelectBranch as i32,
                    selected_branch_index: index,
                    ..Default::default()
                })),
            },
        )
        .expect("choose resolution branch")
}

#[test]
fn flux_has_its_complete_oracle_effect_sequence() {
    let face = CardRegistry::global()
        .get("flux")
        .expect("Flux is admitted as a complete card")
        .primary_face();

    assert_eq!(face.name, "Flux");
    assert_eq!(face.types, ["Sorcery"]);
    assert!(matches!(
        face.spell_effect.as_slice(),
        [
            SpellEffectKind::Discard {
                who: PlayerRecipient::EachPlayer,
                quantity: DiscardQuantity::AnyNumber,
            },
            SpellEffectKind::Draw {
                who: PlayerRecipient::EachPlayer,
                count: Amount::Count(CountExpression::CardsMatchingResultForAffectedPlayer {
                    filter: CardResultFilter {
                        source: CardResultSource::PreviousEffect,
                        action: CardResultAction::Discard,
                        players: RelativePlayerSet::All,
                        card_type: None,
                    },
                }),
            },
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }
        ]
    ));
}

#[test]
fn abundance_has_its_complete_draw_replacement() {
    let face = CardRegistry::global()
        .get("abundance")
        .expect("Abundance is admitted as a complete card")
        .primary_face();

    assert_eq!(face.name, "Abundance");
    assert_eq!(face.types, ["Enchantment"]);
    assert!(matches!(
        face.static_abilities.as_slice(),
        [ability]
            if matches!(
                ability.definition,
                StaticAbilityDef::ReplaceControllerDrawWithLibraryChoice {
                    kind: LibraryDrawReplacement::RevealUntilLandOrNonland,
                }
            )
    ));
}

#[test]
fn flux_uses_each_players_committed_discard_count() {
    let player_ids = [4, 9, 27];
    let decks = Some(vec![vec!["island".into(); 20]; player_ids.len()]);
    let mut engine = GameEngine::new(84001, &player_ids, 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    let drawn = [
        set_library_top(&mut engine, 0, &["grizzly_bears", "storm_crow"]),
        set_library_top(&mut engine, 1, &["savannah_lions"]),
        set_library_top(&mut engine, 2, &["forest", "mountain"]),
    ];
    let flux = inject_card_into_hand(&mut engine, 0, "flux");
    give_mana(
        &mut engine,
        player_ids[0],
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "flux");
    engine
        .apply_command(player_ids[0], &cast_spell(slot, vec![]))
        .expect("cast Flux");

    let mut batch = Default::default();
    for _ in 0..player_ids.len() {
        let actor = engine.state.priority_player_id();
        batch = engine.apply_command(actor, &pass()).expect("pass for Flux");
    }

    let discard_counts = [1, 0, 2];
    let mut discarded_by_player = [Vec::new(), Vec::new(), Vec::new()];
    for (player, expected_count) in discard_counts.into_iter().enumerate() {
        let choice = find_resolution_choice(&batch).expect("each player chooses a discard count");
        assert_eq!(choice.deciding_player_id, player_ids[player]);
        assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
        let selected = choice.candidate_object_ids[..expected_count].to_vec();
        discarded_by_player[player] = selected.clone();
        batch = engine
            .apply_command(player_ids[player], &submit_resolution_choice(selected))
            .expect("submit Flux discard choice");
    }

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.stack.len(), 0);
    for (player, discarded) in discarded_by_player.iter().enumerate() {
        assert!(discarded
            .iter()
            .all(|object_id| engine.state.players[player].graveyard.contains(object_id)));
    }
    assert!(drawn[0]
        .iter()
        .all(|object_id| engine.state.players[0].hand.contains(object_id)));
    assert!(drawn[1]
        .iter()
        .all(|object_id| engine.state.players[1].library.contains(object_id)));
    assert!(drawn[2]
        .iter()
        .all(|object_id| engine.state.players[2].hand.contains(object_id)));
    assert!(engine.state.players[0].graveyard.contains(&flux));
    assert_eq!(engine.state.objects[&drawn[0][1]].zone, Zone::Hand);
}

#[test]
fn flux_draws_interact_with_abundance_one_draw_at_a_time() {
    let player_ids = [4, 9, 27];
    let mut decks = vec![vec!["island".into(); 20]; player_ids.len()];
    decks[0] = deck_with("island", &["abundance"]);
    let mut engine = GameEngine::new(84002, &player_ids, 20, Some(decks), true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    move_ready_to_battlefield(&mut engine, 0, "abundance");

    let top = set_library_top(
        &mut engine,
        0,
        &[
            "grizzly_bears",
            "storm_crow",
            "llanowar_elves",
            "island",
            "forest",
        ],
    );
    let discarded = engine.state.players[0].hand[0];
    let flux = inject_card_into_hand(&mut engine, 0, "flux");
    give_mana(
        &mut engine,
        player_ids[0],
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "flux");
    engine
        .apply_command(player_ids[0], &cast_spell(slot, vec![]))
        .expect("cast Flux");

    let mut batch = Default::default();
    for _ in 0..player_ids.len() {
        let actor = engine.state.priority_player_id();
        batch = engine.apply_command(actor, &pass()).expect("pass for Flux");
    }
    for (player, expected_count) in [(0, 1), (1, 0), (2, 0)] {
        let choice = find_resolution_choice(&batch).expect("Flux discard choice");
        assert_eq!(choice.deciding_player_id, player_ids[player]);
        let selected = choice.candidate_object_ids[..expected_count].to_vec();
        batch = engine
            .apply_command(player_ids[player], &submit_resolution_choice(selected))
            .expect("submit Flux discard choice");
    }

    let replacement = find_resolution_choice(&batch).expect("Abundance replacement choice");
    assert_eq!(replacement.deciding_player_id, player_ids[0]);
    assert_eq!(replacement.choice_kind(), ChoiceKind::ResolutionBranch);
    batch = choose_branch(&mut engine, player_ids[0], 0); // replace the first draw
    let kind_choice = find_resolution_choice(&batch).expect("choose land or nonland");
    assert_eq!(kind_choice.choice_kind(), ChoiceKind::ResolutionBranch);
    batch = choose_branch(&mut engine, player_ids[0], 0); // choose land

    let reveal = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::CardsRevealed(reveal)) => Some(reveal.clone()),
            _ => None,
        })
        .expect("Abundance reveals through the first land");
    assert_eq!(
        reveal
            .cards
            .iter()
            .map(|card| card.object_id)
            .collect::<Vec<_>>(),
        top[..4]
    );
    let order_choice = find_resolution_choice(&batch).expect("order cards put on the bottom");
    assert_eq!(order_choice.choice_kind(), ChoiceKind::LibraryLook);
    assert!(order_choice.ordered);
    assert!(order_choice.public_reveal.is_none());
    assert_eq!(order_choice.candidate_object_ids, top[..3]);
    batch = engine
        .apply_command(
            player_ids[0],
            &submit_resolution_choice(vec![top[2], top[1], top[0]]),
        )
        .expect("order revealed nonlands on the bottom");

    let second_replacement =
        find_resolution_choice(&batch).expect("Abundance handles Flux's next draw separately");
    assert_eq!(
        second_replacement.choice_kind(),
        ChoiceKind::ResolutionBranch
    );
    batch = choose_branch(&mut engine, player_ids[0], 1); // decline replacement and draw normally

    assert!(find_resolution_choice(&batch).is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.players[0].hand.contains(&top[3]));
    assert!(engine.state.players[0].hand.contains(&top[4]));
    assert!(engine.state.players[0].graveyard.contains(&discarded));
    assert!(engine.state.players[0].graveyard.contains(&flux));
    let library = engine.state.players[0]
        .library
        .iter()
        .copied()
        .collect::<Vec<_>>();
    assert!(library.ends_with(&[top[2], top[1], top[0]]));
}
