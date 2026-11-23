//! Actual-card scenarios for the queue-v57 batch A cohorts.
//!
//! Expectations are printed Oracle values. CR 301.5/702.6 govern Equipment and equip;
//! CR 603.6a puts the entry trigger on the stack; CR 701.3 attaches; CR 611.3b and 613.4c
//! make the attached modifier and the "until end of turn" grant last as printed; CR 704.5n
//! detaches an Equipment whose host left; and CR 608.2b removes a trigger whose only
//! target is illegal.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::ruled_event::Ev;
use tricerules_proto::ruled::v1::{
    ChooseTriggerTarget, ResolutionChoiceDecision, RuledCommand, RuledEventBatch,
    SubmitResolutionChoice,
};

struct EquipmentCase {
    id: &'static str,
    mana: ManaGift,
    equip: ManaGift,
    power: u32,
    toughness: u32,
    static_keywords: &'static [Keyword],
    grants_first_strike: bool,
    untaps: bool,
}

fn equipment_cases() -> Vec<EquipmentCase> {
    vec![
        EquipmentCase {
            id: "galadhrim_bow",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            equip: ManaGift {
                c: 2,
                ..Default::default()
            },
            power: 1,
            toughness: 2,
            static_keywords: &[Keyword::Reach],
            grants_first_strike: false,
            untaps: true,
        },
        EquipmentCase {
            id: "hidden_footblade",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            equip: ManaGift {
                c: 2,
                ..Default::default()
            },
            power: 1,
            toughness: 0,
            static_keywords: &[Keyword::Haste],
            grants_first_strike: true,
            untaps: false,
        },
        EquipmentCase {
            id: "javelin_of_lightning",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            equip: ManaGift {
                c: 4,
                ..Default::default()
            },
            power: 2,
            toughness: 0,
            static_keywords: &[Keyword::FirstStrike],
            grants_first_strike: false,
            untaps: false,
        },
        EquipmentCase {
            id: "quick-draw_dagger",
            mana: ManaGift {
                c: 3,
                ..Default::default()
            },
            equip: ManaGift {
                c: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
            static_keywords: &[],
            grants_first_strike: true,
            untaps: false,
        },
    ]
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

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: target_object(object_id),
        })),
    }
}

fn equipment_engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("island", &[])]);
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

fn cast_and_attach(e: &mut GameEngine, case: &EquipmentCase, creature: u32) -> u32 {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast the Equipment");
    pass_both_players(e);
    e.apply_scenario_command(0, &choose_trigger_target(creature))
        .expect("choose the entry attachment target");
    pass_both_players(e);
    battlefield_object_for_card(e, 0, case.id)
}

fn move_battlefield_object_to_graveyard(e: &mut GameEngine, player: usize, object_id: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object_id);
    e.state.players[player].graveyard.push(object_id);
    e.state.objects.get_mut(&object_id).expect("object").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object_id).or_default() += 1;
}

fn assert_static_keywords(e: &GameEngine, object: u32, case: &EquipmentCase, expected: bool) {
    for &keyword in case.static_keywords {
        assert_eq!(
            e.effective_has_keyword(object, keyword),
            expected,
            "{}: {keyword:?}",
            case.id
        );
    }
}

#[test]
fn q57_flash_equipment_canary_quick_draw_dagger_attaches_and_grants_first_strike() {
    let case = equipment_cases()
        .into_iter()
        .find(|case| case.id == "quick-draw_dagger")
        .expect("quick-draw dagger case");
    let mut e = equipment_engine(57_100_000, case.id);
    let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let equipment = cast_and_attach(&mut e, &case, creature);

    assert_eq!(
        e.state.objects[&equipment].attached_to,
        Some(AttachmentRecipient::Object(creature)),
        "the entry trigger attaches to the chosen creature"
    );
    assert_eq!(e.effective_power(creature), Some(3), "+1/+1");
    assert_eq!(e.effective_toughness(creature), Some(3), "+1/+1");
    assert!(
        e.effective_has_keyword(creature, Keyword::FirstStrike),
        "the entry trigger grants first strike until end of turn"
    );
    assert_eq!(e.state.objects[&equipment].zone, Zone::Battlefield);
}

