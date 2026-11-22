//! Actual-card scenarios for the reviewed Izzet Chronarch graveyard-return family.
//!
//! Expectations are printed Oracle values. CR 603.6a puts the entry trigger on the
//! stack; CR 406.3 and 400.7 identify the exact graveyard card; CR 608.2b fizzles a
//! trigger whose only target has left the graveyard. The saved Archaeomancer ruling
//! confirms that an instant or sorcery which put the creature onto the battlefield is
//! already in the graveyard and can be chosen.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChooseTriggerTarget, RuledCommand, TargetRef,
};

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "archaeomancer",
            mana: ManaGift {
                u: 2,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "ardent_elementalist",
            mana: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "izzet_chronarch",
            mana: ManaGift {
                u: 1,
                r: 1,
                c: 3,
                ..Default::default()
            },
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
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

fn cast_creature(e: &mut GameEngine, case: &Case) -> RuledEventBatch {
    give_mana(e, 0, gift(&case.mana));
    inject_card_into_hand(e, 0, case.id);
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    e.apply_scenario_command(0, &pass()).expect("p0 pass");
    e.apply_scenario_command(1, &pass())
        .expect("p1 pass resolves the creature and parks the entry trigger")
}

fn candidates(e: &GameEngine, batch: &RuledEventBatch) -> Vec<u32> {
    let pending = e
        .state
        .pending_triggers
        .front()
        .expect("entry trigger pending");
    let key = (pending.source_permanent_id as u64) << 32 | pending.ability_index as u64;
    let mut ids = batch.legal_by_player[&0].valid_targets_by_ability[&key].groups[0]
        .valid_graveyard_ids
        .clone();
    ids.sort_unstable();
    ids
}

fn choose_target(e: &mut GameEngine, target: u32) {
    e.apply_scenario_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                decline: false,
                selected_modes: Vec::new(),
                targets: vec![TargetRef {
                    counter_amount: 0,
                    expected_zone_change_generation: None,
                    object_id: target,
                    damage_amount: 0,
                    group_index: 0,
                    kind: 0,
                }],
            })),
        },
    )
    .expect("choose graveyard target");
}

#[test]
fn izzet_chronarch_batch_cards_return_an_instant_or_sorcery() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(850_000 + index as u64);
        let instant = inject_graveyard_card(&mut e, 0, "lightning_bolt");
        let sorcery = inject_graveyard_card(&mut e, 0, "divination");
        let artifact = inject_graveyard_card(&mut e, 0, "bonesplitter");
        let creature = inject_graveyard_card(&mut e, 0, "grizzly_bears");
        let opponent_instant = inject_graveyard_card(&mut e, 1, "counterspell");
        let batch = cast_creature(&mut e, &case);
        let mut expected = vec![instant, sorcery];
        expected.sort_unstable();
        assert_eq!(candidates(&e, &batch), expected, "{}", case.id);
        assert!(!candidates(&e, &batch).contains(&artifact), "{}", case.id);
        assert!(!candidates(&e, &batch).contains(&creature), "{}", case.id);
        assert!(
            !candidates(&e, &batch).contains(&opponent_instant),
            "{}: own graveyard only",
            case.id
        );

        choose_target(&mut e, instant);
        resolve_entire_stack_two_player(&mut e);
        assert!(
            e.state.players[0].hand.contains(&instant),
            "{}: returned to hand",
            case.id
        );
        assert!(
            !e.state.players[0].graveyard.contains(&instant),
            "{}",
            case.id
        );
    }
}

#[test]
fn izzet_chronarch_batch_cards_fizzle_when_the_target_leaves_the_graveyard() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(851_000 + index as u64);
        let immediate = inject_graveyard_card(&mut e, 0, "lightning_bolt");
        let sorcery = inject_graveyard_card(&mut e, 0, "divination");
        let batch = cast_creature(&mut e, &case);
        assert_eq!(candidates(&e, &batch).len(), 2, "{}", case.id);
        choose_target(&mut e, immediate);

        e.state.players[0]
            .graveyard
            .retain(|candidate| *candidate != immediate);
        e.state.players[0].exile.push(immediate);
        e.state.objects.get_mut(&immediate).expect("target").zone = Zone::Exile;
        *e.state.zone_change_generation.entry(immediate).or_default() += 1;

        resolve_entire_stack_two_player(&mut e);
        assert!(!e.state.players[0].hand.contains(&immediate), "{}", case.id);
        assert!(
            !e.state.players[0].graveyard.contains(&immediate),
            "{}",
            case.id
        );
        assert_eq!(e.state.objects[&immediate].zone, Zone::Exile, "{}", case.id);
        assert!(
            e.state.players[0].graveyard.contains(&sorcery),
            "{}: the other card is unaffected",
            case.id
        );
    }
}

#[test]
fn izzet_chronarch_batch_archaeomancer_can_return_the_spell_that_reanimated_it() {
    let mut e = engine(852_000);
    let archaeomancer = inject_graveyard_card(&mut e, 0, "archaeomancer");
    inject_card_into_hand(&mut e, 0, "zombify");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "zombify");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(archaeomancer)))
        .expect("cast zombify");
    e.apply_scenario_command(0, &pass()).expect("p0 pass");
    let batch = e
        .apply_scenario_command(1, &pass())
        .expect("zombify resolves and parks the entry trigger");

    assert!(
        e.state.players[0].battlefield.contains(&archaeomancer),
        "archaeomancer is on the battlefield"
    );
    assert_eq!(
        e.state.objects[&archaeomancer].zone,
        Zone::Battlefield,
        "archaeomancer zone"
    );
    let zombies = e.state.players[0]
        .graveyard
        .iter()
        .find(|oid| e.state.objects[oid].card_id == "zombify")
        .copied()
        .expect("Zombify is in the graveyard");
    assert!(
        candidates(&e, &batch).contains(&zombies),
        "the resolving reanimation spell is a legal target"
    );
    choose_target(&mut e, zombies);
    resolve_entire_stack_two_player(&mut e);
    assert!(
        e.state.players[0].hand.contains(&zombies),
        "Zombify returns to hand"
    );
}
