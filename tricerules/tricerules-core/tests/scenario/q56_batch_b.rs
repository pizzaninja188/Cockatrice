//! Actual-card scenarios for the queue-v56 batch B cohorts.
//!
//! Expectations are printed Oracle values. CR 115.1a/601.2c choose targets as the spell is
//! cast; CR 611.2a/613.4c apply until-end-of-turn shrink; CR 614.6/616.1 give the turn-scoped
//! exile replacement; CR 701.34 proliferates; CR 301.5/702.6 govern Equipment, attach and equip;
//! CR 603.6a puts the entry trigger on the stack; CR 702.34a-d govern flashback; CR 608.2b
//! fizzles a spell whose only target is illegal.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{
    CastMethod, CastSpell, ChoiceKind, RuledCommand, SubmitResolutionChoice,
};

struct ShrinkCase {
    id: &'static str,
    mana: ManaGift,
    penalty: u32,
    proliferates: bool,
}

fn shrink_cases() -> [ShrinkCase; 4] {
    [
        ShrinkCase {
            id: "bleed_dry",
            mana: ManaGift {
                b: 2,
                c: 2,
                ..Default::default()
            },
            penalty: 13,
            proliferates: false,
        },
        ShrinkCase {
            id: "ob_nixiliss_cruelty",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            penalty: 5,
            proliferates: false,
        },
        ShrinkCase {
            id: "drown_in_ichor",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            penalty: 4,
            proliferates: true,
        },
        ShrinkCase {
            id: "whisper_of_the_dross",
            mana: ManaGift {
                b: 1,
                ..Default::default()
            },
            penalty: 1,
            proliferates: true,
        },
    ]
}

struct EquipCase {
    id: &'static str,
    mana: ManaGift,
    equip: ManaGift,
    power: u32,
    toughness: u32,
    flying: bool,
}

fn equip_cases() -> [EquipCase; 4] {
    [
        EquipCase {
            id: "bramble_armor",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            equip: ManaGift {
                c: 4,
                ..Default::default()
            },
            power: 2,
            toughness: 1,
            flying: false,
        },
        EquipCase {
            id: "utility_knife",
            mana: ManaGift {
                c: 1,
                ..Default::default()
            },
            equip: ManaGift {
                c: 3,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
            flying: false,
        },
        EquipCase {
            id: "cliffhaven_kitesail",
            mana: ManaGift {
                c: 1,
                ..Default::default()
            },
            equip: ManaGift {
                c: 2,
                ..Default::default()
            },
            power: 0,
            toughness: 0,
            flying: true,
        },
        EquipCase {
            id: "mechanical_glider",
            mana: ManaGift {
                c: 1,
                ..Default::default()
            },
            equip: ManaGift {
                c: 2,
                ..Default::default()
            },
            power: 0,
            toughness: 0,
            flying: true,
        },
    ]
}

struct TokenCase {
    id: &'static str,
    mana: ManaGift,
    flashback: ManaGift,
    token: &'static str,
    count: usize,
    power: u32,
    toughness: u32,
    flying: bool,
}

fn token_cases() -> [TokenCase; 3] {
    [
        TokenCase {
            id: "call_of_the_herd",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            flashback: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            token: "elephant_g_3_3",
            count: 1,
            power: 3,
            toughness: 3,
            flying: false,
        },
        TokenCase {
            id: "elephant_ambush",
            mana: ManaGift {
                g: 2,
                c: 2,
                ..Default::default()
            },
            flashback: ManaGift {
                g: 2,
                c: 6,
                ..Default::default()
            },
            token: "elephant_g_3_3",
            count: 1,
            power: 3,
            toughness: 3,
            flying: false,
        },
        TokenCase {
            id: "lingering_souls",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            flashback: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            token: "spirit_w_1_1_flying",
            count: 2,
            power: 1,
            toughness: 1,
            flying: true,
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

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("forest", &[])]);
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

fn pool(e: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn choose_proliferate(objects: Vec<u32>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            chosen_object_ids: objects,
            decision: tricerules_proto::ruled::v1::ResolutionChoiceDecision::Unspecified as i32,
            ..Default::default()
        })),
    }
}

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("object").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

// ---------------------------------------------------------------------------
// 04-shrink-rider

fn cast_shrink(e: &mut GameEngine, case: &ShrinkCase, target: u32) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    assert_eq!(pool(e, 0), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
}

