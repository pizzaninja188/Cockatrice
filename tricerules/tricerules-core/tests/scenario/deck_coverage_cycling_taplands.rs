//! Exact deck-corpus coverage for six tapped cycling lands.
//!
//! Expectations come from the pinned Oracle text and the current Comprehensive Rules: CR 614.1c
//! (enters-tapped replacement), CR 605.1a/605.3b (mana abilities resolve immediately), and
//! CR 702.29a (hand-only cycling, with discard as a cost and a draw on resolution).

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::AbilitySourceZone;

type ManaOption = (u32, u32, u32, u32, u32, u32);

struct CyclingTapland {
    card: &'static str,
    cycling_cost: ManaOption,
    mana_options: &'static [ManaOption],
}

const LANDS: [CyclingTapland; 6] = [
    CyclingTapland {
        card: "tranquil_thicket",
        cycling_cost: (0, 0, 0, 0, 1, 0),
        mana_options: &[(0, 0, 0, 0, 1, 0)],
    },
    CyclingTapland {
        card: "forgotten_cave",
        cycling_cost: (0, 0, 0, 1, 0, 0),
        mana_options: &[(0, 0, 0, 1, 0, 0)],
    },
    CyclingTapland {
        card: "fetid_pools",
        cycling_cost: (0, 0, 0, 0, 0, 2),
        mana_options: &[(0, 1, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0)],
    },
    CyclingTapland {
        card: "scattered_groves",
        cycling_cost: (0, 0, 0, 0, 0, 2),
        mana_options: &[(0, 0, 0, 0, 1, 0), (1, 0, 0, 0, 0, 0)],
    },
    CyclingTapland {
        card: "irrigated_farmland",
        cycling_cost: (0, 0, 0, 0, 0, 2),
        mana_options: &[(1, 0, 0, 0, 0, 0), (0, 1, 0, 0, 0, 0)],
    },
    CyclingTapland {
        card: "sheltered_thicket",
        cycling_cost: (0, 0, 0, 0, 0, 2),
        mana_options: &[(0, 0, 0, 1, 0, 0), (0, 0, 0, 0, 1, 0)],
    },
];

fn land_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("mountain", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn hand_cycling(engine: &GameEngine, source: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: semantic::generation(engine, source),
            ability_index: 1,
            ..Default::default()
        })),
    }
}

fn mana_option(engine: &GameEngine, source: u32, option: u32) -> RuledCommand {
    let mut command = activate_ability_for(engine, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!()
    };
    ability.mana_option_index = option;
    command
}

fn mana_pool(engine: &GameEngine) -> ManaOption {
    let pool = &engine.state.players[0].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
}

fn advance_to_controller_next_main1(engine: &mut GameEngine) {
    end_active_turn(engine, 0);
    advance_to_main1_from_game_start(engine);
    assert_eq!(engine.state.active_player_id(), 1);
    end_active_turn(engine, 1);
    advance_to_main1_from_game_start(engine);
    assert_eq!(engine.state.active_player_id(), 0);
}

#[test]
fn deck_coverage_cycling_taplands_enter_tapped_and_produce_each_printed_color() {
    for (land_index, land) in LANDS.iter().enumerate() {
        for (option_index, expected) in land.mana_options.iter().enumerate() {
            let mut engine = land_engine(20_260_926 + (land_index * 2 + option_index) as u64);
            let source = inject_card_into_hand(&mut engine, 0, land.card);
            let slot = hand_index_for_card(&engine, 0, land.card);
            semantic::accepted(&mut engine, 0, &play_land(slot));

            assert!(
                engine.state.objects[&source].tapped,
                "{} enters tapped",
                land.card
            );
            let before = engine.state.command_index;
            engine
                .apply_command(0, &mana_option(&engine, source, option_index as u32))
                .expect_err("a land entering tapped cannot pay its mana ability's tap cost");
            assert_eq!(engine.state.command_index, before);
            assert!(engine.state.objects[&source].tapped);

            advance_to_controller_next_main1(&mut engine);
            assert!(
                !engine.state.objects[&source].tapped,
                "{} untaps",
                land.card
            );
            let command = mana_option(&engine, source, option_index as u32);
            semantic::accepted(&mut engine, 0, &command);
            assert_eq!(mana_pool(&engine), *expected, "{} mana option", land.card);
            assert!(engine.state.objects[&source].tapped);
            assert!(
                engine.state.stack.is_empty(),
                "mana ability resolves immediately"
            );
        }
    }
}

#[test]
fn deck_coverage_cycling_taplands_pay_the_printed_cost_discard_and_draw() {
    for (index, land) in LANDS.iter().enumerate() {
        let mut engine = land_engine(20_260_940 + index as u64);
        let source = inject_card_into_hand(&mut engine, 0, land.card);
        let drawn = inject_library_card(&mut engine, 0, "grizzly_bears");
        engine.state.players[0].library.retain(|oid| *oid != drawn);
        engine.state.players[0].library.push_front(drawn);

        let available = engine.initial_response_batch();
        assert!(available.legal_by_player[&0]
            .zone_ability_actions
            .iter()
            .any(|action| action.object_id == source
                && action.source_zone() == AbilitySourceZone::Hand));
        assert!(!available.legal_by_player[&1]
            .zone_ability_actions
            .iter()
            .any(|action| action.object_id == source));

        let command = hand_cycling(&engine, source);
        let command_index = engine.state.command_index;
        engine
            .apply_command(0, &command)
            .expect_err("Cycling requires its printed mana cost");
        assert_eq!(engine.state.command_index, command_index);
        assert_eq!(engine.state.objects[&source].zone, Zone::Hand);
        assert_eq!(engine.state.objects[&drawn].zone, Zone::Library);

        give_mana(
            &mut engine,
            0,
            ManaGift {
                w: land.cycling_cost.0,
                u: land.cycling_cost.1,
                b: land.cycling_cost.2,
                r: land.cycling_cost.3,
                g: land.cycling_cost.4,
                c: land.cycling_cost.5,
            },
        );
        let opponent_hand = engine.state.players[1].hand.clone();
        semantic::accepted(&mut engine, 0, &command);
        assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
        assert_eq!(engine.state.objects[&drawn].zone, Zone::Library);
        assert_eq!(mana_pool(&engine), (0, 0, 0, 0, 0, 0));

        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(engine.state.objects[&drawn].zone, Zone::Hand);
        assert_eq!(engine.state.players[1].hand, opponent_hand);
        engine
            .apply_command(0, &command)
            .expect_err("the spent Cycling card is no longer in hand");
    }
}