#[test]
fn q57_flash_equipment_canary_javelin_of_lightning_only_buffs_during_its_controllers_turn() {
    let case = equipment_cases()
        .into_iter()
        .find(|case| case.id == "javelin_of_lightning")
        .expect("javelin case");
    let mut e = equipment_engine(57_100_001, case.id);
    let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    cast_and_attach(&mut e, &case, creature);

    assert_eq!(e.effective_power(creature), Some(4), "your turn +2/+0");
    assert!(
        e.effective_has_keyword(creature, Keyword::FirstStrike),
        "your turn first strike"
    );

    end_active_turn(&mut e, 0);
    assert_eq!(e.state.active_player_id(), 1, "opponent's turn");
    assert_eq!(
        e.effective_power(creature),
        Some(2),
        "the bonus is off during the opponent's turn"
    );
    assert!(
        !e.effective_has_keyword(creature, Keyword::FirstStrike),
        "the granted first strike is off during the opponent's turn"
    );

    let equipment = battlefield_object_for_card(&e, 0, case.id);
    assert_eq!(
        e.state.objects[&equipment].attached_to,
        Some(AttachmentRecipient::Object(creature)),
        "the Equipment stays attached across the turn"
    );
}

#[test]
fn q57_flash_equipment_cards_attach_grant_and_move_the_printed_bonus() {
    for (index, case) in equipment_cases().into_iter().enumerate() {
        let mut e = equipment_engine(57_110_000 + index as u64, case.id);
        let first = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let equipment = cast_and_attach(&mut e, &case, first);
        assert_eq!(
            e.state.objects[&equipment].attached_to,
            Some(AttachmentRecipient::Object(first)),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_power(first),
            Some(2 + case.power),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(first),
            Some(2 + case.toughness),
            "{}",
            case.id
        );
        assert_static_keywords(&e, first, &case, true);
        if case.grants_first_strike {
            assert!(
                e.effective_has_keyword(first, Keyword::FirstStrike),
                "{}: entry first strike",
                case.id
            );
        }

        let second = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        give_mana(&mut e, 0, gift(&case.equip));
        apply_ability(&mut e, 0, equipment, 0, target_object(second))
            .expect("activate equip for the second creature");
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&equipment].attached_to,
            Some(AttachmentRecipient::Object(second)),
            "{}",
            case.id
        );
        assert_eq!(e.effective_power(first), Some(2), "{}", case.id);
        assert_eq!(e.effective_toughness(first), Some(2), "{}", case.id);
        assert_static_keywords(&e, first, &case, false);
        assert_eq!(
            e.effective_power(second),
            Some(2 + case.power),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(second),
            Some(2 + case.toughness),
            "{}",
            case.id
        );
        assert_static_keywords(&e, second, &case, true);
        if case.grants_first_strike {
            assert!(
                e.effective_has_keyword(first, Keyword::FirstStrike),
                "{}: the entry grant lasts until end of turn even after the Equipment moves",
                case.id
            );
        }
    }
}

#[test]
fn q57_flash_equipment_cards_untap_the_chosen_creature() {
    for (index, case) in equipment_cases().into_iter().enumerate() {
        if !case.untaps {
            continue;
        }
        let mut e = equipment_engine(57_120_000 + index as u64, case.id);
        let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let bystander = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .expect("cast the Equipment");
        pass_both_players(&mut e);
        e.apply_scenario_command(0, &choose_trigger_target(creature))
            .expect("choose the entry attachment target");
        e.state.objects.get_mut(&creature).expect("creature").tapped = true;
        e.state
            .objects
            .get_mut(&bystander)
            .expect("bystander")
            .tapped = true;
        pass_both_players(&mut e);
        assert!(
            !e.state.objects[&creature].tapped,
            "{}: the chosen creature is untapped",
            case.id
        );
        assert!(
            e.state.objects[&bystander].tapped,
            "{}: only the chosen creature is untapped",
            case.id
        );
    }
}

#[test]
fn q57_flash_equipment_cards_reject_an_opponents_creature_as_an_equip_target() {
    for (index, case) in equipment_cases().into_iter().enumerate() {
        let mut e = equipment_engine(57_130_000 + index as u64, case.id);
        let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let equipment = cast_and_attach(&mut e, &case, own);
        let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        give_mana(&mut e, 0, gift(&case.equip));
        let before = &e.state.players[0].mana_pool;
        let before_pool = (
            before.white,
            before.blue,
            before.black,
            before.red,
            before.green,
            before.colorless,
        );
        assert!(
            apply_ability(&mut e, 0, equipment, 0, target_object(opponent)).is_err(),
            "{}: an opponent's creature is not a legal equip target",
            case.id
        );
        assert_eq!(
            e.state.objects[&equipment].attached_to,
            Some(AttachmentRecipient::Object(own)),
            "{}",
            case.id
        );
        let after = &e.state.players[0].mana_pool;
        assert_eq!(
            (
                after.white,
                after.blue,
                after.black,
                after.red,
                after.green,
                after.colorless,
            ),
            before_pool,
            "{}: the rejected activation spends no mana",
            case.id
        );
    }
}

