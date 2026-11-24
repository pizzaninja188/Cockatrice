//! Actual-card scenarios for the queue-v74 02-flashback-utility cohort.
//!
//! Expectations are printed Oracle values. Flashback is an alternative graveyard cost that
//! always exiles the spell (CR 702.34a-d) and still obeys the card type's timing. Damage follows
//! CR 120.3 and the attacking/blocking role from CR 508/509; the basic-land search follows
//! CR 701.23/701.24a; the flying grant ends at cleanup (CR 611.2a/514.2); combat damage is
//! replaced by zero for the turn (CR 614.1a/615.1a); and a Treasure token is an artifact
//! (CR 111.10f, 701.6a).
use super::helpers::*;
use tricerules_cards::primitives::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{CastMethod, CastSource, RuledCommand};

struct FlashCase {
    id: &'static str,
    mana: ManaGift,
    flashback: ManaGift,
}

fn flash_cases() -> [FlashCase; 5] {
    [
        FlashCase {
            id: "burning_oil",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            flashback: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
        },
        FlashCase {
            id: "deep_reconnaissance",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            flashback: ManaGift {
                g: 1,
                c: 4,
                ..Default::default()
            },
        },
        FlashCase {
            id: "defy_gravity",
            mana: ManaGift {
                u: 1,
                ..Default::default()
            },
            flashback: ManaGift {
                u: 1,
                ..Default::default()
            },
        },
        FlashCase {
            id: "moments_peace",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            flashback: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
        },
        FlashCase {
            id: "strike_it_rich",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            flashback: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
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

fn combat_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
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

fn pool(e: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("object").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

fn flashback_cast(spell: u32, targets: Vec<TargetRef>, generation: u64) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Flashback as i32,
            source: Some(graveyard_cast_source(spell, generation)),
            targets,
            ..Default::default()
        })),
    }
}

fn cast_in_main(e: &mut GameEngine, id: &str, mana: &ManaGift, targets: Vec<TargetRef>) -> u32 {
    let spell = inject_card_into_hand(e, 0, id);
    give_mana(e, 0, gift(mana));
    let slot = hand_index_for_card(e, 0, id);
    e.apply_scenario_command(0, &cast_spell(slot, targets))
        .unwrap_or_else(|error| panic!("{id}: cast: {error:?}"));
    resolve_entire_stack_two_player(e);
    assert_eq!(pool(e, 0), (0, 0, 0, 0, 0, 0), "{id}: exact printed cost");
    assert_eq!(
        e.state.objects[&spell].zone,
        Zone::Graveyard,
        "{id}: first cast resolves"
    );
    spell
}

#[test]
fn q74_flashback_canary_simplest_strike_it_rich_creates_treasures_and_exiles() {
    let case = &flash_cases()[4];
    let mut e = engine(74_200_001);
    let spell = cast_in_main(&mut e, case.id, &case.mana, vec![]);
    assert_eq!(
        battlefield_token_oids(&e, 0, "treasure").len(),
        1,
        "the first cast creates exactly one Treasure"
    );
    let generation = e.state.zone_change_generation[&spell];
    give_mana(&mut e, 0, gift(&case.flashback));
    e.apply_scenario_command(0, &flashback_cast(spell, vec![], generation))
        .expect("flashback Strike It Rich");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact flashback cost");
    assert_eq!(
        battlefield_token_oids(&e, 0, "treasure").len(),
        2,
        "the flashback creates a second Treasure"
    );
    assert_eq!(
        e.state.objects[&spell].zone,
        Zone::Exile,
        "flashback exiles the spell"
    );
}