fn resolve_shrink(e: &mut GameEngine, case: &ShrinkCase, proliferate_target: Option<u32>) {
    pass_both_players(e);
    if case.proliferates {
        if let Some(pending) = e.state.pending_resolution.as_ref() {
            assert_eq!(
                pending.presentation.choice_kind,
                ChoiceKind::Proliferate,
                "{}",
                case.id
            );
            let objects = proliferate_target.into_iter().collect();
            e.apply_scenario_command(0, &choose_proliferate(objects))
                .unwrap_or_else(|error| panic!("{}: proliferate: {error:?}", case.id));
        } else {
            assert!(
                proliferate_target.is_none(),
                "{}: a candidate counter must park the Proliferate choice",
                case.id
            );
        }
    }
    assert!(e.state.stack.is_empty(), "{}", case.id);
}

#[test]
fn q56_b_canary_simplest_whisper_of_the_dross_shrinks_then_proliferates() {
    let case = &shrink_cases()[3];
    let mut e = engine(56_400_001);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    e.state
        .objects
        .get_mut(&target)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    cast_shrink(&mut e, case, target);
    resolve_shrink(&mut e, case, Some(target));
    assert_eq!(
        e.state.objects[&target].counter_count(CounterKind::PlusOnePlusOne),
        2,
        "Proliferate adds another counter of each kind already there"
    );
    assert_eq!(e.effective_power(target), Some(3));
    assert_eq!(e.effective_toughness(target), Some(3));
}

#[test]
fn q56_b_drown_in_ichor_proliferates_a_chosen_counter_bearing_permanent() {
    let case = &shrink_cases()[2];
    let mut e = engine(56_400_003);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 6, 6);
    let chosen = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let unchosen = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.state
        .objects
        .get_mut(&chosen)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    e.state
        .objects
        .get_mut(&unchosen)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    cast_shrink(&mut e, case, target);
    resolve_shrink(&mut e, case, Some(chosen));
    assert_eq!(
        e.state.objects[&chosen].counter_count(CounterKind::PlusOnePlusOne),
        2,
        "the chosen counter-bearing permanent gets one more of each kind"
    );
    assert_eq!(
        e.state.objects[&unchosen].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "an unchosen permanent is unchanged"
    );
    assert_eq!(e.effective_toughness(target), Some(2));
}

#[test]
fn q56_b_canary_hardest_bleed_dry_exiles_the_lethally_shrunk_creature() {
    let case = &shrink_cases()[0];
    let mut e = engine(56_400_002);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 13, 13);
    cast_shrink(&mut e, case, target);
    resolve_shrink(&mut e, case, None);
    assert_eq!(e.state.objects[&target].zone, Zone::Exile);
    assert_eq!(e.state.turn_history.current.creatures_died, 0);
}

#[test]
fn q56_b_shrink_riders_apply_their_printed_values() {
    for (index, case) in shrink_cases().iter().enumerate() {
        let mut e = engine(56_401_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 20, 20);
        cast_shrink(&mut e, case, target);
        resolve_shrink(&mut e, case, None);
        assert_eq!(
            e.effective_power(target),
            Some(20 - case.penalty),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(target),
            Some(20 - case.penalty),
            "{}",
            case.id
        );
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert_eq!(e.effective_power(target), Some(20), "{}", case.id);
        assert_eq!(e.effective_toughness(target), Some(20), "{}", case.id);
    }
}

#[test]
fn q56_b_shrink_riders_exile_a_later_death_that_turn() {
    for case in shrink_cases()
        .iter()
        .filter(|case| matches!(case.id, "ob_nixiliss_cruelty" | "bleed_dry"))
    {
        let mut e = engine(56_402_000);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 14, 14);
        cast_shrink(&mut e, case, target);
        resolve_shrink(&mut e, case, None);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );

        inject_card_into_hand(&mut e, 0, "village_rites");
        give_mana(
            &mut e,
            0,
            ManaGift {
                b: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, "village_rites");
        e.apply_scenario_command(
            0,
            &cast_spell_with_costs(slot, vec![], vec![permanent_cost_selection(0, target)]),
        )
        .unwrap_or_else(|error| panic!("{}: sacrifice: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Exile,
            "{}: the exile replacement lasts the turn",
            case.id
        );
    }
}