#[test]
fn q57_flash_equipment_cards_fizzle_the_entry_attach_when_the_target_leaves() {
    for (index, case) in equipment_cases().into_iter().enumerate() {
        let mut e = equipment_engine(57_140_000 + index as u64, case.id);
        let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let bystander = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .expect("cast the Equipment");
        pass_both_players(&mut e);
        e.apply_scenario_command(0, &choose_trigger_target(target))
            .expect("choose the entry attachment target");
        e.state
            .objects
            .get_mut(&bystander)
            .expect("bystander")
            .tapped = true;
        move_battlefield_object_to_graveyard(&mut e, 0, target);
        pass_both_players(&mut e);
        let equipment = battlefield_object_for_card(&e, 0, case.id);
        assert_eq!(
            e.state.objects[&equipment].attached_to, None,
            "{}: a stale entry target leaves the Equipment unattached",
            case.id
        );
        assert_eq!(
            e.state.objects[&equipment].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        if case.untaps {
            assert!(
                e.state.objects[&bystander].tapped,
                "{}: a removed trigger untaps no creature",
                case.id
            );
        }
    }
}

#[test]
fn q57_flash_equipment_cards_remove_the_entry_trigger_with_no_legal_creature() {
    for (index, case) in equipment_cases().into_iter().enumerate() {
        let mut e = equipment_engine(57_150_000 + index as u64, case.id);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .expect("cast the Equipment");
        pass_both_players(&mut e);
        let equipment = battlefield_object_for_card(&e, 0, case.id);
        assert!(
            e.state.pending_triggers.is_empty(),
            "{}: no entry trigger remains without a legal creature",
            case.id
        );
        assert_eq!(e.state.objects[&equipment].attached_to, None, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q57_flash_equipment_cards_flash_on_an_opponents_turn() {
    for (index, case) in equipment_cases().into_iter().enumerate() {
        let mut e = equipment_engine(57_160_000 + index as u64, case.id);
        let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        end_active_turn(&mut e, 0);
        assert_eq!(e.state.active_player_id(), 1, "{}", case.id);
        assert_eq!(
            e.state.turn_step,
            tricerules_core::TurnStep::Upkeep,
            "{}",
            case.id
        );
        e.apply_scenario_command(1, &pass())
            .expect("opponent passes in upkeep");
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .expect("cast the flash Equipment on the opponent's turn");
        pass_both_players(&mut e);
        e.apply_scenario_command(0, &choose_trigger_target(own))
            .expect("choose the entry attachment target");
        pass_both_players(&mut e);
        let equipment = battlefield_object_for_card(&e, 0, case.id);
        assert_eq!(
            e.state.objects[&equipment].attached_to,
            Some(AttachmentRecipient::Object(own)),
            "{}",
            case.id
        );
    }
}

// ---------------------------------------------------------------------------
// 02-basic-land-search
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum SearchKind {
    Hand,
    BattlefieldTapped,
    Split,
    EtbBattlefield,
    EtbTop,
    EtbTopReach,
}

struct SearchCase {
    id: &'static str,
    mana: ManaGift,
    kind: SearchKind,
    power: Option<u32>,
    toughness: Option<u32>,
}

fn search_cases() -> Vec<SearchCase> {
    let green = |c: u32| ManaGift {
        g: 1,
        c,
        ..Default::default()
    };
    vec![
        SearchCase {
            id: "giant_ladybug",
            mana: green(2),
            kind: SearchKind::EtbTopReach,
            power: Some(4),
            toughness: Some(1),
        },
        SearchCase {
            id: "kodamas_reach",
            mana: green(2),
            kind: SearchKind::Split,
            power: None,
            toughness: None,
        },
        SearchCase {
            id: "lay_of_the_land",
            mana: green(0),
            kind: SearchKind::Hand,
            power: None,
            toughness: None,
        },
        SearchCase {
            id: "loam_larva",
            mana: green(1),
            kind: SearchKind::EtbTop,
            power: Some(1),
            toughness: Some(3),
        },
        SearchCase {
            id: "natural_connection",
            mana: green(2),
            kind: SearchKind::BattlefieldTapped,
            power: None,
            toughness: None,
        },
        SearchCase {
            id: "quirion_trailblazer",
            mana: green(3),
            kind: SearchKind::EtbBattlefield,
            power: Some(1),
            toughness: Some(2),
        },
    ]
}

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn shuffle_log_count(batch: &RuledEventBatch, player: i32) -> usize {
    batch
        .events
        .iter()
        .filter(|event| {
            matches!(
                &event.ev,
                Some(Ev::Log(log)) if log.text == format!("P{player} shuffles their library.")
            )
        })
        .count()
}

fn search_engine(seed: u64, case: &SearchCase) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &[case.id]),
        deck_with("island", &[]),
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

fn cast_search_card(e: &mut GameEngine, case: &SearchCase) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast or deploy the search card");
    pass_both_players(e);
    if case.kind == SearchKind::EtbBattlefield
        || case.kind == SearchKind::EtbTop
        || case.kind == SearchKind::EtbTopReach
    {
        pass_both_players(e);
    }
}

fn pending_search_candidates(e: &GameEngine) -> Vec<u32> {
    e.state
        .pending_resolution
        .as_ref()
        .expect("private library search")
        .presentation
        .candidates
        .clone()
}

fn accept_etb_search(e: &mut GameEngine, case: &SearchCase) {
    let branch = e
        .state
        .pending_resolution
        .as_ref()
        .expect("optional search branch");
    assert!(
        branch.presentation.candidates.is_empty(),
        "{}: the branch prompt leaks no library card",
        case.id
    );
    assert_eq!(
        branch.deciding_player, 0,
        "{}: the controller chooses",
        case.id
    );
    e.apply_scenario_command(0, &select_branch(0))
        .expect("choose to search");
}

#[test]
fn q57_basic_land_search_canary_lay_of_the_land_puts_a_basic_land_into_hand() {
    let case = search_cases()
        .into_iter()
        .find(|case| case.id == "lay_of_the_land")
        .expect("lay of the land case");
    let mut e = search_engine(57_200_000, &case);
    let nonbasic = inject_library_card(&mut e, 0, "grizzly_bears");
    let basic = inject_library_card(&mut e, 0, "forest");
    let opponent_card = inject_library_card(&mut e, 1, "island");
    cast_search_card(&mut e, &case);

    let candidates = pending_search_candidates(&e);
    assert!(candidates.contains(&basic), "the basic land is offered");
    assert!(!candidates.contains(&nonbasic), "a nonland is not offered");
    assert!(
        !candidates.contains(&opponent_card),
        "another player's library is not searched"
    );
    let pending = e.state.pending_resolution.as_ref().expect("search prompt");
    assert_eq!(pending.deciding_player, 0, "the caster chooses");
    let completion = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![basic]))
        .expect("choose the basic land");
    assert_eq!(e.state.objects[&basic].zone, Zone::Hand);
    assert_eq!(shuffle_log_count(&completion, 0), 1, "one shuffle");
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn q57_basic_land_search_canary_kodamas_reach_splits_two_ordered_basics() {
    let case = search_cases()
        .into_iter()
        .find(|case| case.id == "kodamas_reach")
        .expect("kodama's reach case");
    let mut e = search_engine(57_200_001, &case);
    let forest = inject_library_card(&mut e, 0, "forest");
    let island = inject_library_card(&mut e, 0, "island");
    let taiga = inject_library_card(&mut e, 0, "taiga");
    cast_search_card(&mut e, &case);

    let choice = e.state.pending_resolution.as_ref().expect("search prompt");
    assert_eq!((choice.presentation.min, choice.presentation.max), (0, 2));
    let candidates = pending_search_candidates(&e);
    assert!(candidates.contains(&forest) && candidates.contains(&island));
    assert!(!candidates.contains(&taiga), "Taiga is not basic");
    let completion = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![forest, island]))
        .expect("choose two basics in order");
    assert_eq!(e.state.objects[&forest].zone, Zone::Battlefield);
    assert!(e.state.objects[&forest].tapped, "the first enters tapped");
    assert_eq!(e.state.objects[&island].zone, Zone::Hand);
    assert_eq!(e.state.objects[&island].controller, 0);
    assert_eq!(shuffle_log_count(&completion, 0), 1);
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn q57_basic_land_search_canary_kodamas_reach_handles_one_or_zero_basics() {
    let case = search_cases()
        .into_iter()
        .find(|case| case.id == "kodamas_reach")
        .expect("kodama's reach case");
    let mut one = search_engine(57_200_002, &case);
    let only = inject_library_card(&mut one, 0, "forest");
    cast_search_card(&mut one, &case);
    let completion = one
        .apply_scenario_command(0, &submit_resolution_choice(vec![only]))
        .expect("choose the only basic");
    assert_eq!(one.state.objects[&only].zone, Zone::Battlefield);
    assert!(one.state.objects[&only].tapped);
    assert_eq!(shuffle_log_count(&completion, 0), 1);

    let mut none = search_engine(57_200_003, &case);
    let nonbasic = inject_library_card(&mut none, 0, "grizzly_bears");
    cast_search_card(&mut none, &case);
    let completion = none
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("find nothing");
    assert_eq!(none.state.objects[&nonbasic].zone, Zone::Library);
    assert_eq!(shuffle_log_count(&completion, 0), 1, "still shuffles once");
}

