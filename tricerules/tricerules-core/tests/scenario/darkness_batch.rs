//! Actual-card scenarios for the reviewed Darkness combat-damage-prevention family.
//!
//! Expectations are printed Oracle values. CR 614.1a and 615.1a replace combat damage
//! with zero for the turn; the effect applies only to combat damage and lasts until end
//! of turn (CR 611.2a/514.2). Noncombat damage is unaffected.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "darkness",
            mana: ManaGift {
                b: 1,
                ..Default::default()
            },
        },
        Case {
            id: "holy_day",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
        },
        Case {
            id: "root_snare",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        std::iter::repeat_n("grizzly_bears".to_string(), 10).collect::<Vec<_>>(),
        vec!["forest".into(); 10],
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
    advance_to_declare_attackers(&mut e);
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

fn cast_before_damage(e: &mut GameEngine, case: &Case) {
    let attacker = *e.state.players[0]
        .battlefield
        .last()
        .expect("attacker on battlefield");
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_scenario_command(0, &pass())
        .expect("p0 passes in declare attackers");
    inject_card_into_hand(e, 1, case.id);
    give_mana(e, 1, gift(&case.mana));
    let slot = hand_index_for_card(e, 1, case.id);
    e.apply_scenario_command(1, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    e.apply_scenario_command(1, &pass())
        .expect("p1 pass on stack");
    e.apply_scenario_command(0, &pass())
        .expect("p0 pass resolves the prevention");
}

#[test]
fn darkness_batch_cards_prevent_all_combat_damage_this_turn() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(840_000 + index as u64);
        cast_before_damage(&mut e, &case);
        e.apply_scenario_command(0, &pass())
            .expect("p0 passes after prevention");
        e.apply_scenario_command(1, &pass())
            .expect("p1 passes to declare blockers");
        e.apply_scenario_command(0, &pass())
            .expect("p0 passes in declare blockers");
        let batch = e
            .apply_scenario_command(1, &pass())
            .expect("combat damage resolves");
        assert!(
            batch.events.iter().all(|event| {
                !matches!(
                    &event.ev,
                    Some(tricerules_proto::ruled::v1::ruled_event::Ev::LifeChanged(change))
                        if change.delta < 0
                )
            }),
            "{}: no combat life loss",
            case.id
        );
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);
    }
}

#[test]
fn darkness_batch_cards_do_not_prevent_noncombat_damage() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(841_000 + index as u64);
        cast_before_damage(&mut e, &case);
        // The active player passes, the defending player gets priority and casts Shock.
        e.apply_scenario_command(0, &pass())
            .expect("p0 passes after prevention");
        inject_card_into_hand(&mut e, 1, "shock");
        give_mana(
            &mut e,
            1,
            ManaGift {
                r: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 1, "shock");
        e.apply_scenario_command(1, &cast_spell(slot, target_player(0)))
            .expect("shock the active player");
        e.apply_scenario_command(1, &pass()).expect("p1 pass");
        e.apply_scenario_command(0, &pass())
            .expect("p0 pass resolves shock");
        assert_eq!(
            e.state.players[0].life, 18,
            "{}: noncombat damage is not prevented",
            case.id
        );
    }
}

#[test]
fn darkness_batch_cards_prevention_expires_at_end_of_turn() {
    for (index, case) in cases().into_iter().enumerate() {
        let decks = Some(vec![deck_with("forest", &[]), deck_with("forest", &[])]);
        let mut e = GameEngine::new(
            tricerules_cards::registry::global(),
            842_000 + index as u64,
            &[0, 1],
            20,
            decks,
            true,
        )
        .expect("engine");
        advance_to_main1_from_game_start(&mut e);
        give_mana(&mut e, 0, gift(&case.mana));
        inject_card_into_hand(&mut e, 0, case.id);
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
        e.apply_scenario_command(0, &pass()).expect("p0 pass");
        e.apply_scenario_command(1, &pass())
            .expect("p1 pass resolves the prevention");
        assert!(
            !e.state.damage_prevention_effects.is_empty(),
            "{}: prevention is active this turn",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert!(
            e.state.damage_prevention_effects.is_empty(),
            "{}: the prevention effect ends with the turn",
            case.id
        );
    }
}
