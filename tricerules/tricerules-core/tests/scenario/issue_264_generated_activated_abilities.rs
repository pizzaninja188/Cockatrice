//! Issue #264 — generated activated abilities reuse engine-owned atomic costs, private library
//! search, selectable mana, per-turn accounting, source identity, and temporary P/T effects.
//!
//! Oracle and rulings checked 2026-09-12. CR 106, 117, 118, 400.7, 602, and 605 govern mana,
//! priority, costs, zone-change identity, and activated/mana abilities; CR 613.4c and 611.2a
//! govern the temporary P/T effects; CR 701.21 and 701.23 govern sacrifice and search/shuffle.

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::permanent_moved::Destination;

fn activate_with_mana_option(
    engine: &GameEngine,
    source: u32,
    ability_index: u32,
    option: u32,
) -> RuledCommand {
    let mut command = activate_ability_for(engine, source, ability_index, Vec::new());
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = option;
    command
}

#[test]
fn generated_terramorphic_expanse_pays_atomically_and_searches_with_exact_identity() {
    let decks = Some(vec![
        deck_with("forest", &["terramorphic_expanse", "vibrant_cityscape"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(264_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    let wrong_zone = inject_card_into_hand(&mut engine, 0, "vibrant_cityscape");
    assert!(apply_ability(&mut engine, 0, wrong_zone, 0, Vec::new()).is_err());
    assert_eq!(engine.state.objects[&wrong_zone].zone, Zone::Hand);

    let expanse = relocate_to_battlefield(&mut engine, 0, "terramorphic_expanse", false);
    let forest = inject_library_card(&mut engine, 0, "forest");
    let taiga = inject_library_card(&mut engine, 0, "taiga");

    let mut stale = activate_ability_for(&engine, expanse, 0, Vec::new());
    let Some(Cmd::ActivateAbility(activation)) = stale.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation += 1;
    assert!(engine.apply_command(0, &stale).is_err());
    assert_eq!(engine.state.objects[&expanse].zone, Zone::Battlefield);
    assert!(!engine.state.objects[&expanse].tapped);

    engine.state.objects.get_mut(&expanse).unwrap().tapped = true;
    assert!(apply_ability(&mut engine, 0, expanse, 0, Vec::new()).is_err());
    assert_eq!(engine.state.objects[&expanse].zone, Zone::Battlefield);
    engine.state.objects.get_mut(&expanse).unwrap().tapped = false;

    let payment = apply_ability(&mut engine, 0, expanse, 0, Vec::new())
        .expect("pay tap and sacrifice costs together");
    assert_eq!(engine.state.objects[&expanse].zone, Zone::Graveyard);
    assert!(permanents_moved_in(&payment).iter().any(|moved| {
        moved.object_id == expanse && moved.destination == Destination::Graveyard as i32
    }));

    engine.apply_command(0, &pass()).expect("controller passes");
    let search = engine
        .apply_command(1, &pass())
        .expect("resolve search ability");
    let choice = find_resolution_choice(&search).expect("private basic-land search");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert!(choice.candidate_object_ids.contains(&forest));
    assert!(!choice.candidate_object_ids.contains(&taiga));

    let generation_before = engine
        .state
        .zone_change_generation
        .get(&forest)
        .copied()
        .unwrap_or(0);
    let completion = engine
        .apply_command(0, &submit_resolution_choice(vec![forest]))
        .expect("choose the published basic land");
    assert_eq!(engine.state.objects[&forest].zone, Zone::Battlefield);
    assert!(engine.state.objects[&forest].tapped);
    assert_eq!(
        engine.state.zone_change_generation[&forest],
        generation_before + 1
    );
    assert!(completion.events.iter().any(|event| {
        matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library.")
    }));
}

#[test]
fn generated_scarecrow_guide_offers_five_colors_and_resets_its_turn_limit() {
    let mut engine = anthem_engine(264_002, "scarecrow_guide");
    let guide = relocate_to_battlefield(&mut engine, 0, "scarecrow_guide", false);

    assert!(engine
        .apply_command(0, &activate_with_mana_option(&engine, guide, 0, 0))
        .is_err());
    assert!(engine.state.activation_uses_this_turn.is_empty());

    let mut sources = vec![guide];
    for _ in 1..5 {
        sources.push(inject_creature_on_battlefield(
            &mut engine,
            0,
            "scarecrow_guide",
        ));
    }
    for (option, source) in sources.iter().copied().enumerate() {
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        engine
            .apply_command(
                0,
                &activate_with_mana_option(&engine, source, 0, option as u32),
            )
            .expect("pay one and produce the selected color");
    }
    let pool = &engine.state.players[0].mana_pool;
    assert_eq!(
        (pool.white, pool.blue, pool.black, pool.red, pool.green),
        (1, 1, 1, 1, 1)
    );
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let pool_before = engine.state.players[0].mana_pool;
    engine
        .apply_command(0, &activate_with_mana_option(&engine, guide, 0, 4))
        .expect_err("one source cannot activate twice in the same turn");
    assert_eq!(engine.state.players[0].mana_pool, pool_before);

    end_active_turn(&mut engine, 0);
    assert!(engine.state.activation_uses_this_turn.is_empty());
    engine.state.priority_idx = 0;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &activate_with_mana_option(&engine, guide, 0, 4))
        .expect("per-turn activation resets on the next turn");
    assert_eq!(engine.state.players[0].mana_pool.green, 1);
}

#[test]
fn generated_self_pumps_pay_exact_costs_obey_limits_and_expire_at_cleanup() {
    let mut engine = anthem_engine(264_003, "burrog_banemaker");
    let burrog = relocate_to_battlefield(&mut engine, 0, "burrog_banemaker", false);
    let cats = inject_creature_on_battlefield(&mut engine, 0, "kravens_cats");

    let mut stale = activate_ability_for(&engine, cats, 0, Vec::new());
    let Some(Cmd::ActivateAbility(activation)) = stale.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation += 1;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let pool_before = engine.state.players[0].mana_pool;
    engine
        .apply_command(0, &stale)
        .expect_err("stale source generation is illegal");
    assert_eq!(engine.state.players[0].mana_pool, pool_before);

    apply_ability(&mut engine, 0, cats, 0, Vec::new()).expect("activate Cats");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(cats), Some(4));
    assert_eq!(engine.effective_toughness(cats), Some(4));
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &activate_ability_for(&engine, cats, 0, Vec::new()))
        .expect_err("Cats is exhausted for this turn");

    assert!(apply_ability(&mut engine, 0, burrog, 0, Vec::new()).is_err());
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 2,
            c: 2,
            ..Default::default()
        },
    );
    for _ in 0..2 {
        apply_ability(&mut engine, 0, burrog, 0, Vec::new()).expect("repeatable Burrog pump");
        resolve_entire_stack_two_player(&mut engine);
    }
    assert_eq!(engine.effective_power(burrog), Some(3));
    assert_eq!(engine.effective_toughness(burrog), Some(3));

    end_active_turn(&mut engine, 0);
    assert_eq!(engine.effective_power(burrog), Some(1));
    assert_eq!(engine.effective_toughness(burrog), Some(1));
    assert_eq!(engine.effective_power(cats), Some(2));
    assert_eq!(engine.effective_toughness(cats), Some(2));
}

#[test]
fn generated_team_pump_snapshots_only_controlled_creatures_until_cleanup() {
    let mut engine = anthem_engine(264_004, "dwarven_provisioner");
    let provisioner = relocate_to_battlefield(&mut engine, 0, "dwarven_provisioner", false);
    let mine = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let theirs = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    assert!(apply_ability(&mut engine, 0, provisioner, 0, Vec::new()).is_err());
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 3,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, provisioner, 0, Vec::new()).expect("activate team pump");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.effective_power(provisioner), Some(3));
    assert_eq!(engine.effective_power(mine), Some(3));
    assert_eq!(engine.effective_toughness(mine), Some(3));
    assert_eq!(engine.effective_power(theirs), Some(2));
    let late = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    assert_eq!(engine.effective_power(late), Some(2));

    end_active_turn(&mut engine, 0);
    assert_eq!(engine.effective_power(provisioner), Some(2));
    assert_eq!(engine.effective_power(mine), Some(2));
}