#[test]
fn q74_flashback_canary_hardest_burning_oil_kills_two_attackers_and_exiles() {
    let case = &flash_cases()[0];
    let mut e = combat_engine(74_200_002);
    let first = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let second = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 2);
    e.apply_scenario_command(0, &declare_attackers(vec![first, second]))
        .expect("declare two attackers");
    let spell = cast_in_main(&mut e, case.id, &case.mana, target_object(first));
    assert_eq!(
        e.state.objects[&first].zone,
        Zone::Graveyard,
        "three damage kills the first attacking creature"
    );
    assert_eq!(
        e.state.objects[&second].zone,
        Zone::Battlefield,
        "the second attacker survives the first cast"
    );
    let generation = e.state.zone_change_generation[&spell];
    give_mana(&mut e, 0, gift(&case.flashback));
    e.apply_scenario_command(0, &flashback_cast(spell, target_object(second), generation))
        .expect("flashback Burning Oil");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact flashback cost");
    assert_eq!(
        e.state.objects[&second].zone,
        Zone::Graveyard,
        "the flashback kills the second attacker"
    );
    assert_eq!(e.state.objects[&spell].zone, Zone::Exile);
}

#[test]
fn q74_flashback_utility_cards_apply_their_printed_bodies_and_flash_back() {
    for (index, case) in flash_cases().into_iter().enumerate() {
        let mut e = engine(74_201_000 + index as u64);
        match case.id {
            "burning_oil" => {
                let mut e = combat_engine(74_201_100 + index as u64);
                let first = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
                let second = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 2);
                e.apply_scenario_command(0, &declare_attackers(vec![first, second]))
                    .expect("declare two attackers");
                let spell = cast_in_main(&mut e, case.id, &case.mana, target_object(first));
                assert_eq!(e.state.objects[&first].zone, Zone::Graveyard);
                let generation = e.state.zone_change_generation[&spell];
                give_mana(&mut e, 0, gift(&case.flashback));
                e.apply_scenario_command(
                    0,
                    &flashback_cast(spell, target_object(second), generation),
                )
                .expect("flashback at the second attacker");
                resolve_entire_stack_two_player(&mut e);
                assert_eq!(e.state.objects[&second].zone, Zone::Graveyard);
                assert_eq!(e.state.objects[&spell].zone, Zone::Exile);
                continue;
            }
            "deep_reconnaissance" => {
                let opponent_library = e.state.players[1].library.clone();
                let first = inject_library_card(&mut e, 0, "forest");
                e.state.players[0]
                    .library
                    .retain(|candidate| *candidate != first);
                e.state.players[0].library.push_front(first);
                let spell = inject_card_into_hand(&mut e, 0, case.id);
                give_mana(&mut e, 0, gift(&case.mana));
                let slot = hand_index_for_card(&e, 0, case.id);
                e.apply_scenario_command(0, &cast_spell(slot, vec![]))
                    .expect("cast");
                e.apply_scenario_command(0, &pass()).expect("caster pass");
                let batch = e.apply_scenario_command(1, &pass()).expect("search");
                let choice = find_resolution_choice(&batch).expect("basic search choice");
                assert_eq!((choice.min, choice.max), (0, 1));
                let completion = e
                    .apply_scenario_command(0, &submit_resolution_choice(vec![first]))
                    .expect("choose the basic");
                assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed cost");
                assert_eq!(e.state.objects[&first].zone, Zone::Battlefield);
                assert!(e.state.objects[&first].tapped);
                assert!(e.state.players[0].battlefield.contains(&first));
                assert_eq!(
                    e.state.players[1].library, opponent_library,
                    "only the caster's library is searched"
                );
                assert!(
                    completion.events.iter().any(|event| matches!(
                        &event.ev,
                        Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
                    )),
                    "Deep Reconnaissance shuffles after searching"
                );
                let generation = e.state.zone_change_generation[&spell];
                let second = inject_library_card(&mut e, 0, "island");
                e.state.players[0]
                    .library
                    .retain(|candidate| *candidate != second);
                e.state.players[0].library.push_front(second);
                give_mana(&mut e, 0, gift(&case.flashback));
                e.apply_scenario_command(0, &flashback_cast(spell, vec![], generation))
                    .expect("flashback search");
                e.apply_scenario_command(0, &pass()).expect("caster pass");
                let batch = e
                    .apply_scenario_command(1, &pass())
                    .expect("flashback search");
                let choice = find_resolution_choice(&batch).expect("second basic search choice");
                assert_eq!((choice.min, choice.max), (0, 1));
                let completion = e
                    .apply_scenario_command(0, &submit_resolution_choice(vec![second]))
                    .expect("choose the second basic");
                assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact flashback cost");
                assert_eq!(e.state.objects[&second].zone, Zone::Battlefield);
                assert!(e.state.objects[&second].tapped);
                assert_eq!(
                    e.state.players[1].library, opponent_library,
                    "the flashback also searches only the caster's library"
                );
                assert!(
                    completion.events.iter().any(|event| matches!(
                        &event.ev,
                        Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
                    )),
                    "the flashback copy also shuffles"
                );
                assert_eq!(e.state.objects[&spell].zone, Zone::Exile);
                continue;
            }
            "defy_gravity" => {
                let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
                let spell = cast_in_main(&mut e, case.id, &case.mana, target_object(target));
                assert!(e.effective_has_keyword(target, Keyword::Flying));
                let generation = e.state.zone_change_generation[&spell];
                give_mana(&mut e, 0, gift(&case.flashback));
                e.apply_scenario_command(
                    0,
                    &flashback_cast(spell, target_object(target), generation),
                )
                .expect("flashback Defy Gravity");
                resolve_entire_stack_two_player(&mut e);
                assert!(e.effective_has_keyword(target, Keyword::Flying));
                assert_eq!(e.state.objects[&spell].zone, Zone::Exile);
                end_active_turn(&mut e, 0);
                assert!(
                    !e.effective_has_keyword(target, Keyword::Flying),
                    "the grant expires at cleanup"
                );
                continue;
            }
            "moments_peace" => {
                // The prevention body is exercised by the combat test below; this path proves
                // exact costs, the flashback copy and cleanup expiry.
                let spell = inject_card_into_hand(&mut e, 0, case.id);
                give_mana(&mut e, 0, gift(&case.mana));
                let slot = hand_index_for_card(&e, 0, case.id);
                e.apply_scenario_command(0, &cast_spell(slot, vec![]))
                    .expect("cast");
                resolve_entire_stack_two_player(&mut e);
                assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed cost");
                assert!(!e.state.damage_prevention_effects.is_empty());
                let generation = e.state.zone_change_generation[&spell];
                give_mana(&mut e, 0, gift(&case.flashback));
                e.apply_scenario_command(0, &flashback_cast(spell, vec![], generation))
                    .expect("flashback Moment's Peace");
                resolve_entire_stack_two_player(&mut e);
                assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact flashback cost");
                assert_eq!(e.state.objects[&spell].zone, Zone::Exile);
                end_active_turn(&mut e, 0);
                assert!(
                    e.state.damage_prevention_effects.is_empty(),
                    "the prevention expires at end of turn"
                );
                continue;
            }
            "strike_it_rich" => {
                cast_in_main(&mut e, case.id, &case.mana, vec![]);
                assert_eq!(battlefield_token_oids(&e, 0, "treasure").len(), 1);
                continue;
            }
            other => panic!("unhandled {other}"),
        }
    }
}

