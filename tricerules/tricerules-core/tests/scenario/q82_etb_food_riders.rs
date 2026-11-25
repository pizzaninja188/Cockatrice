//! Actual-card scenarios for the queue-v82 03-etb-food-riders cohort.
//!
//! The shared clause is an enters trigger that creates one Food token. Expectations are printed
//! Oracle values. CR 603.6a stages the trigger, CR 111.10a/701.7 creates the token, CR 702.17 and
//! 702.19 are the printed combat keywords, CR 702.29 governs Generous Ent's Forestcycling, and
//! CR 118.3/701.21a/119.3 govern Tempting Witch's sacrifice activation. The pinned rulings confirm
//! Food is an artifact subtype and one Food cannot pay two costs.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, AbilitySourceZone, ActivateAbility, RuledCommand, TargetRef,
};

struct Case {
    id: &'static str,
    mana: ManaGift,
    power: u32,
    toughness: u32,
    keyword: Option<Keyword>,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "fierce_witchstalker",
            mana: ManaGift {
                g: 2,
                c: 2,
                ..Default::default()
            },
            power: 4,
            toughness: 4,
            keyword: Some(Keyword::Trample),
        },
        Case {
            id: "generous_ent",
            mana: ManaGift {
                g: 1,
                c: 5,
                ..Default::default()
            },
            power: 5,
            toughness: 7,
            keyword: Some(Keyword::Reach),
        },
        Case {
            id: "tempting_witch",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            power: 1,
            toughness: 3,
            keyword: None,
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &["grizzly_bears"]),
        deck_with("island", &["grizzly_bears"]),
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

fn cast_and_food(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(e, 0, gift(&case.mana));
    inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
    assert_eq!(
        battlefield_token_oids(e, 0, "food").len(),
        1,
        "{}: exactly one Food",
        case.id
    );
    assert!(
        battlefield_token_oids(e, 1, "food").is_empty(),
        "{}: the opponent gets no Food",
        case.id
    );
    e.state
        .objects
        .iter()
        .find_map(|(id, object)| {
            (object.card_id == case.id && object.zone == Zone::Battlefield).then_some(*id)
        })
        .unwrap_or_else(|| panic!("{} did not stay on the battlefield", case.id))
}

fn zone_ability(
    engine: &GameEngine,
    source: u32,
    source_zone: AbilitySourceZone,
    ability_index: u32,
    targets: Vec<TargetRef>,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: source_zone as i32,
            expected_zone_change_generation: engine
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index,
            targets,
            ..Default::default()
        })),
    }
}

#[test]
fn q82_food_canary_simplest_fierce_witchstalker_creates_one_food() {
    let case = &cases()[0];
    let mut e = engine(82_030_001);
    let source = cast_and_food(&mut e, case);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "the printed cost is spent");
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(case.power), Some(case.toughness)),
        "printed body"
    );
    assert!(
        e.effective_has_keyword(source, Keyword::Trample),
        "printed trample"
    );
    assert_eq!(battlefield_token_oids(&e, 0, "food").len(), 1);
}

#[test]
fn q82_food_canary_hardest_generous_ent_cycles_for_a_forest() {
    let mut e = engine(82_031_001);
    let ent = inject_card_into_hand(&mut e, 0, "generous_ent");
    let forest = inject_library_card(&mut e, 0, "forest");
    let island = inject_library_card(&mut e, 0, "island");
    let opponent_library_before = e.state.players[1].library.clone();
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let command = zone_ability(&e, ent, AbilitySourceZone::Hand, 0, vec![]);
    e.apply_scenario_command(0, &command)
        .expect("activate Forestcycling {1}");
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "the printed {{1}} is spent");
    assert_eq!(
        e.state.objects[&ent].zone,
        Zone::Graveyard,
        "the discarded card is the typecycling card"
    );
    assert_eq!(e.state.stack.len(), 1);
    semantic::accepted(&mut e, 0, &pass());
    let search = semantic::accepted(&mut e, 1, &pass());
    let pending = e
        .state
        .pending_resolution
        .as_ref()
        .expect("private library search parks for the searching player");
    assert_eq!(
        pending.deciding_player, e.state.players[0].id,
        "only the activating player searches"
    );
    let choice = find_resolution_choice(&search).expect("private Forest search");
    assert!(
        choice
            .candidate_object_ids
            .iter()
            .all(|oid| e.state.players[0].library.contains(oid)),
        "the search candidates are from the activating player's library"
    );
    assert_eq!(
        e.state.players[1].library, opponent_library_before,
        "the opponent's library is unchanged"
    );
    assert!(
        choice.candidate_object_ids.contains(&forest),
        "the injected Forest is an eligible candidate"
    );
    assert!(
        !choice.candidate_object_ids.contains(&island),
        "the search is restricted to Forest cards"
    );
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![forest]));
    assert_eq!(e.state.objects[&forest].zone, Zone::Hand);
    assert!(e.state.players[0].hand.contains(&forest));
}

#[test]
fn q82_food_cards_create_one_food_and_match_printed_characteristics() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(82_032_000 + index as u64);
        let source = cast_and_food(&mut e, &case);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
            case.id
        );
        if let Some(keyword) = case.keyword {
            assert!(
                e.effective_has_keyword(source, keyword),
                "{}: printed keyword",
                case.id
            );
        }
    }
}

#[test]
fn q82_food_tempting_witch_sacrifices_food_for_three_life() {
    let mut e = engine(82_033_001);
    let witch = inject_permanent_on_battlefield(&mut e, 0, "tempting_witch");
    let food = inject_permanent_on_battlefield(&mut e, 0, "food");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let command = activate_ability_with_costs(
        witch,
        0,
        target_player(1),
        vec![permanent_cost_selection(2, food)],
    );
    semantic::accepted(&mut e, 0, &command);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact printed {{2}}");
    assert!(e.state.objects[&witch].tapped, "the {{T}} cost is paid");
    assert!(
        e.state
            .objects
            .get(&food)
            .is_none_or(|object| object.zone != Zone::Battlefield),
        "the chosen Food is sacrificed"
    );
    assert_eq!(e.state.stack.len(), 1);
    assert_eq!(
        e.state.players[1].life, 20,
        "life loss waits for resolution"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[1].life, 17,
        "the target player loses 3 life"
    );
}

#[test]
fn q82_food_tempting_witch_rejects_a_non_food_sacrifice() {
    let mut e = engine(82_034_001);
    let witch = inject_permanent_on_battlefield(&mut e, 0, "tempting_witch");
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let before = e.diagnostic_snapshot().expect("diagnostic snapshot");
    e.apply_scenario_command(
        0,
        &activate_ability_with_costs(
            witch,
            0,
            target_player(1),
            vec![permanent_cost_selection(2, bear)],
        ),
    )
    .expect_err("the sacrifice must be a Food");
    assert_eq!(
        e.diagnostic_snapshot().expect("diagnostic snapshot"),
        before,
        "the rejected activation rolls back mana and tap"
    );
}