#[test]
fn q57_basic_land_search_spell_cases_use_their_printed_destination() {
    for (index, case) in search_cases().into_iter().enumerate() {
        if matches!(
            case.kind,
            SearchKind::EtbBattlefield | SearchKind::EtbTop | SearchKind::EtbTopReach
        ) {
            continue;
        }
        let mut e = search_engine(57_210_000 + index as u64, &case);
        let land = inject_library_card(&mut e, 0, "forest");
        cast_search_card(&mut e, &case);
        let completion = e
            .apply_scenario_command(0, &submit_resolution_choice(vec![land]))
            .expect("choose the basic land");
        match case.kind {
            SearchKind::Hand => assert_eq!(e.state.objects[&land].zone, Zone::Hand, "{}", case.id),
            SearchKind::BattlefieldTapped => {
                assert_eq!(
                    e.state.objects[&land].zone,
                    Zone::Battlefield,
                    "{}",
                    case.id
                );
                assert!(e.state.objects[&land].tapped, "{}", case.id);
                assert_eq!(e.state.objects[&land].controller, 0, "{}", case.id);
            }
            _ => {}
        }
        assert_eq!(shuffle_log_count(&completion, 0), 1, "{}", case.id);
    }
}

#[test]
fn q57_basic_land_search_entry_cases_may_decline_without_shuffling() {
    for (index, case) in search_cases().into_iter().enumerate() {
        if !matches!(
            case.kind,
            SearchKind::EtbBattlefield | SearchKind::EtbTop | SearchKind::EtbTopReach
        ) {
            continue;
        }
        let mut e = search_engine(57_220_000 + index as u64, &case);
        let land = inject_library_card(&mut e, 0, "forest");
        cast_search_card(&mut e, &case);
        let library_before = e.state.players[0].library.len();
        let batch = e
            .apply_scenario_command(
                0,
                &submit_resolution_decision(ResolutionChoiceDecision::Decline),
            )
            .expect("decline the search");
        assert_eq!(shuffle_log_count(&batch, 0), 0, "{}", case.id);
        assert_eq!(
            e.state.players[0].library.len(),
            library_before,
            "{}",
            case.id
        );
        assert_eq!(e.state.objects[&land].zone, Zone::Library, "{}", case.id);
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
    }
}

