//! Actual-card scenarios for the queue-v83 01-artifact-death-draw cohort.
//!
//! Every card is an Implement: one activated ability paying a colored mana and sacrificing the
//! artifact, plus "When this artifact is put into a graveyard from the battlefield, draw a card."
//! Expectations are printed Oracle values. CR 602.2/701.21a pay the cost and sacrifice the source;
//! CR 603.6c/603.10a and the pinned 2017-02-09 ruling stage the leaves-the-battlefield draw above
//! the activation, so the controller draws before the first ability resolves; CR 115.1/608.2b
//! govern the optional target; CR 119.3, 120.3, 122.1 and 701.9 are the four other effects.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    activation: ManaGift,
    effect: Effect,
}

#[derive(Clone, Copy)]
enum Effect {
    DamagePlayer,
    DrawOne,
    CounterCreature,
    GainTwo,
    DiscardOne,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "implement_of_combustion",
            mana: ManaGift {
                c: 1,
                ..Default::default()
            },
            activation: ManaGift {
                r: 1,
                ..Default::default()
            },
            effect: Effect::DamagePlayer,
        },
        Case {
            id: "implement_of_examination",
            mana: ManaGift {
                c: 3,
                ..Default::default()
            },
            activation: ManaGift {
                u: 1,
                ..Default::default()
            },
            effect: Effect::DrawOne,
        },
        Case {
            id: "implement_of_ferocity",
            mana: ManaGift {
                c: 1,
                ..Default::default()
            },
            activation: ManaGift {
                g: 1,
                ..Default::default()
            },
            effect: Effect::CounterCreature,
        },
        Case {
            id: "implement_of_improvement",
            mana: ManaGift {
                c: 1,
                ..Default::default()
            },
            activation: ManaGift {
                w: 1,
                ..Default::default()
            },
            effect: Effect::GainTwo,
        },
        Case {
            id: "implement_of_malice",
            mana: ManaGift {
                c: 2,
                ..Default::default()
            },
            activation: ManaGift {
                b: 1,
                ..Default::default()
            },
            effect: Effect::DiscardOne,
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", &["grizzly_bears", "shatter"]),
        deck_with("forest", &["grizzly_bears"]),
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

fn gift(m: &ManaGift) -> ManaGift {
    ManaGift {
        w: m.w,
        u: m.u,
        b: m.b,
        r: m.r,
        g: m.g,
        c: m.c,
    }
}

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn cast_artifact(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(e, 0, gift(&case.mana));
    inject_card_into_hand(e, 0, case.id);
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cast", case.id);
    battlefield_object_for_card(e, 0, case.id)
}

fn enter_declare_attackers(e: &mut GameEngine) {
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to beginning of combat");
    pass_both_players(e);
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );
}

#[test]
fn q83_implement_canary_simplest_improvement_gains_life_and_draws() {
    let case = &cases()[3];
    let mut e = engine(83_010_001);
    let artifact = cast_artifact(&mut e, case);
    let life = e.state.players[0].life;
    let hand = e.state.players[0].hand.len();
    give_mana(&mut e, 0, gift(&case.activation));
    apply_ability(&mut e, 0, artifact, 0, vec![]).expect("activate Improvement");
    assert_eq!(
        e.state.objects[&artifact].zone,
        Zone::Graveyard,
        "the sacrifice is a cost"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, life + 2, "printed life gain");
    assert_eq!(
        e.state.players[0].hand.len(),
        hand + 1,
        "the leaves-the-battlefield trigger draws one"
    );
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact activation cost");
}

#[test]
fn q83_implement_canary_hardest_malice_draws_before_the_discard() {
    let case = &cases()[4];
    let mut e = engine(83_011_001);
    let artifact = cast_artifact(&mut e, case);
    let cleared = std::mem::take(&mut e.state.players[1].hand);
    e.state.players[1].library.extend(cleared);
    let opponent_card = inject_card_into_hand(&mut e, 1, "grizzly_bears");
    let hand = e.state.players[0].hand.len();
    give_mana(&mut e, 0, gift(&case.activation));
    apply_ability(&mut e, 0, artifact, 0, target_player(1)).expect("activate Malice");
    // The death trigger is above the activation: resolve it first, then the discard stalls on the
    // opponent's private choice.
    pass_both_players(&mut e);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand + 1,
        "the death trigger draws before the discard ability resolves"
    );
    let batch = pass_both_players_with_batch(&mut e);
    let choice = find_resolution_choice(&batch).expect("opponent discards a card");
    assert!(choice.candidate_object_ids.contains(&opponent_card));
    e.apply_scenario_command(1, &submit_resolution_choice(vec![opponent_card]))
        .expect("discard the only card");
    assert_eq!(e.state.objects[&opponent_card].zone, Zone::Graveyard);
    assert_eq!(e.state.players[1].hand.len(), 0);
    assert_eq!(e.state.players[0].hand.len(), hand + 1);
    assert_eq!(e.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0));
}

