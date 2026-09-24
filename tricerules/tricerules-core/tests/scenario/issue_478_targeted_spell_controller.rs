use crate::helpers::*;
use tricerules_core::Zone;

fn setup(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &["eject", "opt", "grizzly_bears"]),
        deck_with("island", &["an_offer_you_cant_refuse"]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "eject");
    ensure_in_hand(&mut engine, 1, "an_offer_you_cant_refuse");
    engine
}

fn stack_target(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id,
        damage_amount: 0,
        group_index: 0,
        kind: TargetRefKind::Stack as i32,
    }]
}

#[test]
fn issue_478_legal_uncounterable_spell_gives_its_controller_treasure() {
    let mut engine = setup(478_101);
    let permanent = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 4,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "eject");
    engine
        .apply_command(0, &cast_spell(slot, target_object(permanent)))
        .unwrap();
    let eject = engine.state.stack.last().unwrap().id;
    engine.apply_command(0, &pass()).unwrap();
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 1, "an_offer_you_cant_refuse");
    engine
        .apply_command(1, &cast_spell(slot, stack_target(eject)))
        .unwrap();
    engine.apply_command(1, &pass()).unwrap();
    engine.apply_command(0, &pass()).unwrap();
    assert!(engine.state.stack.iter().any(|item| item.id == eject));
    assert_eq!(battlefield_token_oids(&engine, 0, "treasure").len(), 2);
    assert!(battlefield_token_oids(&engine, 1, "treasure").is_empty());
}

#[test]
fn issue_478_countered_spell_gives_target_controller_treasure() {
    let mut engine = setup(478_102);
    ensure_in_hand(&mut engine, 0, "opt");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "opt");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    let target = engine.state.stack.last().unwrap().id;
    engine.apply_command(0, &pass()).unwrap();
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 1, "an_offer_you_cant_refuse");
    engine
        .apply_command(1, &cast_spell(slot, stack_target(target)))
        .unwrap();
    engine.apply_command(1, &pass()).unwrap();
    engine.apply_command(0, &pass()).unwrap();
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(battlefield_token_oids(&engine, 0, "treasure").len(), 2);
    assert!(battlefield_token_oids(&engine, 1, "treasure").is_empty());
}

#[test]
fn issue_478_illegal_stack_target_fizzles_without_treasure() {
    let mut engine = setup(478_103);
    ensure_in_hand(&mut engine, 0, "opt");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "opt");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    let target = engine.state.stack.last().unwrap().id;
    engine.apply_command(0, &pass()).unwrap();
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 1, "an_offer_you_cant_refuse");
    engine
        .apply_command(1, &cast_spell(slot, stack_target(target)))
        .unwrap();
    // The target leaves the stack before Offer resolves, as after a responding counterspell.
    engine.state.stack.retain(|item| item.id != target);
    engine.state.objects.get_mut(&target).unwrap().zone = Zone::Graveyard;
    engine.state.players[0].graveyard.push(target);
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    engine.apply_command(1, &pass()).unwrap();
    engine.apply_command(0, &pass()).unwrap();
    assert!(battlefield_token_oids(&engine, 0, "treasure").is_empty());
    assert!(battlefield_token_oids(&engine, 1, "treasure").is_empty());
}

#[test]
fn issue_478_rejects_creature_spells_at_cast() {
    let mut engine = setup(478_104);
    ensure_in_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            g: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "grizzly_bears");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    let target = engine.state.stack.last().unwrap().id;
    engine.apply_command(0, &pass()).unwrap();
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 1, "an_offer_you_cant_refuse");
    let revision = engine.state.command_index;
    assert!(engine
        .apply_command(1, &cast_spell(slot, stack_target(target)))
        .is_err());
    assert_eq!(engine.state.command_index, revision);
    assert!(battlefield_token_oids(&engine, 0, "treasure").is_empty());
}
