//! Issue #323 — the eight reviewed single-clause completion cards.
//!
//! These scenarios drive the generated definitions through the authoritative command path.
//! CR 701.7/614 govern mass destruction and its indestructible/regeneration boundaries; CR 701.6
//! and 118.12a govern the {2} soft counter and its payer; CR 702.4/514.2 govern the double strike
//! grant and its cleanup expiry; CR 111.10a governs the predefined Treasure and its mana ability;
//! CR 701.23/614.1d govern the basic-land search onto the battlefield tapped; CR 611.3 governs the
//! live trample anthem; and CR 509.1b governs the unblockable grant and the single-blocker limit.

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, BlockPair, ChoiceKind, ResolutionChoiceDecision,
};

fn deck_engine(seed: u64, own: &[&str], opposing: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", own),
        deck_with("forest", opposing),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn advance_main1_to_declare_attackers(engine: &mut GameEngine) {
    engine
        .apply_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    engine
        .apply_command(0, &pass())
        .expect("active player passes in beginning of combat");
    engine
        .apply_command(1, &pass())
        .expect("defender passes in beginning of combat");
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
}

fn pass_to_declare_blockers(engine: &mut GameEngine) -> RuledEventBatch {
    engine
        .apply_command(0, &pass())
        .expect("active player passes after declaring attackers");
    engine
        .apply_command(1, &pass())
        .expect("defender passes after attackers are declared")
}

fn has_log(batch: &RuledEventBatch, text: &str) -> bool {
    batch
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == text))
}