#[test]
fn q83_implement_cards_apply_their_printed_effect() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(83_012_000 + index as u64);
        let artifact = cast_artifact(&mut e, &case);
        let life = e.state.players[0].life;
        let opponent_life = e.state.players[1].life;
        let hand = e.state.players[0].hand.len();
        let opponent_hand = e.state.players[1].hand.len();
        let own_creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let targets = match case.effect {
            Effect::DamagePlayer => target_player(1),
            Effect::CounterCreature => target_object(own_creature),
            Effect::DiscardOne => target_player(1),
            Effect::DrawOne | Effect::GainTwo => vec![],
        };
        give_mana(&mut e, 0, gift(&case.activation));
        apply_ability(&mut e, 0, artifact, 0, targets)
            .unwrap_or_else(|error| panic!("{}: activate: {error:?}", case.id));
        assert_eq!(
            e.state.objects[&artifact].zone,
            Zone::Graveyard,
            "{}: sacrificed as a cost",
            case.id
        );
        if matches!(case.effect, Effect::DiscardOne) {
            pass_both_players(&mut e);
            let batch = pass_both_players_with_batch(&mut e);
            let choice = find_resolution_choice(&batch).expect("discard choice");
            e.apply_scenario_command(
                1,
                &submit_resolution_choice(vec![choice.candidate_object_ids[0]]),
            )
            .expect("discard");
        } else {
            resolve_entire_stack_two_player(&mut e);
        }
        match case.effect {
            Effect::DamagePlayer => {
                assert_eq!(
                    e.state.players[1].life,
                    opponent_life - 1,
                    "{}: one damage to the player",
                    case.id
                )
            }
            Effect::DrawOne => assert_eq!(
                e.state.players[0].hand.len(),
                hand + 2,
                "{}: activation draw plus death draw",
                case.id
            ),
            Effect::CounterCreature => assert_eq!(
                e.state.objects[&own_creature]
                    .counters
                    .get(&CounterKind::PlusOnePlusOne)
                    .copied()
                    .unwrap_or(0),
                1,
                "{}: one +1/+1 counter",
                case.id
            ),
            Effect::GainTwo => {
                assert_eq!(e.state.players[0].life, life + 2, "{}: two life", case.id)
            }
            Effect::DiscardOne => assert_eq!(
                e.state.players[1].hand.len(),
                opponent_hand.saturating_sub(1),
                "{}: one discarded card",
                case.id
            ),
        }
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q83_implement_combustion_rejects_a_creature_target() {
    let case = &cases()[0];
    let mut e = engine(83_013_001);
    let artifact = cast_artifact(&mut e, case);
    let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(&mut e, 0, gift(&case.activation));
    let before = e.diagnostic_snapshot().expect("snapshot");
    assert!(
        apply_ability(&mut e, 0, artifact, 0, target_object(creature)).is_err(),
        "a creature is not a legal player-or-planeswalker target"
    );
    assert_eq!(e.diagnostic_snapshot().expect("snapshot"), before);
    assert_eq!(e.state.objects[&artifact].zone, Zone::Battlefield);
}

#[test]
fn q83_implement_combustion_stale_planeswalker_target_fizzles_but_death_draws() {
    let case = &cases()[0];
    let mut e = engine(83_014_001);
    let artifact = cast_artifact(&mut e, case);
    let writer = inject_permanent_on_battlefield(&mut e, 1, "jace_beleren");
    let hand = e.state.players[0].hand.len();
    give_mana(&mut e, 0, gift(&case.activation));
    apply_ability(&mut e, 0, artifact, 0, target_object(writer))
        .expect("activate at the planeswalker");
    // Remove the planeswalker before the activation resolves; the draw already triggered.
    e.state.players[1]
        .battlefield
        .retain(|object| *object != writer);
    e.state.players[1].graveyard.push(writer);
    e.state.objects.get_mut(&writer).expect("writer").zone = Zone::Graveyard;
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand + 1,
        "the leaves-the-battlefield trigger still draws"
    );
    assert_eq!(
        e.state.players[1].life, 20,
        "the stale target fizzles with no damage"
    );
}