#[test]
fn q74_moments_peace_prevents_all_combat_damage_this_turn() {
    let case = &flash_cases()[3];
    let mut e = combat_engine(74_202_001);
    let attacker = *e.state.players[0]
        .battlefield
        .last()
        .expect("attacker on battlefield");
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_scenario_command(0, &pass()).expect("p0 passes");
    inject_card_into_hand(&mut e, 1, case.id);
    give_mana(&mut e, 1, gift(&case.mana));
    let slot = hand_index_for_card(&e, 1, case.id);
    e.apply_scenario_command(1, &cast_spell(slot, vec![]))
        .expect("Moment's Peace");
    e.apply_scenario_command(1, &pass()).expect("p1 pass");
    e.apply_scenario_command(0, &pass())
        .expect("prevention resolves");
    e.apply_scenario_command(0, &pass())
        .expect("p0 passes after prevention");
    e.apply_scenario_command(1, &pass())
        .expect("p1 passes to blockers");
    e.apply_scenario_command(0, &pass())
        .expect("p0 passes in blockers");
    let batch = e.apply_scenario_command(1, &pass()).expect("combat damage");
    assert!(
        batch.events.iter().all(|event| {
            !matches!(
                &event.ev,
                Some(tricerules_proto::ruled::v1::ruled_event::Ev::LifeChanged(change))
                    if change.delta < 0
            )
        }),
        "no combat life loss"
    );
    assert_eq!(e.state.players[1].life, 20);
    assert_eq!(
        e.state.objects[&attacker].zone,
        Zone::Battlefield,
        "prevention does not remove the attacker"
    );
}

