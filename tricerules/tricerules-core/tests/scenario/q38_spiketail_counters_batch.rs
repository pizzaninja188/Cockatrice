//! Actual-card scenarios for the queue-v38 Spiketail soft-counter cohort.
//!
//! Expectations are printed Oracle values and the pinned 2004-10-04 rulings. CR 701.6 counters
//! the spell; CR 118.12/118.12a give the targeted spell's controller the payment choice when the
//! ability resolves; CR 602.1/118.3 make the sacrifice a cost paid at activation.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ResolutionChoiceDecision, TargetRef};

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
    payment: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "spiketail_drake",
            power: 3,
            toughness: 3,
            payment: 3,
        },
        Case {
            id: "spiketail_drakeling",
            power: 2,
            toughness: 2,
            payment: 2,
        },
        Case {
            id: "spiketail_hatchling",
            power: 1,
            toughness: 1,
            payment: 1,
        },
    ]
}

fn spell_target(object_id: u32) -> TargetRef {
    TargetRef {
        counter_amount: 0,
        expected_zone_change_generation: None,
        object_id,
        damage_amount: 0,
        group_index: 0,
        kind: 0,
    }
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &["grizzly_bears"]),
        deck_with("forest", &[]),
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

fn cast_bears(e: &mut GameEngine) -> u32 {
    inject_card_into_hand(e, 0, "grizzly_bears");
    give_mana(
        e,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, 0, "grizzly_bears");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Grizzly Bears");
    e.state.stack.last().expect("Bears on the stack").id
}

fn activate_counter(e: &mut GameEngine, case: &Case) -> (u32, u32) {
    let source = inject_creature_with_stats(e, 1, case.id, case.power, case.toughness);
    assert!(e.effective_has_keyword(source, tricerules_cards::Keyword::Flying));
    let spell = cast_bears(e);
    e.apply_scenario_command(0, &pass())
        .expect("active player passes priority after casting");
    apply_ability(e, 1, source, 0, vec![spell_target(spell)]).expect("activate Spiketail");
    assert_eq!(
        e.state.objects[&source].zone,
        Zone::Graveyard,
        "the sacrifice is a cost paid at activation"
    );
    (source, spell)
}

fn resolve_to_payment(e: &mut GameEngine) -> tricerules_proto::ruled::v1::ResolutionChoiceRequired {
    e.apply_scenario_command(1, &pass())
        .expect("ability controller passes");
    let batch = e
        .apply_scenario_command(0, &pass())
        .expect("spell controller resolves the ability");
    find_resolution_choice(&batch).expect("payment choice")
}

#[test]
fn q38_spiketail_counters_canary_simplest() {
    let case = &cases()[2];
    let mut e = engine(38_060_001);
    let (source, spell) = activate_counter(&mut e, case);
    let choice = resolve_to_payment(&mut e);
    assert_eq!(choice.generic_mana_cost, 1);
    assert_eq!(
        choice.deciding_player_id, 0,
        "the spell's controller chooses"
    );
    e.apply_scenario_command(
        0,
        &submit_resolution_decision(ResolutionChoiceDecision::Decline),
    )
    .expect("decline the payment");
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    assert!(e.state.stack.is_empty());
}

#[test]
fn q38_spiketail_counters_canary_hardest() {
    let case = &cases()[0];
    let mut e = engine(38_060_002);
    let (source, spell) = activate_counter(&mut e, case);
    let choice = resolve_to_payment(&mut e);
    assert_eq!(choice.generic_mana_cost, 3);
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    submit_mana_resolution_decision(&mut e, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay the exact generic amount");
    assert_eq!(e.state.objects[&spell].zone, Zone::Stack);
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&spell].zone, Zone::Battlefield);
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
}

#[test]
fn q38_spiketail_cards_offer_their_printed_payment_to_the_spells_controller() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(38_061_000 + index as u64);
        let (_, spell) = activate_counter(&mut e, case);
        let choice = resolve_to_payment(&mut e);
        assert_eq!(choice.generic_mana_cost, case.payment, "{}", case.id);
        assert_eq!(choice.deciding_player_id, 0, "{}", case.id);
        e.apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline");
        assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard, "{}", case.id);
    }
}

#[test]
fn q38_spiketail_cards_require_a_spell_target_and_cannot_activate_twice() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(38_062_000 + index as u64);
        let source = inject_creature_with_stats(&mut e, 1, case.id, case.power, case.toughness);
        apply_ability(&mut e, 1, source, 0, vec![]).expect_err("a spell target is required");
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );

        let spell = cast_bears(&mut e);
        e.apply_scenario_command(0, &pass())
            .expect("active player passes priority after casting");
        apply_ability(&mut e, 1, source, 0, vec![spell_target(spell)]).expect("first activation");
        apply_ability(&mut e, 1, source, 0, vec![spell_target(spell)])
            .expect_err("the source was sacrificed");
    }
}