#[test]
fn q83_implement_sorcery_speed_activations_reject_combat_timing() {
    for case in [&cases()[2], &cases()[4]] {
        let mut e = engine(83_015_000 + u64::from(case.effect as u8 == 2));
        let artifact = cast_artifact(&mut e, case);
        // Combat steps exist only when an attacker is available; inject one so the test reaches
        // DeclareAttackers instead of skipping the combat phase.
        inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        enter_declare_attackers(&mut e);
        give_mana(&mut e, 0, gift(&case.activation));
        let targets = if matches!(case.effect, Effect::DiscardOne) {
            target_player(1)
        } else {
            target_object(inject_creature_on_battlefield(&mut e, 0, "grizzly_bears"))
        };
        assert!(
            apply_ability(&mut e, 0, artifact, 0, targets).is_err(),
            "{}: 'Activate only as a sorcery' rejects combat timing",
            case.id
        );
        assert_eq!(
            e.state.objects[&artifact].zone,
            Zone::Battlefield,
            "{}: a rejected activation pays nothing",
            case.id
        );
    }
}

#[test]
fn q83_implement_dies_trigger_fires_when_destroyed_without_activation() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(83_016_000 + index as u64);
        let artifact = cast_artifact(&mut e, &case);
        ensure_card_in_hand(&mut e, 0, "shatter");
        let hand = e.state.players[0].hand.len();
        give_mana(
            &mut e,
            0,
            ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, "shatter");
        e.apply_scenario_command(0, &cast_spell(slot, target_object(artifact)))
            .expect("cast Shatter at the Implement");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&artifact].zone,
            Zone::Graveyard,
            "{}: the Implement is destroyed",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand,
            "{}: the destroyed Implement still draws one card",
            case.id
        );
    }
}

#[test]
fn q83_implement_unaffordable_activation_rejects_atomically() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(83_017_000 + index as u64);
        let artifact = cast_artifact(&mut e, &case);
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: no mana left", case.id);
        let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let hand = e.state.players[0].hand.len();
        let opponent_hand = e.state.players[1].hand.len();
        let targets = match case.effect {
            Effect::DamagePlayer | Effect::DiscardOne => target_player(1),
            Effect::CounterCreature => target_object(creature),
            Effect::DrawOne | Effect::GainTwo => vec![],
        };
        assert!(
            apply_ability(&mut e, 0, artifact, 0, targets).is_err(),
            "{}: an unpaid activation is rejected",
            case.id
        );
        assert_eq!(
            e.state.objects[&artifact].zone,
            Zone::Battlefield,
            "{}: the artifact is not sacrificed",
            case.id
        );
        assert_eq!(e.state.players[0].hand.len(), hand, "{}", case.id);
        assert_eq!(e.state.players[1].hand.len(), opponent_hand, "{}", case.id);
        assert!(
            e.state.stack.is_empty(),
            "{}: no trigger or ability",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}", case.id);
    }
}

#[test]
fn q83_implement_combustion_damages_a_planeswalker() {
    let case = &cases()[0];
    let mut e = engine(83_018_001);
    let artifact = cast_artifact(&mut e, case);
    let writer = inject_permanent_on_battlefield(&mut e, 1, "jace_beleren");
    e.state
        .objects
        .get_mut(&writer)
        .expect("writer")
        .set_counter(CounterKind::Loyalty, 3);
    let hand = e.state.players[0].hand.len();
    give_mana(&mut e, 0, gift(&case.activation));
    apply_ability(&mut e, 0, artifact, 0, target_object(writer))
        .expect("activate at the planeswalker");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&writer].counter_count(CounterKind::Loyalty),
        2,
        "one damage removes one loyalty counter"
    );
    assert_eq!(e.state.players[1].life, 20, "the player is not damaged");
    assert_eq!(
        e.state.players[0].hand.len(),
        hand + 1,
        "the death trigger draws one card"
    );
}