#[test]
fn q57_basic_land_search_entry_cases_use_their_printed_destination() {
    for (index, case) in search_cases().into_iter().enumerate() {
        if !matches!(
            case.kind,
            SearchKind::EtbBattlefield | SearchKind::EtbTop | SearchKind::EtbTopReach
        ) {
            continue;
        }
        let mut e = search_engine(57_230_000 + index as u64, &case);
        let land = inject_library_card(&mut e, 0, "forest");
        cast_search_card(&mut e, &case);
        accept_etb_search(&mut e, &case);
        let candidates = pending_search_candidates(&e);
        assert!(candidates.contains(&land), "{}", case.id);
        let completion = e
            .apply_scenario_command(0, &submit_resolution_choice(vec![land]))
            .expect("choose the basic land");
        match case.kind {
            SearchKind::EtbBattlefield => {
                assert_eq!(
                    e.state.objects[&land].zone,
                    Zone::Battlefield,
                    "{}",
                    case.id
                );
                assert!(e.state.objects[&land].tapped, "{}", case.id);
                assert_eq!(e.state.objects[&land].controller, 0, "{}", case.id);
            }
            SearchKind::EtbTop | SearchKind::EtbTopReach => {
                assert_eq!(e.state.objects[&land].zone, Zone::Library, "{}", case.id);
                assert_eq!(
                    e.state.players[0].library.front().copied(),
                    Some(land),
                    "{}: the found land is on top",
                    case.id
                );
            }
            _ => {}
        }
        assert_eq!(shuffle_log_count(&completion, 0), 1, "{}", case.id);
        if case.kind == SearchKind::EtbTopReach {
            let ladybug = battlefield_object_for_card(&e, 0, case.id);
            assert!(
                e.effective_has_keyword(ladybug, Keyword::Reach),
                "Giant Ladybug has reach"
            );
            assert_eq!(e.effective_power(ladybug), case.power);
            assert_eq!(e.effective_toughness(ladybug), case.toughness);
        }
    }
}

