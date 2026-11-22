//! Actual-card scenarios for the queue-v19 artifact sacrifice-ping cohort.
//!
//! Expectations are printed Oracle values. CR 602.1a/602.2 activate the ability and
//! pay its costs; CR 701.21a moves the sacrificed artifact to its owner's graveyard as
//! a cost; CR 115.1 chooses the target when the ability is activated; CR 608.2b
//! fizzles the ability if the target has left.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    activation: ManaGift,
    amount: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "disruptor_pistol",
            mana: ManaGift {
                c: 2,
                ..Default::default()
            },
            activation: ManaGift {
                c: 4,
                ..Default::default()
            },
            amount: 5,
        },
        Case {
            id: "flamecast_wheel",
            mana: ManaGift {
                c: 1,
                ..Default::default()
            },
            activation: ManaGift {
                c: 5,
                ..Default::default()
            },
            amount: 3,
        },
        Case {
            id: "scalding_cauldron",
            mana: ManaGift {
                c: 1,
                ..Default::default()
            },
            activation: ManaGift {
                c: 3,
                ..Default::default()
            },
            amount: 3,
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &[]),
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
    let slot = inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cast cost", case.id);
    resolve_entire_stack_two_player(e);
    slot
}

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("object").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

#[test]
fn artifact_sac_ping_batch_cards_cast_with_exact_characteristics() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_030_000 + index as u64);
        let artifact = cast_artifact(&mut e, &case);
        assert_eq!(
            e.state.objects[&artifact].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.objects[&artifact].controller, e.state.players[0].id,
            "{}: controller",
            case.id
        );
        let characteristics = e.characteristics(artifact).expect("characteristics");
        assert!(characteristics.has_type("Artifact"), "{}", case.id);
        assert!(!characteristics.is_creature(), "{}: no body", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn artifact_sac_ping_batch_cards_damage_the_target_creature_exactly() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_031_000 + index as u64);
        let artifact = cast_artifact(&mut e, &case);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 6, 6);
        give_mana(&mut e, 0, gift(&case.activation));
        apply_ability(&mut e, 0, artifact, 0, target_object(target))
            .unwrap_or_else(|error| panic!("{}: activate: {error:?}", case.id));
        pass_both_players(&mut e);
        assert_eq!(
            pool(&e),
            (0, 0, 0, 0, 0, 0),
            "{}: exact activation cost",
            case.id
        );
        assert_eq!(
            e.state.objects[&target].damage, case.amount,
            "{}: printed damage",
            case.id
        );
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Battlefield,
            "{}: a 6/6 survives the printed damage",
            case.id
        );
    }
}

#[test]
fn artifact_sac_ping_batch_cards_sacrifice_itself_as_a_cost() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_032_000 + index as u64);
        let artifact = cast_artifact(&mut e, &case);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        give_mana(&mut e, 0, gift(&case.activation));
        apply_ability(&mut e, 0, artifact, 0, target_object(target))
            .unwrap_or_else(|error| panic!("{}: activate: {error:?}", case.id));
        assert_eq!(
            e.state.objects[&artifact].zone,
            Zone::Graveyard,
            "{}: the artifact is sacrificed when the ability is activated",
            case.id
        );
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{}: lethal damage destroys the 2/2",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn artifact_sac_ping_batch_cards_reject_activation_when_the_cost_cannot_be_paid() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_033_000 + index as u64);
        let artifact = cast_artifact(&mut e, &case);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let before = pool(&e);
        assert!(
            apply_ability(&mut e, 0, artifact, 0, target_object(target)).is_err(),
            "{}: the activation needs its exact mana",
            case.id
        );
        assert_eq!(pool(&e), before, "{}: no mana is spent", case.id);
        assert_eq!(
            e.state.objects[&artifact].zone,
            Zone::Battlefield,
            "{}: the artifact stays",
            case.id
        );
        assert_eq!(
            e.state.objects[&target].damage, 0,
            "{}: no damage is dealt",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}: no stack object", case.id);
    }
}

#[test]
fn artifact_sac_ping_batch_cards_stale_target_fizzles_after_the_creature_leaves() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_034_000 + index as u64);
        let artifact = cast_artifact(&mut e, &case);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let bystander = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        give_mana(&mut e, 0, gift(&case.activation));
        apply_ability(&mut e, 0, artifact, 0, target_object(target))
            .unwrap_or_else(|error| panic!("{}: activate: {error:?}", case.id));
        assert_eq!(
            e.state.objects[&artifact].zone,
            Zone::Graveyard,
            "{}: the cost is paid even if the target later leaves",
            case.id
        );
        move_to_graveyard(&mut e, 0, target);
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&bystander].damage, 0,
            "{}: the fizzled ability does not damage another creature",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