#[test]
fn q56_b_shrink_riders_stop_exiling_deaths_after_the_turn() {
    for case in shrink_cases()
        .iter()
        .filter(|case| matches!(case.id, "ob_nixiliss_cruelty" | "bleed_dry"))
    {
        let mut e = engine(56_402_500);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 14, 14);
        cast_shrink(&mut e, case, target);
        resolve_shrink(&mut e, case, None);
        end_active_turn(&mut e, 0);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        end_active_turn(&mut e, 1);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        resolve_cleanup_discards_if_any(&mut e);
        for _ in 0..4 {
            if e.state.turn_step == tricerules_core::TurnStep::Main1 {
                break;
            }
            pass_priority_round(&mut e);
            resolve_cleanup_discards_if_any(&mut e);
        }
        assert_eq!(
            e.state.turn_step,
            tricerules_core::TurnStep::Main1,
            "{}: back in the caster's main phase",
            case.id
        );

        inject_card_into_hand(&mut e, 0, "village_rites");
        give_mana(
            &mut e,
            0,
            ManaGift {
                b: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, "village_rites");
        e.apply_scenario_command(
            0,
            &cast_spell_with_costs(slot, vec![], vec![permanent_cost_selection(0, target)]),
        )
        .unwrap_or_else(|error| panic!("{}: sacrifice: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{}: the exile replacement expired with the turn",
            case.id
        );
        assert_eq!(
            e.state.turn_history.current.creatures_died, 1,
            "{}: the creature died normally",
            case.id
        );
    }
}

#[test]
fn q56_b_shrink_riders_reject_a_player_target() {
    for (index, case) in shrink_cases().iter().enumerate() {
        let mut e = engine(56_403_000 + index as u64);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        let before = pool(&e, 0);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
                .is_err(),
            "{}: a player is not a legal creature target",
            case.id
        );
        assert_eq!(pool(&e, 0), before, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

// ---------------------------------------------------------------------------
// 05-equipment-etb-attach

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(
            tricerules_proto::ruled::v1::ChooseTriggerTarget {
                decline: false,
                selected_modes: Vec::new(),
                targets: target_object(object_id),
            },
        )),
    }
}

fn cast_and_attach(e: &mut GameEngine, case: &EquipCase, creature: u32) -> u32 {
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

#[test]
fn q56_b_canary_simplest_utility_knife_attaches_and_moves_its_bonus() {
    let case = &equip_cases()[1];
    let mut e = engine(56_500_001);
    let first = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let equipment = cast_and_attach(&mut e, case, first);
    assert_eq!(
        e.state.objects[&equipment].attached_to,
        Some(AttachmentRecipient::Object(first))
    );
    assert_eq!(e.effective_power(first), Some(3));
    assert_eq!(e.effective_toughness(first), Some(3));

    let second = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    give_mana(&mut e, 0, gift(&case.equip));
    apply_ability(&mut e, 0, equipment, 0, target_object(second))
        .expect("equip the second creature");
    pass_both_players(&mut e);
    assert_eq!(
        e.state.objects[&equipment].attached_to,
        Some(AttachmentRecipient::Object(second))
    );
    assert_eq!(e.effective_power(first), Some(2));
    assert_eq!(e.effective_toughness(first), Some(2));
    assert_eq!(e.effective_power(second), Some(3));
}

#[test]
fn q56_b_canary_hardest_bramble_armor_moves_plus_two_plus_one() {
    let case = &equip_cases()[0];
    let mut e = engine(56_500_002);
    let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let equipment = cast_and_attach(&mut e, case, target);
    assert_eq!(e.effective_power(target), Some(4));
    assert_eq!(e.effective_toughness(target), Some(3));
    assert_eq!(
        e.state.objects[&equipment].attached_to,
        Some(AttachmentRecipient::Object(target))
    );
}

#[test]
fn q56_b_equipment_etb_cards_apply_the_printed_static_bonus() {
    for (index, case) in equip_cases().iter().enumerate() {
        let mut e = engine(56_501_000 + index as u64);
        let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        cast_and_attach(&mut e, case, target);
        assert_eq!(
            e.effective_power(target),
            Some(2 + case.power),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(target),
            Some(2 + case.toughness),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_has_keyword(target, Keyword::Flying),
            case.flying,
            "{}",
            case.id
        );
    }
}

#[test]
fn q56_b_equipment_etb_cards_reject_an_opponents_creature() {
    for (index, case) in equip_cases().iter().enumerate() {
        let mut e = engine(56_502_000 + index as u64);
        let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let equipment = cast_and_attach(&mut e, case, own);
        let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        give_mana(&mut e, 0, gift(&case.equip));
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
    }
}

#[test]
fn q56_b_equipment_etb_cards_remain_unattached_when_the_target_leaves() {
    for (index, case) in equip_cases().iter().enumerate() {
        let mut e = engine(56_503_000 + index as u64);
        let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .expect("cast the Equipment");
        pass_both_players(&mut e);
        e.apply_scenario_command(0, &choose_trigger_target(target))
            .expect("choose the entry attachment target");
        move_to_graveyard(&mut e, 0, target);
        pass_both_players(&mut e);
        let equipment = battlefield_object_for_card(&e, 0, case.id);
        assert_eq!(e.state.objects[&equipment].attached_to, None, "{}", case.id);
    }
}

// ---------------------------------------------------------------------------
// 06-flashback-tokens

fn flashback_cast(spell: u32, generation: u64) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Flashback as i32,
            source: Some(graveyard_cast_source(spell, generation)),
            ..Default::default()
        })),
    }
}

