//! Actual-card semantic checks for five Standard life-gain identities.
//! Oracle and rulings endpoints checked 2026-09-22; no card-specific rulings.
//! CR 119.3, 121.1, 508.3a, 510.2, 603.2/603.6a, 602.5, and 605.1a govern the behavior.

use super::helpers::*;
use tricerules_cards::Keyword;

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

fn cast(e: &mut GameEngine, player: i32, card_id: &str) {
    inject_card_into_hand(e, player as usize, card_id);
    grant_pool(e, player as usize);
    let slot = hand_index_for_card(e, player as usize, card_id);
    semantic::accepted(e, player, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(e);
}

#[test]
fn issue_misc24_potioners_trove_requires_own_instant_or_sorcery_for_life() {
    let mut e = engine(824_001);
    let trove = inject_permanent_on_battlefield(&mut e, 0, "potioners_trove");
    assert!(e
        .apply_command(0, &activate_ability(trove, 1, vec![]))
        .is_err());
    assert!(
        !e.state.objects[&trove].tapped,
        "rejected activation cannot pay tap cost"
    );

    cast(&mut e, 0, "grizzly_bears");
    assert!(
        e.apply_command(0, &activate_ability(trove, 1, vec![]))
            .is_err(),
        "an own creature spell does not meet the instant-or-sorcery condition"
    );
    inject_card_into_hand(&mut e, 1, "lightning_bolt");
    grant_pool(&mut e, 1);
    let opponent_slot = hand_index_for_card(&e, 1, "lightning_bolt");
    semantic::accepted(&mut e, 0, &pass());
    semantic::accepted(&mut e, 1, &cast_spell(opponent_slot, target_player(0)));
    resolve_entire_stack_two_player(&mut e);
    assert!(
        e.apply_command(0, &activate_ability(trove, 1, vec![]))
            .is_err(),
        "an opponent's instant does not meet the controller-relative condition"
    );

    inject_card_into_hand(&mut e, 0, "lightning_bolt");
    let slot = hand_index_for_card(&e, 0, "lightning_bolt");
    semantic::accepted(&mut e, 0, &cast_spell(slot, target_player(1)));
    resolve_entire_stack_two_player(&mut e);
    let life = e.state.players[0].life;
    semantic::accepted(&mut e, 0, &activate_ability(trove, 1, vec![]));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, life + 2);
    assert!(e.state.objects[&trove].tapped);

    let mut mana = engine(824_011);
    let source = inject_permanent_on_battlefield(&mut mana, 0, "potioners_trove");
    semantic::accepted(&mut mana, 0, &activate_ability(source, 0, vec![]));
    assert!(
        mana.state.objects[&source].tapped,
        "mana ability taps without a spell prerequisite"
    );
}

#[test]
fn issue_misc24_wylie_duke_gains_and_draws_when_tapped() {
    let mut e = engine(824_002);
    let wylie = inject_creature_on_battlefield(&mut e, 0, "wylie_duke,_atiin_hero");
    assert!(e.effective_has_keyword(wylie, Keyword::Vigilance));
    let icy = inject_permanent_on_battlefield(&mut e, 1, "icy_manipulator");
    let before_life = e.state.players[0].life;
    let before_hand = e.state.players[0].hand.len();
    semantic::accepted(&mut e, 0, &pass());
    semantic::accepted(&mut e, 1, &activate_ability(icy, 0, target_object(wylie)));
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.objects[&wylie].tapped);
    assert_eq!(e.state.players[0].life, before_life + 1);
    assert_eq!(e.state.players[0].hand.len(), before_hand + 1);
}

#[test]
fn issue_misc24_jeskai_shrinekeeper_rewards_combat_damage_to_player() {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(824_003, &[0, 1], 20, decks, true).expect("engine");
    advance_to_declare_attackers(&mut e);
    let dragon = inject_creature_with_stats(&mut e, 0, "jeskai_shrinekeeper", 3, 3);
    assert!(e.effective_has_keyword(dragon, Keyword::Flying));
    assert!(e.effective_has_keyword(dragon, Keyword::Haste));
    let before_life = e.state.players[0].life;
    let before_their_life = e.state.players[1].life;
    let before_hand = e.state.players[0].hand.len();
    semantic::accepted(&mut e, 0, &declare_attackers(vec![dragon]));
    assert_eq!(
        e.state.players[0].life, before_life,
        "attacking alone does not trigger it"
    );
    semantic::accepted(&mut e, 0, &pass());
    semantic::accepted(&mut e, 1, &pass());
    semantic::accepted(&mut e, 0, &pass());
    semantic::accepted(&mut e, 1, &pass());
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[1].life,
        before_their_life - 3,
        "three combat damage"
    );
    assert_eq!(e.state.players[0].life, before_life + 1);
    assert_eq!(e.state.players[0].hand.len(), before_hand + 1);
}

#[test]
fn issue_misc24_pactdoll_counts_self_and_own_artifacts_only() {
    let mut e = engine(824_004);
    cast(&mut e, 0, "pactdoll_terror");
    assert_eq!(
        (e.state.players[0].life, e.state.players[1].life),
        (21, 19),
        "own entry triggers once"
    );
    cast(&mut e, 0, "grizzly_bears");
    assert_eq!((e.state.players[0].life, e.state.players[1].life), (21, 19));
    cast(&mut e, 0, "magitek_armor");
    assert_eq!((e.state.players[0].life, e.state.players[1].life), (22, 18));

    // Put the Pactdoll under player 0's control while player 1 is the active player,
    // then let player 1 cast an artifact at sorcery speed.
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
    let mut opponent = GameEngine::new(824_014, &[1, 0], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut opponent);
    inject_permanent_on_battlefield(&mut opponent, 1, "pactdoll_terror");
    inject_card_into_hand(&mut opponent, 0, "magitek_armor");
    grant_pool(&mut opponent, 0);
    let slot = hand_index_for_card(&opponent, 0, "magitek_armor");
    semantic::accepted(&mut opponent, 1, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(&mut opponent);
    assert_eq!(
        opponent
            .state
            .players
            .iter()
            .map(|p| p.life)
            .collect::<Vec<_>>(),
        vec![20, 20],
        "the opponent's artifact cannot trigger Pactdoll"
    );
}

#[test]
fn issue_misc24_shroudstomper_has_entry_and_attack_occurrences() {
    let mut e = engine(824_005);
    let before_hand = e.state.players[0].hand.len();
    cast(&mut e, 0, "shroudstomper");
    assert_eq!((e.state.players[0].life, e.state.players[1].life), (22, 18));
    assert_eq!(e.state.players[0].hand.len(), before_hand + 1);

    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut attack = GameEngine::new(824_015, &[0, 1], 20, decks, true).expect("engine");
    advance_to_declare_attackers(&mut attack);
    let shroud = inject_creature_on_battlefield(&mut attack, 0, "shroudstomper");
    assert!(attack.effective_has_keyword(shroud, Keyword::Deathtouch));
    let before_hand = attack.state.players[0].hand.len();
    semantic::accepted(&mut attack, 0, &declare_attackers(vec![shroud]));
    resolve_entire_stack_two_player(&mut attack);
    assert_eq!(
        (attack.state.players[0].life, attack.state.players[1].life),
        (22, 18)
    );
    assert_eq!(attack.state.players[0].hand.len(), before_hand + 1);
}