// ---------------------------------------------------------------------------
// 03-static-anthems
// ---------------------------------------------------------------------------

fn pass_once(e: &mut GameEngine) {
    let actor = e.state.priority_player_id();
    e.apply_scenario_command(actor, &pass())
        .expect("pass priority toward the requested step");
}

fn pass_round(e: &mut GameEngine) {
    pass_once(e);
    pass_once(e);
}

fn advance_to_step(e: &mut GameEngine, wanted: tricerules_core::TurnStep) {
    for _ in 0..40 {
        if e.state.turn_step == wanted {
            return;
        }
        pass_once(e);
    }
    panic!("game did not reach {wanted:?}");
}

fn enter_declare_attackers(e: &mut GameEngine, active_player: i32) {
    e.apply_scenario_command(active_player, &primitive_yield())
        .expect("yield from main phase to beginning of combat");
    advance_to_step(e, tricerules_core::TurnStep::DeclareAttackers);
}

fn advance_to_next_turn(e: &mut GameEngine) {
    let starting_turn = e.state.turn;
    for _ in 0..80 {
        if e.state.turn > starting_turn {
            return;
        }
        if let Some(player) = e.state.cleanup_discard_player {
            let player_index = e.state.player_idx(player).expect("cleanup player");
            let excess = e.state.players[player_index].hand.len() - 7;
            e.apply_scenario_command(player, &discard_cleanup_batch((0..excess as u32).collect()))
                .expect("discard down to hand size during cleanup");
        } else {
            pass_once(e);
        }
    }
    panic!("game did not advance to the next turn");
}

fn advance_to_player_main1(e: &mut GameEngine, player: i32) {
    for _ in 0..80 {
        if e.state.turn_step == tricerules_core::TurnStep::Main1
            && e.state.active_player_id() == player
        {
            return;
        }
        if let Some(player) = e.state.cleanup_discard_player {
            let player_index = e.state.player_idx(player).expect("cleanup player");
            let excess = e.state.players[player_index].hand.len() - 7;
            e.apply_scenario_command(player, &discard_cleanup_batch((0..excess as u32).collect()))
                .expect("discard down to hand size during cleanup");
        } else {
            pass_once(e);
        }
    }
    panic!("game did not reach player {player}'s main phase");
}

fn cast_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("cast engine");
    advance_to_main1_from_game_start(&mut e);
    e
}

