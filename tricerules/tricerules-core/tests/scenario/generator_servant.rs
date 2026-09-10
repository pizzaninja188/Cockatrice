use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{BeginSpellCast, SpellCastAnnouncement};

fn begin_cast(command: RuledCommand) -> RuledCommand {
    let Some(Cmd::CastSpell(cast)) = command.cmd else {
        panic!("expected cast command")
    };
    RuledCommand {
        cmd: Some(Cmd::BeginSpellCast(BeginSpellCast {
            announcement: Some(SpellCastAnnouncement {
                targets: cast.targets,
                x_value: cast.x_value,
                flex_payments: cast.flex_payments,
                face_index: cast.face_index,
                selected_modes: cast.selected_modes,
                source: cast.source,
                cost_selections: cast.cost_selections,
                cast_cost_group_selections: cast.cast_cost_group_selections,
                cast_method: cast.cast_method,
                casting_permission_id: cast.casting_permission_id,
            }),
        })),
    }
}

fn only_tap_cost_flags(engine: &mut GameEngine, object_id: u32) -> Vec<bool> {
    engine
        .initial_response_batch()
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .and_then(|view| view.per_player.first())
        .into_iter()
        .flat_map(|player| player.battlefield_objects.iter())
        .find(|object| object.object_id == object_id)
        .map(|object| {
            object
                .activated_abilities
                .iter()
                .map(|ability| ability.has_only_tap_cost)
                .collect()
        })
        .unwrap_or_default()
}

fn setup(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn activate_generator_servant(engine: &mut GameEngine) -> u32 {
    let servant = inject_permanent_on_battlefield(engine, 0, "generator_servant");
    engine
        .apply_command(0, &activate_ability_for(engine, servant, 0, vec![]))
        .expect("activate Generator Servant");
    assert_eq!(engine.state.objects[&servant].zone, Zone::Graveyard);
    let contribution = engine.state.players[0]
        .restricted_mana
        .last()
        .expect("tagged Generator Servant mana");
    assert_eq!(contribution.amount.c, 2);
    contribution.restriction_group_id
}

fn cast_with_generator_mana(engine: &mut GameEngine, card_id: &str, group: u32) -> u32 {
    inject_card_into_hand(engine, 0, card_id);
    let slot = hand_index_for_card(engine, 0, card_id);
    let object_id = engine.state.players[0].hand[slot];
    let mut command = cast_spell(slot, vec![]);
    let Some(Cmd::CastSpell(cast)) = command.cmd.as_mut() else {
        unreachable!()
    };
    cast.restricted_mana.push(ManaSpendSelection {
        restriction_group_id: group,
        c: 1,
        ..Default::default()
    });
    engine
        .apply_command(0, &command)
        .unwrap_or_else(|error| panic!("cast {card_id} with Generator Servant mana: {error}"));
    object_id
}

#[test]
fn generator_servant_mana_is_unrestricted_but_only_paid_creature_spells_gain_haste() {
    let mut engine = setup(240_001);
    let group = activate_generator_servant(&mut engine);

    let equipment = cast_with_generator_mana(&mut engine, "bonesplitter", group);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&equipment].zone, Zone::Battlefield);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let mana_creature = cast_with_generator_mana(&mut engine, "iron_myr", group);
    assert!(engine
        .characteristics(mana_creature)
        .is_some_and(|characteristics| characteristics.has_keyword(Keyword::Haste)));
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&mana_creature].zone, Zone::Battlefield);
    engine
        .apply_command(0, &activate_ability_for(&engine, mana_creature, 0, vec![]))
        .expect("creature spell paid with Generator Servant mana has haste");
    assert_eq!(engine.state.players[0].mana_pool.red, 1);
    assert!(engine.state.players[0].restricted_mana.is_empty());

    inject_card_into_hand(&mut engine, 0, "iron_myr");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "iron_myr");
    let ordinary_myr = engine.state.players[0].hand[slot];
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast creature without Generator Servant mana");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &activate_ability_for(&engine, ordinary_myr, 0, vec![]))
        .expect_err("a creature not paid with Generator Servant mana remains summoning sick");
}

#[test]
fn generator_servant_can_grant_haste_to_two_different_creature_spells() {
    let mut engine = setup(240_002);
    let group = activate_generator_servant(&mut engine);

    for _ in 0..2 {
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        let mana_creature = cast_with_generator_mana(&mut engine, "iron_myr", group);
        pass_both_players(&mut engine);
        engine
            .apply_command(0, &activate_ability_for(&engine, mana_creature, 0, vec![]))
            .expect("each creature paid with one of the two mana has haste");
    }

    assert!(engine.state.players[0].restricted_mana.is_empty());
}

#[test]
fn generator_servant_publishes_menu_and_pending_payment_metadata() {
    let mut engine = setup(240_003);
    let servant = inject_permanent_on_battlefield(&mut engine, 0, "generator_servant");
    let forest = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    assert_eq!(only_tap_cost_flags(&mut engine, servant), [false]);
    assert_eq!(only_tap_cost_flags(&mut engine, forest), [true]);

    engine
        .apply_command(0, &activate_ability_for(&engine, servant, 0, vec![]))
        .expect("activate Generator Servant");
    let group = engine.state.players[0]
        .restricted_mana
        .last()
        .expect("Generator Servant mana")
        .restriction_group_id;
    inject_card_into_hand(&mut engine, 0, "iron_myr");
    let slot = hand_index_for_card(&engine, 0, "iron_myr");
    let batch = engine
        .apply_command(0, &begin_cast(cast_spell(slot, vec![])))
        .expect("begin creature cast");
    let pending = batch.legal_by_player[&0]
        .pending_spell_cast
        .as_ref()
        .expect("pending cast metadata");
    assert_eq!(pending.eligible_restricted_mana_group_ids, [group]);
}
