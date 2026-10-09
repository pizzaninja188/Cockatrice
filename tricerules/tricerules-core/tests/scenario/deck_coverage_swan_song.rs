//! Actual-card coverage for Swan Song's three legal spell types, target-controller receipt, and
//! uncounterable-spell ruling.
//!
//! Oracle and rulings were checked against Scryfall on 2026-09-26. CR 608.2b governs legality at
//! resolution, CR 701.6a governs countering, and CR 111.3-4 define the Bird token's characteristics,
//! name, and subtype.

use super::helpers::*;
use tricerules_cards::{Color, Keyword};
use tricerules_core::{GameEngine, Zone};

const SWAN_SONG: &str = "swan_song";
const BIRD_TOKEN: &str = "bird_u_2_2_flying";

fn swan_song_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("island", &[])]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new Swan Song game");
    advance_to_main1_from_game_start(&mut engine);
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

fn cast_swan_song(engine: &mut GameEngine, target_spell: u32) {
    give_mana(
        engine,
        1,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(engine, 1, SWAN_SONG);
    let slot = hand_index_for_card(engine, 1, SWAN_SONG);
    engine
        .apply_command(1, &cast_spell(slot, stack_target(target_spell)))
        .expect("cast Swan Song targeting the spell");
}

fn assert_bird_created_for(engine: &GameEngine, controller: usize) {
    let birds = battlefield_token_oids(engine, controller, BIRD_TOKEN);
    assert_eq!(birds.len(), 1, "target spell's controller gets one Bird");
    let bird = birds[0];
    assert_eq!(engine.state.objects[&bird].zone, Zone::Battlefield);
    assert_eq!(engine.effective_power(bird), Some(2));
    assert_eq!(engine.effective_toughness(bird), Some(2));
    assert!(engine.effective_has_keyword(bird, Keyword::Flying));
    assert_eq!(
        engine.characteristics(bird).unwrap().colors,
        vec![Color::Blue]
    );
}

fn assert_no_bird(engine: &GameEngine) {
    assert!(battlefield_token_oids(engine, 0, BIRD_TOKEN).is_empty());
    assert!(battlefield_token_oids(engine, 1, BIRD_TOKEN).is_empty());
}

#[test]
fn swan_song_targets_each_allowed_spell_type_and_gives_its_controller_a_bird() {
    let cases = [
        (
            "opt",
            ManaGift {
                u: 1,
                ..Default::default()
            },
        ),
        (
            "divination",
            ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
        ),
        (
            "pacifism",
            ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
        ),
    ];

    for (index, (card_id, mana)) in cases.into_iter().enumerate() {
        let mut engine = swan_song_engine(202_609_270 + index as u64);
        let permanent = (card_id == "pacifism")
            .then(|| inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears"));
        let target = inject_card_into_hand(&mut engine, 0, card_id);
        give_mana(&mut engine, 0, mana);
        let slot = hand_index_for_card(&engine, 0, card_id);
        let targets = permanent.map_or_else(Vec::new, target_object);
        engine
            .apply_command(0, &cast_spell(slot, targets))
            .expect("cast the selected instant, sorcery, or enchantment");
        engine
            .apply_command(0, &pass())
            .expect("pass to Swan Song's controller");

        cast_swan_song(&mut engine, target);
        engine
            .apply_command(1, &pass())
            .expect("pass after Swan Song");
        engine.apply_command(0, &pass()).expect("resolve Swan Song");

        assert_eq!(
            engine.state.objects[&target].zone,
            Zone::Graveyard,
            "{card_id}"
        );
        assert_bird_created_for(&engine, 0);
        assert!(battlefield_token_oids(&engine, 1, BIRD_TOKEN).is_empty());
    }
}

#[test]
fn swan_song_gives_a_bird_even_when_its_target_cannot_be_countered() {
    let mut engine = swan_song_engine(202_609_274);
    let creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let eject = inject_card_into_hand(&mut engine, 0, "eject");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "eject");
    engine
        .apply_command(0, &cast_spell(slot, target_object(creature)))
        .expect("cast the uncounterable instant Eject");
    engine
        .apply_command(0, &pass())
        .expect("pass to Swan Song's controller");

    cast_swan_song(&mut engine, eject);
    engine
        .apply_command(1, &pass())
        .expect("pass after Swan Song");
    engine.apply_command(0, &pass()).expect("resolve Swan Song");

    assert!(engine.state.stack.iter().any(|item| item.id == eject));
    assert_eq!(engine.state.objects[&creature].zone, Zone::Battlefield);
    assert_bird_created_for(&engine, 0);
    assert!(battlefield_token_oids(&engine, 1, BIRD_TOKEN).is_empty());

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Hand);
}

#[test]
fn swan_song_rejects_a_creature_spell_as_a_target() {
    let mut engine = swan_song_engine(202_609_275);
    let creature = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast the creature spell");
    engine
        .apply_command(0, &pass())
        .expect("pass to Swan Song's controller");

    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut engine, 1, SWAN_SONG);
    let slot = hand_index_for_card(&engine, 1, SWAN_SONG);
    let command_index = engine.state.command_index;
    assert!(engine
        .apply_command(1, &cast_spell(slot, stack_target(creature)))
        .is_err());
    assert_eq!(engine.state.command_index, command_index);
    assert!(engine.state.stack.iter().any(|item| item.id == creature));
    assert_no_bird(&engine);
}

#[test]
fn swan_song_creates_no_bird_if_its_target_leaves_the_stack_first() {
    let mut engine = swan_song_engine(202_609_276);
    let target = inject_card_into_hand(&mut engine, 0, "opt");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "opt");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Opt");
    engine
        .apply_command(0, &pass())
        .expect("pass to Swan Song's controller");
    cast_swan_song(&mut engine, target);
    engine
        .apply_command(1, &pass())
        .expect("pass to Opt's controller");

    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut engine, 0, "counterspell");
    let slot = hand_index_for_card(&engine, 0, "counterspell");
    engine
        .apply_command(0, &cast_spell(slot, stack_target(target)))
        .expect("counter Opt before Swan Song resolves");
    engine
        .apply_command(0, &pass())
        .expect("pass counterspell response");
    engine
        .apply_command(1, &pass())
        .expect("resolve the counterspell");
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);

    resolve_entire_stack_two_player(&mut engine);
    assert_no_bird(&engine);
}