fn cast_anthem(e: &mut GameEngine, id: &str, mana: ManaGift) -> u32 {
    let object = inject_card_into_hand(e, 0, id);
    give_mana(e, 0, mana);
    let slot = hand_index_for_card(e, 0, id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast the anthem");
    resolve_entire_stack_two_player(e);
    assert_eq!(e.state.objects[&object].zone, Zone::Battlefield, "{id}");
    object
}

fn move_permanent_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("source").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

fn attacking_pump_cases() -> [(&'static str, ManaGift); 2] {
    [
        (
            "orcish_oriflamme",
            ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
        ),
        (
            "war_horn",
            ManaGift {
                c: 3,
                ..Default::default()
            },
        ),
    ]
}

#[test]
fn q57_static_anthems_canary_war_horn_pumps_only_attacking_creatures_you_control() {
    let mut e = cast_engine(57_300_000);
    cast_anthem(
        &mut e,
        "war_horn",
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let bystander = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
    let opposing_creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    assert_eq!(e.effective_power(attacker), Some(2), "not attacking yet");
    enter_declare_attackers(&mut e, 0);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare one attacker");
    assert_eq!(e.effective_power(attacker), Some(3), "attacking +1/+0");
    assert_eq!(
        e.effective_toughness(attacker),
        Some(2),
        "no toughness bonus"
    );
    assert_eq!(
        e.effective_power(bystander),
        Some(2),
        "nonattacking is unaffected"
    );
    assert_eq!(
        e.effective_power(opposing_creature),
        Some(2),
        "the opponent's creature is unaffected"
    );

    pass_round(&mut e);
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers
    );
    e.apply_scenario_command(1, &declare_blockers(vec![]))
        .expect("declare no blockers");
    pass_round(&mut e);
    pass_round(&mut e);
    advance_to_step(&mut e, tricerules_core::TurnStep::Main2);
    assert_eq!(
        e.effective_power(attacker),
        Some(2),
        "the bonus ends when the creature is no longer attacking"
    );
}

#[test]
fn q57_static_anthems_canary_blade_historian_grants_double_strike_only_while_attacking() {
    let mut e = cast_engine(57_300_001);
    cast_anthem(
        &mut e,
        "blade_historian",
        ManaGift {
            w: 4,
            ..Default::default()
        },
    );
    let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let bystander = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
    let opposing_attacker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    assert!(!e.effective_has_keyword(attacker, Keyword::DoubleStrike));
    enter_declare_attackers(&mut e, 0);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare one attacker");

    assert!(
        e.effective_has_keyword(attacker, Keyword::DoubleStrike),
        "your attacking creature gains double strike"
    );
    assert!(
        !e.effective_has_keyword(bystander, Keyword::DoubleStrike),
        "your nonattacking creature does not gain double strike"
    );
    assert!(
        !e.effective_has_keyword(opposing_attacker, Keyword::DoubleStrike),
        "the opponent's creature does not gain double strike"
    );

    pass_round(&mut e);
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers
    );
    e.apply_scenario_command(1, &declare_blockers(vec![]))
        .expect("declare no blockers");
    pass_round(&mut e);
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::FirstStrikeDamage
    );
    assert_eq!(e.state.players[1].life, 18);
    assert!(
        e.effective_has_keyword(attacker, Keyword::DoubleStrike),
        "the attacker remains double striking after first-strike damage"
    );
    pass_round(&mut e);
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::CombatDamage);
    assert_eq!(
        e.state.players[1].life, 16,
        "the 2/2 attacker deals damage in both combat-damage steps"
    );

    advance_to_step(&mut e, tricerules_core::TurnStep::Main2);
    assert!(
        !e.effective_has_keyword(attacker, Keyword::DoubleStrike),
        "the continuous grant ends when the creature is no longer attacking"
    );

    advance_to_next_turn(&mut e);
    advance_to_player_main1(&mut e, 1);
    enter_declare_attackers(&mut e, 1);
    e.apply_scenario_command(1, &declare_attackers(vec![opposing_attacker]))
        .expect("the opponent declares an attacker");
    assert!(
        !e.effective_has_keyword(opposing_attacker, Keyword::DoubleStrike),
        "the static ability follows its controller rather than the active player"
    );
}

#[test]
fn q57_static_anthems_attacking_pump_covers_each_attacking_card() {
    for (index, (id, mana)) in attacking_pump_cases().into_iter().enumerate() {
        let mut e = cast_engine(57_310_000 + index as u64);
        cast_anthem(&mut e, id, mana);
        let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        enter_declare_attackers(&mut e, 0);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare one attacker");
        assert_eq!(e.effective_power(attacker), Some(3), "{id}: +1/+0");
        assert_eq!(e.effective_toughness(attacker), Some(2), "{id}: no +0/+1");
    }
}

#[test]
fn q57_static_anthems_attacking_filter_follows_its_controller() {
    for (index, (id, mana)) in attacking_pump_cases().into_iter().enumerate() {
        let mut e = cast_engine(57_320_000 + index as u64);
        cast_anthem(&mut e, id, mana);
        let opposing_attacker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        end_active_turn(&mut e, 0);
        assert_eq!(e.state.active_player_id(), 1, "{id}: opponent's turn");
        advance_to_player_main1(&mut e, 1);
        enter_declare_attackers(&mut e, 1);
        e.apply_scenario_command(1, &declare_attackers(vec![opposing_attacker]))
            .expect("the opponent declares an attacker");
        assert_eq!(
            e.effective_power(opposing_attacker),
            Some(2),
            "{id}: the anthem follows its controller, not the attacking player"
        );
    }
}