#[test]
fn q56_b_canary_simplest_call_of_the_herd_makes_elephants_and_flashes_back() {
    let case = &token_cases()[0];
    let mut e = engine(56_600_001);
    give_mana(&mut e, 0, gift(&case.mana));
    let spell = inject_card_into_hand(&mut e, 0, case.id);
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast from hand");
    resolve_entire_stack_two_player(&mut e);
    let first = battlefield_token_oids(&e, 0, case.token);
    assert_eq!(first.len(), case.count);
    assert_eq!(
        e.state.objects[&first[0]]
            .power
            .zip(e.state.objects[&first[0]].toughness),
        Some((case.power, case.toughness))
    );
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);

    let generation = e.state.zone_change_generation[&spell];
    give_mana(&mut e, 0, gift(&case.flashback));
    e.apply_scenario_command(0, &flashback_cast(spell, generation))
        .expect("cast with flashback");
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact flashback cost");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        battlefield_token_oids(&e, 0, case.token).len(),
        case.count * 2
    );
    assert_eq!(e.state.objects[&spell].zone, Zone::Exile);
}

#[test]
fn q56_b_canary_hardest_lingering_souls_makes_two_flying_spirits() {
    let case = &token_cases()[2];
    let mut e = engine(56_600_002);
    give_mana(&mut e, 0, gift(&case.mana));
    let spell = inject_card_into_hand(&mut e, 0, case.id);
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast from hand");
    resolve_entire_stack_two_player(&mut e);
    let spirits = battlefield_token_oids(&e, 0, case.token);
    assert_eq!(spirits.len(), 2);
    for spirit in &spirits {
        assert_eq!(
            e.state.objects[spirit]
                .power
                .zip(e.state.objects[spirit].toughness),
            Some((1, 1))
        );
        assert!(e.effective_has_keyword(*spirit, Keyword::Flying));
    }
    let generation = e.state.zone_change_generation[&spell];
    give_mana(&mut e, 0, gift(&case.flashback));
    e.apply_scenario_command(0, &flashback_cast(spell, generation))
        .expect("flashback");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(battlefield_token_oids(&e, 0, case.token).len(), 4);
    assert_eq!(e.state.objects[&spell].zone, Zone::Exile);
}

#[test]
fn q56_b_flashback_token_cards_make_the_printed_tokens_and_exile_after_flashback() {
    for (index, case) in token_cases().iter().enumerate() {
        let mut e = engine(56_601_000 + index as u64);
        give_mana(&mut e, 0, gift(&case.mana));
        let spell = inject_card_into_hand(&mut e, 0, case.id);
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        let tokens = battlefield_token_oids(&e, 0, case.token);
        assert_eq!(tokens.len(), case.count, "{}", case.id);
        assert_eq!(
            e.state.objects[&tokens[0]]
                .power
                .zip(e.state.objects[&tokens[0]].toughness),
            Some((case.power, case.toughness)),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_has_keyword(tokens[0], Keyword::Flying),
            case.flying,
            "{}",
            case.id
        );
        assert_eq!(
            battlefield_token_oids(&e, 1, case.token).len(),
            0,
            "{}",
            case.id
        );

        let generation = e.state.zone_change_generation[&spell];
        give_mana(&mut e, 0, gift(&case.flashback));
        e.apply_scenario_command(0, &flashback_cast(spell, generation))
            .unwrap_or_else(|error| panic!("{}: flashback: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            battlefield_token_oids(&e, 0, case.token).len(),
            case.count * 2,
            "{}",
            case.id
        );
        assert_eq!(e.state.objects[&spell].zone, Zone::Exile, "{}", case.id);
    }
}