#[test]
fn issue_323_day_of_judgment_destroys_all_with_indestructible_and_regeneration_boundaries() {
    let decks = Some(vec![
        deck_with(
            "plains",
            &[
                "day_of_judgment",
                "grizzly_bears",
                "darksteel_myr",
                "drudge_skeletons",
            ],
        ),
        deck_with("plains", &["savannah_lions"]),
    ]);
    let mut engine = GameEngine::new(323_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    let bears = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let myr = relocate_to_battlefield(&mut engine, 0, "darksteel_myr", false);
    let skeletons = relocate_to_battlefield(&mut engine, 0, "drudge_skeletons", false);
    engine
        .state
        .objects
        .get_mut(&skeletons)
        .expect("skeletons")
        .regeneration_shields = 1;
    let lions = relocate_to_battlefield(&mut engine, 1, "savannah_lions", false);
    ensure_in_hand(&mut engine, 0, "day_of_judgment");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 4,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, "day_of_judgment");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Day of Judgment");
    engine.apply_command(0, &pass()).expect("caster passes");
    let completion = engine
        .apply_command(1, &pass())
        .expect("opponent passes into resolution");

    let moved = permanents_moved_in(&completion);
    for destroyed in [bears, lions] {
        assert_eq!(
            engine.state.objects[&destroyed].zone,
            Zone::Graveyard,
            "the ordinary creature is destroyed"
        );
        assert!(
            moved.iter().any(|event| event.object_id == destroyed),
            "the destruction is published in the shared resolution batch"
        );
    }
    assert_eq!(
        engine.state.objects[&myr].zone,
        Zone::Battlefield,
        "CR 702.12b: an indestructible creature survives destroy-all"
    );
    assert_eq!(
        engine.state.objects[&skeletons].zone,
        Zone::Battlefield,
        "a regeneration shield replaces the ordinary destruction"
    );
    assert!(
        engine.state.objects[&skeletons].tapped,
        "regeneration taps the saved creature"
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_323_itll_quench_ya_charges_two_and_counters_on_decline() {
    let decks = Some(vec![
        deck_with("island", &["lightning_bolt"]),
        deck_with("island", &["itll_quench_ya!"]),
    ]);
    let mut engine = GameEngine::new(323_010, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    ensure_in_hand(&mut engine, 0, "lightning_bolt");
    let bolt_slot = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_command(0, &cast_spell(bolt_slot, target_player(1)))
        .expect("cast Lightning Bolt");
    let bolt = engine.state.stack.last().expect("bolt on stack").id;
    engine.apply_command(0, &pass()).expect("pass priority");

    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    ensure_in_hand(&mut engine, 1, "itll_quench_ya!");
    let quench_slot = hand_index_for_card(&engine, 1, "itll_quench_ya!");
    engine
        .apply_command(1, &cast_spell(quench_slot, target_object(bolt)))
        .expect("cast It'll Quench Ya! at the Bolt");
    let quench = engine.state.stack.last().expect("Quench on stack").id;

    engine.apply_command(1, &pass()).expect("caster passes");
    let parked = engine.apply_command(0, &pass()).expect("resolution parks");

    let choice = find_resolution_choice(&parked).expect("unless-pays choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::ManaPayment);
    assert_eq!(
        choice.deciding_player_id, 0,
        "the countered spell's controller pays"
    );
    assert_eq!(choice.generic_mana_cost, 2);

    let completion = engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline the {2} payment");
    assert!(engine.state.players[0].graveyard.contains(&bolt));
    assert!(engine.state.players[1].graveyard.contains(&quench));
    assert!(engine.state.stack.is_empty());
    assert!(
        permanents_moved_in(&completion)
            .iter()
            .any(|event| event.object_id == bolt),
        "the countered Bolt is published as moving to its owner's graveyard"
    );
}

#[test]
fn issue_323_itll_quench_ya_payment_preserves_the_targeted_spell() {
    let decks = Some(vec![
        deck_with("island", &["lightning_bolt"]),
        deck_with("island", &["itll_quench_ya!"]),
    ]);
    let mut engine = GameEngine::new(323_011, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    ensure_in_hand(&mut engine, 0, "lightning_bolt");
    let bolt_slot = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_command(0, &cast_spell(bolt_slot, target_player(1)))
        .expect("cast Lightning Bolt");
    let bolt = engine.state.stack.last().expect("bolt on stack").id;
    engine.apply_command(0, &pass()).expect("pass priority");

    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    ensure_in_hand(&mut engine, 1, "itll_quench_ya!");
    let quench_slot = hand_index_for_card(&engine, 1, "itll_quench_ya!");
    engine
        .apply_command(1, &cast_spell(quench_slot, target_object(bolt)))
        .expect("cast soft counter");
    engine.apply_command(1, &pass()).expect("caster passes");
    engine
        .apply_command(0, &pass())
        .expect("resolution parks a payment");

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay {2}");
    assert!(engine.state.stack.iter().any(|item| item.id == bolt));
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[1].life, 17, "the Bolt still resolves");
}

#[test]
fn issue_323_twice_the_rage_grants_double_strike_until_end_of_turn() {
    let mut engine = deck_engine(
        323_020,
        &["two-headed_hunter_twice_the_rage"],
        &["grizzly_bears"],
    );
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    assert!(!engine.effective_has_keyword(target, Keyword::DoubleStrike));

    ensure_in_hand(&mut engine, 0, "two-headed_hunter_twice_the_rage");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "two-headed_hunter_twice_the_rage");
    engine
        .apply_command(0, &cast_spell_face(slot, target_object(target), 1))
        .expect("cast Twice the Rage");
    resolve_entire_stack_two_player(&mut engine);
    assert!(
        engine.effective_has_keyword(target, Keyword::DoubleStrike),
        "the chosen creature has double strike"
    );

    end_active_turn(&mut engine, 0);
    assert!(
        !engine.effective_has_keyword(target, Keyword::DoubleStrike),
        "the grant expires at cleanup"
    );
}

#[test]
fn issue_323_ancestors_aid_pumps_and_creates_a_usable_treasure() {
    let mut engine = deck_engine(323_030, &["ancestors_aid"], &[]);
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let base = engine
        .characteristics(target)
        .expect("bear characteristics");

    ensure_in_hand(&mut engine, 0, "ancestors_aid");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "ancestors_aid");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Ancestors' Aid");
    engine.apply_command(0, &pass()).expect("caster passes");
    let completion = engine
        .apply_command(1, &pass())
        .expect("opponent passes into resolution");

    let pumped = engine
        .characteristics(target)
        .expect("pumped characteristics");
    assert_eq!(pumped.power, base.power.map(|power| power + 2));
    assert!(engine.effective_has_keyword(target, Keyword::FirstStrike));
    let treasure = battlefield_token_oids(&engine, 0, "treasure");
    assert_eq!(treasure.len(), 1, "Ancestors' Aid creates one Treasure");
    assert!(token_created_events(&completion)
        .iter()
        .any(|token| token.card_id == "treasure"));

    let mut activate = activate_ability_for(&engine, treasure[0], 0, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = activate.cmd.as_mut() else {
        unreachable!()
    };
    ability.mana_option_index = 3;
    let red_before = engine.state.players[0].mana_pool.red;
    engine
        .apply_command(0, &activate)
        .expect("canonical Treasure mana ability");
    assert_eq!(engine.state.players[0].mana_pool.red, red_before + 1);
    assert!(
        !engine.state.objects.contains_key(&treasure[0]),
        "the sacrificed Treasure ceases to exist"
    );
}

#[test]
fn issue_323_shared_roots_searches_a_tapped_basic_land_and_shuffles() {
    let decks = Some(vec![
        deck_with("forest", &["shared_roots"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(323_040, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    let forest = inject_library_card(&mut engine, 0, "forest");
    let taiga = inject_library_card(&mut engine, 0, "taiga");
    ensure_in_hand(&mut engine, 0, "shared_roots");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "shared_roots");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Shared Roots");
    engine.apply_command(0, &pass()).expect("caster passes");
    let search_batch = engine
        .apply_command(1, &pass())
        .expect("opponent passes into resolution");

    let choice = find_resolution_choice(&search_batch).expect("basic-land search");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert!(choice.candidate_object_ids.contains(&forest));
    assert!(
        !choice.candidate_object_ids.contains(&taiga),
        "Taiga is a land but not a basic land"
    );

    let completion = engine
        .apply_command(0, &submit_resolution_choice(vec![forest]))
        .expect("choose the basic land");
    let object = &engine.state.objects[&forest];
    assert_eq!(object.zone, Zone::Battlefield);
    assert!(object.tapped, "the searched basic land enters tapped");
    assert!(!engine.state.players[0].library.contains(&forest));
    assert!(engine.state.players[0].battlefield.contains(&forest));
    assert!(
        has_log(&completion, "P0 shuffles their library."),
        "the search shuffles after the land enters"
    );
}

#[test]
fn issue_323_aggressive_mammoth_anthem_is_live_and_excludes_itself() {
    let decks = Some(vec![
        deck_with("forest", &["aggressive_mammoth", "go_for_the_throat"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(323_050, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    let mammoth = move_ready_to_battlefield(&mut engine, 0, "aggressive_mammoth");
    let first = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    assert!(engine.effective_has_keyword(mammoth, Keyword::Trample));
    assert!(
        engine.effective_has_keyword(first, Keyword::Trample),
        "other creatures you control gain trample"
    );

    let late = inject_creature_on_battlefield(&mut engine, 0, "savannah_lions");
    assert!(
        engine.effective_has_keyword(late, Keyword::Trample),
        "the anthem continuously reevaluates creatures that enter later"
    );

    inject_card_into_hand(&mut engine, 0, "go_for_the_throat");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "go_for_the_throat");
    engine
        .apply_command(0, &cast_spell(slot, target_object(mammoth)))
        .expect("destroy the Mammoth");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&mammoth].zone, Zone::Graveyard);
    assert!(
        !engine.effective_has_keyword(first, Keyword::Trample),
        "the anthem ends when its source leaves the battlefield"
    );
}

#[test]
fn issue_323_enter_the_enigma_makes_the_target_unblockable_this_turn() {
    let decks = Some(vec![
        deck_with("island", &["enter_the_enigma"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(323_060, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "enter_the_enigma");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "enter_the_enigma");
    engine
        .apply_command(0, &cast_spell(slot, target_object(attacker)))
        .expect("cast Enter the Enigma");
    resolve_entire_stack_two_player(&mut engine);
    assert!(
        zone_view_rules_annotation_labels(&mut engine, 0, attacker)
            .iter()
            .any(|label| label == "Can't be blocked"),
        "the resolved restriction is published"
    );

    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("attack with the enchanted creature");
    pass_to_declare_blockers(&mut engine);
    let legal = &engine.initial_response_batch().legal_by_player[&1];
    assert!(
        !legal
            .legal_block_pairs
            .iter()
            .any(|pair| pair.attacker_id == attacker),
        "the unblockable creature has no legal blocker"
    );
    assert!(
        engine
            .apply_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: attacker,
                    blocker_id: blocker,
                }]),
            )
            .is_err(),
        "declaring a block on it is rejected"
    );

    // A separate game confirms the until-end-of-turn restriction expires at cleanup.
    let mut expiry = deck_engine(323_061, &["enter_the_enigma"], &["grizzly_bears"]);
    let target = inject_creature_on_battlefield(&mut expiry, 0, "grizzly_bears");
    ensure_in_hand(&mut expiry, 0, "enter_the_enigma");
    give_mana(
        &mut expiry,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&expiry, 0, "enter_the_enigma");
    expiry
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Enter the Enigma");
    resolve_entire_stack_two_player(&mut expiry);
    assert!(zone_view_rules_annotation_labels(&mut expiry, 0, target)
        .iter()
        .any(|label| label == "Can't be blocked"));
    end_active_turn(&mut expiry, 0);
    assert!(
        !zone_view_rules_annotation_labels(&mut expiry, 0, target)
            .iter()
            .any(|label| label == "Can't be blocked"),
        "the restriction expires at cleanup"
    );
}

#[test]
fn issue_323_professional_wrestler_treasure_and_single_blocker_limit() {
    let decks = Some(vec![
        deck_with("forest", &["professional_wrestler"]),
        deck_with("forest", &["grizzly_bears", "grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(323_070, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    let wrestler = move_ready_to_battlefield(&mut engine, 0, "professional_wrestler");
    engine.apply_command(0, &pass()).expect("controller passes");
    let completion = engine
        .apply_command(1, &pass())
        .expect("opponent passes into the ETB resolution");
    let treasure = battlefield_token_oids(&engine, 0, "treasure");
    assert_eq!(treasure.len(), 1, "the ETB creates one Treasure");
    assert!(token_created_events(&completion)
        .iter()
        .any(|token| token.card_id == "treasure"));

    let first = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    assert!(
        zone_view_rules_annotation_labels(&mut engine, 0, wrestler)
            .iter()
            .any(|label| label == "Can't be blocked by more than 1 creature"),
        "the single-blocker maximum is published"
    );

    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![wrestler]))
        .expect("attack with Professional Wrestler");
    pass_to_declare_blockers(&mut engine);
    let legal = &engine.initial_response_batch().legal_by_player[&1];
    assert!(
        legal
            .legal_block_pairs
            .iter()
            .any(|pair| pair.attacker_id == wrestler && pair.blocker_id == first),
        "a single blocker is legal"
    );

    let command_index = engine.state.command_index;
    assert!(
        engine
            .apply_command(
                1,
                &declare_blockers(vec![
                    BlockPair {
                        attacker_id: wrestler,
                        blocker_id: first,
                    },
                    BlockPair {
                        attacker_id: wrestler,
                        blocker_id: second,
                    },
                ]),
            )
            .is_err(),
        "two blockers exceed the maximum"
    );
    assert_eq!(engine.state.command_index, command_index);
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: wrestler,
                blocker_id: first,
            }]),
        )
        .expect("one blocker is accepted");
}