#[test]
fn q74_flashback_utility_fizzles_stale_targets_and_exiles_flashback_copies() {
    // Burning Oil: the only target leaves before resolution; the first cast has no effect and
    // the flashback copy is still exiled.
    let mut e = combat_engine(74_203_100);
    let first = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let second = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 2);
    e.apply_scenario_command(0, &declare_attackers(vec![first, second]))
        .expect("declare two attackers");
    let spell = inject_card_into_hand(&mut e, 0, "burning_oil");
    give_mana(&mut e, 0, gift(&flash_cases()[0].mana));
    let slot = hand_index_for_card(&e, 0, "burning_oil");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(first)))
        .expect("cast at the first attacker");
    move_to_graveyard(&mut e, 0, first);
    let batch = pass_both_players_with_batch(&mut e);
    assert!(
        batch.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::Log(log)) if log.text.contains("Burning Oil") && log.text.contains("fizzles")
        )),
        "the first Burning Oil fizzles on its stale target"
    );
    assert_eq!(e.state.players[0].life, 20);
    assert_eq!(e.state.players[1].life, 20);
    assert_eq!(
        e.state.objects[&spell].zone,
        Zone::Graveyard,
        "the fizzled first cast goes to the graveyard"
    );
    assert!(e.state.stack.is_empty());
    let generation = e.state.zone_change_generation[&spell];
    give_mana(&mut e, 0, gift(&flash_cases()[0].flashback));
    e.apply_scenario_command(0, &flashback_cast(spell, target_object(second), generation))
        .expect("flashback at the second attacker");
    move_to_graveyard(&mut e, 0, second);
    let batch = pass_both_players_with_batch(&mut e);
    assert!(
        batch.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::Log(log)) if log.text.contains("Burning Oil") && log.text.contains("fizzles")
        )),
        "the flashback Burning Oil fizzles on its stale target"
    );
    assert_eq!(e.state.players[0].life, 20);
    assert_eq!(e.state.players[1].life, 20);
    assert_eq!(
        e.state.objects[&spell].zone,
        Zone::Exile,
        "a flashback copy is exiled even when its target is stale"
    );
    assert!(e.state.stack.is_empty());

    // Defy Gravity: a stale creature target fizzles and grants no flying.
    let mut e2 = engine(74_203_102);
    let stale = inject_creature_with_stats(&mut e2, 0, "grizzly_bears", 2, 2);
    let stale_spell = inject_card_into_hand(&mut e2, 0, "defy_gravity");
    give_mana(&mut e2, 0, gift(&flash_cases()[2].mana));
    let slot = hand_index_for_card(&e2, 0, "defy_gravity");
    e2.apply_scenario_command(0, &cast_spell(slot, target_object(stale)))
        .expect("cast at the creature");
    e2.state.players[0]
        .battlefield
        .retain(|object| *object != stale);
    e2.state.players[0].hand.push(stale);
    e2.state.objects.get_mut(&stale).expect("creature").zone = Zone::Hand;
    *e2.state.zone_change_generation.entry(stale).or_default() += 1;
    let batch = pass_both_players_with_batch(&mut e2);
    assert!(
        batch.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::Log(log)) if log.text.contains("Defy Gravity") && log.text.contains("fizzles")
        )),
        "Defy Gravity fizzles on its stale target"
    );
    assert_eq!(e2.state.objects[&stale_spell].zone, Zone::Graveyard);
    assert_eq!(e2.state.objects[&stale].zone, Zone::Hand);
    assert!(e2.state.stack.is_empty());
}