#[test]
fn q57_static_anthems_nonattacking_creatures_are_not_pumped() {
    for (index, (id, mana)) in attacking_pump_cases().into_iter().enumerate() {
        let mut e = cast_engine(57_330_000 + index as u64);
        cast_anthem(&mut e, id, mana);
        let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let opposing = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        assert_eq!(e.effective_power(own), Some(2), "{id}: not attacking");
        assert_eq!(
            e.effective_power(opposing),
            Some(2),
            "{id}: opponent unaffected"
        );
        assert_eq!(
            e.effective_toughness(opposing),
            Some(2),
            "{id}: opponent unaffected"
        );
    }
}

#[test]
fn q57_static_anthems_cards_apply_to_later_entries_and_stop_when_the_source_leaves() {
    for (index, id) in ["favorable_winds", "primal_rage"].into_iter().enumerate() {
        let mut e = anthem_engine(57_340_000 + index as u64, id);
        let source = move_ready_to_battlefield(&mut e, 0, id);
        let first = inject_creature_on_battlefield(&mut e, 0, "wind_drake");
        let grounded = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let opposing_flyer = inject_creature_on_battlefield(&mut e, 1, "wind_drake");
        let later = inject_creature_on_battlefield(&mut e, 0, "wind_drake");

        match id {
            "favorable_winds" => {
                assert_eq!(
                    (e.effective_power(first), e.effective_toughness(first)),
                    (Some(3), Some(3)),
                    "a controlled flyer gets +1/+1"
                );
                assert_eq!(
                    (e.effective_power(grounded), e.effective_toughness(grounded)),
                    (Some(2), Some(2)),
                    "a grounded creature is not pumped"
                );
                assert_eq!(
                    (
                        e.effective_power(opposing_flyer),
                        e.effective_toughness(opposing_flyer)
                    ),
                    (Some(2), Some(2)),
                    "an opponent's flyer is not pumped"
                );
                assert_eq!(
                    (e.effective_power(later), e.effective_toughness(later)),
                    (Some(3), Some(3)),
                    "a later flyer is pumped"
                );
            }
            "primal_rage" => {
                assert!(
                    e.effective_has_keyword(first, Keyword::Trample),
                    "trample is granted"
                );
                assert!(
                    e.effective_has_keyword(later, Keyword::Trample),
                    "trample is granted to later entries"
                );
                assert!(
                    !e.effective_has_keyword(opposing_flyer, Keyword::Trample),
                    "the opponent does not gain trample"
                );
            }
            _ => unreachable!(),
        }

        move_permanent_to_graveyard(&mut e, 0, source);
        if id == "favorable_winds" {
            assert_eq!(
                (e.effective_power(first), e.effective_toughness(first)),
                (Some(2), Some(2)),
                "the flyer bonus ends with the source"
            );
            assert_eq!(
                (e.effective_power(later), e.effective_toughness(later)),
                (Some(2), Some(2)),
                "the later flyer bonus ends with the source"
            );
        } else {
            assert!(
                !e.effective_has_keyword(first, Keyword::Trample),
                "the trample grant ends with the source"
            );
        }
    }
}

#[test]
fn q57_static_anthems_blade_historian_grants_itself_double_strike() {
    let mut e = anthem_engine(57_350_000, "blade_historian");
    let historian = move_ready_to_battlefield(&mut e, 0, "blade_historian");
    enter_declare_attackers(&mut e, 0);
    e.apply_scenario_command(0, &declare_attackers(vec![historian]))
        .expect("Blade Historian attacks");
    assert!(
        e.effective_has_keyword(historian, Keyword::DoubleStrike),
        "Blade Historian's own attack is covered by its grant"
    );
}

#[test]
fn q57_static_anthems_blade_historian_ruling_prevents_regular_damage_after_it_leaves() {
    let mut e = anthem_engine(57_350_001, "blade_historian");
    let historian = move_ready_to_battlefield(&mut e, 0, "blade_historian");
    let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    enter_declare_attackers(&mut e, 0);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare one attacker");
    pass_round(&mut e);
    e.apply_scenario_command(1, &declare_blockers(vec![]))
        .expect("declare no blockers");
    pass_round(&mut e);
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::FirstStrikeDamage
    );
    assert_eq!(
        e.state.players[1].life, 18,
        "the double striker deals first-strike damage"
    );
    move_permanent_to_graveyard(&mut e, 0, historian);
    pass_round(&mut e);
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::CombatDamage);
    assert_eq!(
        e.state.players[1].life, 18,
        "the attacker lost double strike after already dealing first-strike damage"
    );
}