#[test]
fn q74_flashback_utility_rejects_wrong_targets_and_illegal_flashback() {
    // Burning Oil rejects an idle creature.
    let mut e = engine(74_203_001);
    let idle = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let spell = inject_card_into_hand(&mut e, 0, "burning_oil");
    give_mana(&mut e, 0, gift(&flash_cases()[0].mana));
    let before = pool(&e, 0);
    let hand_before = e.state.players[0].hand.clone();
    let slot = hand_index_for_card(&e, 0, "burning_oil");
    let error = e
        .apply_scenario_command(0, &cast_spell(slot, target_object(idle)))
        .expect_err("an idle creature is not attacking or blocking");
    assert!(matches!(error, tricerules_core::EngineError::Illegal(_)));
    assert_eq!(pool(&e, 0), before, "rejected Burning Oil spends no mana");
    assert_eq!(e.state.players[0].hand, hand_before);
    assert_eq!(e.state.objects[&spell].zone, Zone::Hand);
    assert!(e.state.stack.is_empty());

    // Defy Gravity rejects a noncreature permanent.
    let mut e = engine(74_203_002);
    let land = inject_permanent_on_battlefield(&mut e, 0, "forest");
    let spell = inject_card_into_hand(&mut e, 0, "defy_gravity");
    give_mana(&mut e, 0, gift(&flash_cases()[2].mana));
    let before = pool(&e, 0);
    let hand_before = e.state.players[0].hand.clone();
    let slot = hand_index_for_card(&e, 0, "defy_gravity");
    let error = e
        .apply_scenario_command(0, &cast_spell(slot, target_object(land)))
        .expect_err("Defy Gravity requires a creature");
    assert!(matches!(error, tricerules_core::EngineError::Illegal(_)));
    assert_eq!(pool(&e, 0), before, "rejected Defy Gravity spends no mana");
    assert_eq!(e.state.players[0].hand, hand_before);
    assert_eq!(e.state.objects[&spell].zone, Zone::Hand);
    assert!(e.state.stack.is_empty());

    // A flashback spell cannot be cast from exile.
    let mut e = engine(74_203_003);
    let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let spell = cast_in_main(
        &mut e,
        "defy_gravity",
        &flash_cases()[2].mana,
        target_object(target),
    );
    let generation = e.state.zone_change_generation[&spell];
    give_mana(&mut e, 0, gift(&flash_cases()[2].flashback));
    e.apply_scenario_command(0, &flashback_cast(spell, target_object(target), generation))
        .expect("first flashback");
    e.apply_scenario_command(0, &pass()).expect("p0 pass");
    e.apply_scenario_command(1, &pass())
        .expect("flashback resolves");
    assert_eq!(e.state.objects[&spell].zone, Zone::Exile);
    let exiled_generation = e.state.zone_change_generation[&spell];
    give_mana(&mut e, 0, gift(&flash_cases()[2].flashback));
    let exiled = RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Flashback as i32,
            source: Some(CastSource {
                location: Some(
                    tricerules_proto::ruled::v1::cast_source::Location::ExileObjectId(spell),
                ),
                expected_zone_change_generation: Some(exiled_generation),
            }),
            targets: vec![],
            ..Default::default()
        })),
    };
    let before = pool(&e, 0);
    let hand_before = e.state.players[0].hand.clone();
    let error = e
        .apply_scenario_command(0, &exiled)
        .expect_err("flashback cannot be cast from exile");
    assert!(matches!(error, tricerules_core::EngineError::Illegal(_)));
    assert_eq!(pool(&e, 0), before, "rejected flashback spends no mana");
    assert_eq!(e.state.players[0].hand, hand_before);
    assert_eq!(e.state.objects[&spell].zone, Zone::Exile);
    assert!(e.state.stack.is_empty());
}
