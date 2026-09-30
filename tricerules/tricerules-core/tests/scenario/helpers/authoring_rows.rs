//! Small reviewed scenario families, not a rules DSL. Expectations never come from card effects.
use super::{semantic::*, *};
use std::collections::{BTreeMap, BTreeSet};
use tricerules_cards::authoring_schema::{Row, Surface};
use tricerules_core::{TurnStep, Zone};

fn mana([w, u, b, r, g, c]: [u32; 6]) -> ManaGift {
    ManaGift { w, u, b, r, g, c }
}

pub(crate) fn run_rows(text: &str, mut engine: impl FnMut() -> GameEngine) -> BTreeSet<String> {
    let rows: Vec<Row> = serde_json::from_str(text).expect("valid, explicit scenario rows");
    assert!(!rows.is_empty(), "empty scenario selection");
    let mut exercised = BTreeSet::new();
    for row in rows {
        row.validate().expect("supported scenario row");
        match row {
            Row::Draw {
                card,
                mana: gift,
                surface,
                mode_index,
                mode_id,
                recipient,
                count,
                food,
            } => {
                let surface = match surface {
                    Surface::Mode => DrawSurface::Mode {
                        index: mode_index.expect("mode index"),
                        id: mode_id.as_deref().expect("mode identity"),
                    },
                    Surface::Spell | Surface::Etb => {
                        assert!(
                            mode_index.is_none() && mode_id.is_none(),
                            "mode fields on nonmodal row"
                        );
                        if matches!(surface, Surface::Spell) {
                            DrawSurface::Spell
                        } else {
                            DrawSurface::Etb
                        }
                    }
                };
                exercise_draw_in(
                    engine(),
                    DrawCase {
                        card: &card,
                        mana: mana(gift),
                        surface,
                        recipient,
                        count,
                        food,
                    },
                )
                .require_exercised();
                exercised.insert(card);
            }
            Row::ManaActivation {
                card,
                ability_index,
                produced,
            } => {
                let mut e = engine();
                let source = inject_permanent_on_battlefield(&mut e, 0, &card);
                let command = activate_ability_for(&e, source, ability_index, vec![]);
                reject_unchanged(&mut e, 1, &command);
                accepted(&mut e, 0, &command);
                assert!(e.state.stack.is_empty(), "mana must resolve immediately");
                assert!(e.state.objects[&source].tapped, "tap mana family");
                assert_eq!(pool(&e, 0), produced, "exact mana");
                assert_eq!(pool(&e, 1), [0; 6]);
                reject_unchanged(&mut e, 0, &command);
                assert_object(&e, source, &card, 0, 0, Zone::Battlefield, 0);
                exercised.insert(card);
            }
            Row::Pump {
                card,
                mana: gift,
                ability_index,
                power,
                toughness,
                keywords,
            } => {
                let mut e = engine();
                let victim = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
                let bystander = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
                let illegal = inject_permanent_on_battlefield(&mut e, 1, "forest");
                let source = row_source(&mut e, &card, ability_index);
                let valid = row_command(&e, source, &card, ability_index, target_object(victim));
                let invalid = row_command(&e, source, &card, ability_index, target_object(illegal));
                // Both invalid target and unaffordable payment must be atomic.
                reject_unchanged(&mut e, 0, &valid);
                give_mana(&mut e, 0, mana(gift));
                reject_unchanged(&mut e, 0, &invalid);
                accepted(&mut e, 0, &valid);
                assert_eq!(pool(&e, 0), [0; 6], "exact payment");
                if ability_index.is_some() {
                    assert!(e.state.objects[&source].tapped, "tap activation cost");
                }
                assert_eq!(
                    e.effective_power(victim),
                    Some(2),
                    "effect waits for resolution"
                );
                complete(&mut e, 16, |_| None).require_exercised();
                assert_eq!(e.effective_power(victim), Some(power), "exact pump power");
                assert_eq!(
                    e.effective_toughness(victim),
                    Some(toughness),
                    "exact pump toughness"
                );
                assert_eq!(e.effective_power(bystander), Some(2));
                assert_row_source(&e, source, &card, ability_index);
                for keyword in &keywords {
                    assert!(
                        e.effective_has_keyword(victim, *keyword),
                        "expected pump keyword"
                    );
                    assert!(!e.effective_has_keyword(bystander, *keyword));
                }
                end_active_turn(&mut e, 0);
                assert_eq!(e.effective_power(victim), Some(2), "pump cleanup");
                assert_eq!(e.effective_toughness(victim), Some(2));
                for keyword in keywords {
                    assert!(!e.effective_has_keyword(victim, keyword), "keyword cleanup");
                }
                exercised.insert(card);
            }
            Row::Mill {
                card,
                mana: gift,
                ability_index,
                recipient,
                count,
            } => {
                let mut e = engine();
                let source = row_source(&mut e, &card, ability_index);
                give_mana(&mut e, 0, mana(gift));
                let libraries = [0, 1].map(|p| {
                    e.state.players[p]
                        .library
                        .iter()
                        .copied()
                        .collect::<Vec<_>>()
                });
                let graves = [0, 1].map(|p| e.state.players[p].graveyard.clone());
                let library_cards: BTreeMap<_, _> = libraries
                    .iter()
                    .flatten()
                    .map(|oid| (*oid, e.state.objects[oid].card_id.clone()))
                    .collect();
                assert!(
                    count <= libraries[recipient].len(),
                    "mill fixture library too short"
                );
                let command = row_command(
                    &e,
                    source,
                    &card,
                    ability_index,
                    target_player(recipient as i32),
                );
                let invalid = row_command(&e, source, &card, ability_index, target_player(99));
                reject_unchanged(&mut e, 0, &invalid);
                accepted(&mut e, 0, &command);
                assert_eq!(pool(&e, 0), [0; 6], "exact mill payment");
                if ability_index.is_some() {
                    assert!(e.state.objects[&source].tapped, "tap activation cost");
                }
                complete(&mut e, 16, |_| None).require_exercised();
                assert_row_source(&e, source, &card, ability_index);
                for p in 0..2 {
                    let removed = if p == recipient { count } else { 0 };
                    assert_eq!(
                        e.state.players[p]
                            .library
                            .iter()
                            .copied()
                            .collect::<Vec<_>>(),
                        libraries[p][removed..],
                        "exact mill library"
                    );
                    let expected = &libraries[p][..removed];
                    for oid in expected {
                        assert_object(
                            &e,
                            *oid,
                            &library_cards[oid],
                            p as i32,
                            p as i32,
                            Zone::Graveyard,
                            1,
                        );
                    }
                    let actual: BTreeSet<_> = e.state.players[p]
                        .graveyard
                        .iter()
                        .copied()
                        .filter(|oid| *oid != source)
                        .collect();
                    let expected: BTreeSet<_> = graves[p].iter().chain(expected).copied().collect();
                    assert_eq!(actual, expected, "exact mill graveyard");
                }
                exercised.insert(card);
            }
            Row::UpkeepDamage {
                card,
                hand_at_trigger,
                hand_at_resolution,
                triggers,
                damage,
            } => {
                let mut e = engine();
                inject_permanent_on_battlefield(&mut e, 0, &card);
                set_hand_size(&mut e, 1, hand_at_trigger);
                end_active_turn(&mut e, 0);
                assert_eq!(e.state.active_player_id(), 1);
                assert_eq!(e.state.turn_step, TurnStep::Upkeep);
                assert_eq!(e.state.stack.len(), triggers, "exact upkeep trigger count");
                set_hand_size(&mut e, 1, hand_at_resolution);
                complete(&mut e, 16, |_| None).require_exercised();
                assert_eq!(e.state.players[0].life, 20, "upkeep controller unaffected");
                assert_eq!(e.state.players[1].life, 20 - damage, "exact upkeep damage");
                for _ in 0..8 {
                    if e.state.turn_step == TurnStep::Main1 {
                        break;
                    }
                    let player = e.state.priority_player_id();
                    accepted(&mut e, player, &pass());
                }
                assert_eq!(
                    e.state.turn_step,
                    TurnStep::Main1,
                    "upkeep fixture advance bound"
                );
                end_active_turn(&mut e, 1);
                assert_eq!(e.state.active_player_id(), 0);
                assert_eq!(e.state.turn_step, TurnStep::Upkeep);
                assert!(
                    e.state.stack.is_empty(),
                    "controller upkeep must not trigger opponent observer"
                );
                exercised.insert(card);
            }
            Row::GraveyardRecovery {
                card,
                mana: gift,
                ability_index,
                target,
                sacrifice_source,
            } => {
                let mut e = engine();
                let source = row_source(&mut e, &card, ability_index);
                let victim = inject_graveyard_card(&mut e, 0, &target);
                let opponent = inject_graveyard_card(&mut e, 1, &target);
                let bystander = inject_graveyard_card(&mut e, 0, &target);
                let command = row_command(&e, source, &card, ability_index, target_object(victim));
                reject_unchanged(&mut e, 0, &command);
                give_mana(&mut e, 0, mana(gift));
                let invalid =
                    row_command(&e, source, &card, ability_index, target_object(opponent));
                reject_unchanged(&mut e, 0, &invalid);
                if sacrifice_source {
                    let invalid =
                        row_command(&e, source, &card, ability_index, target_object(source));
                    reject_unchanged(&mut e, 0, &invalid);
                }
                let hand = e.state.players[0].hand.len() - usize::from(ability_index.is_none());
                accepted(&mut e, 0, &command);
                assert_eq!(pool(&e, 0), [0; 6], "exact recovery payment");
                complete(&mut e, 16, |_| None).require_exercised();
                assert_object(&e, victim, &target, 0, 0, Zone::Hand, 1);
                assert_object(&e, opponent, &target, 1, 1, Zone::Graveyard, 0);
                assert_object(&e, bystander, &target, 0, 0, Zone::Graveyard, 0);
                assert_eq!(
                    e.state.players[0].hand.len(),
                    hand + 1,
                    "exact recovery hand"
                );
                if sacrifice_source || ability_index.is_none() {
                    assert_object(
                        &e,
                        source,
                        &card,
                        0,
                        0,
                        Zone::Graveyard,
                        if ability_index.is_some() { 1 } else { 2 },
                    );
                } else {
                    assert_object(&e, source, &card, 0, 0, Zone::Battlefield, 0);
                }
                exercised.insert(card);
            }
            Row::SkullclampAttachedCreatureDiesDraw => {
                let card = "skullclamp";
                let ability_index = 0;
                let spell_mana = [0, 0, 0, 0, 0, 1];
                let gift = [0, 0, 0, 0, 0, 1];
                let power_delta = 1;
                let toughness_delta = -1;
                let draws = 2;
                let mut e = engine();
                let source = inject_card_into_hand(&mut e, 0, card);
                give_mana(&mut e, 0, mana(spell_mana));
                let slot = hand_index_for_card(&e, 0, card);
                accepted(&mut e, 0, &cast_spell(slot, vec![]));
                assert_eq!(pool(&e, 0), [0; 6], "exact equipment spell payment");
                complete(&mut e, 8, |_| None).require_exercised();
                assert_object(&e, source, card, 0, 0, Zone::Battlefield, 2);
                let equipped = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
                let opponent_creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
                assert_eq!(pool(&e, 0), [0; 6], "equipment fixture starts unfunded");
                assert!(
                    authoring_actions::activation(&mut e, 0, source, ability_index).is_err(),
                    "unaffordable equip must not be offered as a usable activation"
                );
                give_mana(&mut e, 0, mana(gift));
                let command = authoring_actions::activation(&mut e, 0, source, ability_index)
                    .expect("engine-offered equip activation");
                let Cmd::ActivateAbility(activation) = command.cmd.as_ref().unwrap() else {
                    panic!("engine offer was not an ability activation")
                };
                assert_eq!(activation.targets.len(), 1, "Equip selects one creature");
                assert_eq!(
                    activation.targets[0].object_id, equipped,
                    "Equip targets the controller's creature"
                );
                let mut invalid = command.clone();
                let Cmd::ActivateAbility(invalid_activation) = invalid.cmd.as_mut().unwrap() else {
                    unreachable!()
                };
                invalid_activation.targets = target_object(opponent_creature);
                reject_unchanged(&mut e, 0, &invalid);

                accepted(&mut e, 0, &command);
                assert_eq!(pool(&e, 0), [0; 6], "exact equip payment");
                pass_both_players(&mut e);
                assert_eq!(e.state.stack.len(), 0, "Equip resolved");
                assert_eq!(
                    e.state.objects[&source].attached_to,
                    Some(tricerules_core::AttachmentRecipient::Object(equipped))
                );
                assert_eq!(
                    e.effective_power(equipped),
                    Some(u32::try_from(2i32 + power_delta).expect("nonnegative fixture power")),
                    "exact attached power"
                );
                assert_eq!(
                    e.effective_toughness(equipped),
                    Some(
                        u32::try_from(2i32 + toughness_delta)
                            .expect("nonnegative fixture toughness")
                    ),
                    "exact attached toughness"
                );

                let controller_hand = e.state.players[0].hand.len();
                let opponent_hand = e.state.players[1].hand.len();
                let life = e.state.players.iter().map(|p| p.life).collect::<Vec<_>>();
                e.state.objects.get_mut(&equipped).unwrap().damage = 1;
                pass_both_players(&mut e);
                assert_eq!(e.state.objects[&equipped].zone, Zone::Graveyard);
                assert_eq!(e.state.objects[&source].zone, Zone::Battlefield);
                assert_eq!(e.state.objects[&source].attached_to, None);
                assert_eq!(
                    e.state.stack.len(),
                    1,
                    "attached-object dies trigger uses LKI"
                );
                complete(&mut e, 8, |_| None).require_exercised();
                assert_eq!(e.state.players[0].hand.len(), controller_hand + draws);
                assert_eq!(e.state.players[1].hand.len(), opponent_hand);
                assert_eq!(
                    e.state.players.iter().map(|p| p.life).collect::<Vec<_>>(),
                    life,
                    "draw trigger does not change life"
                );
                exercised.insert(card.to_owned());
            }
            Row::Destroy {
                card,
                mana: gift,
                target,
                illegal_target,
            } => {
                let mut e = engine();
                let source = inject_card_into_hand(&mut e, 0, &card);
                let victim = inject_permanent_on_battlefield(&mut e, 1, &target);
                let illegal = inject_permanent_on_battlefield(&mut e, 1, &illegal_target);
                let bystander = inject_permanent_on_battlefield(&mut e, 0, &target);
                give_mana(&mut e, 0, mana(gift));
                let slot = hand_index_for_card(&e, 0, &card);
                let before = format!("{:?}", e.state);
                assert!(
                    e.apply_command(0, &cast_spell(slot, target_object(illegal)))
                        .is_err(),
                    "illegal target accepted"
                );
                assert_eq!(
                    format!("{:?}", e.state),
                    before,
                    "rejected cast changed state"
                );
                accepted(&mut e, 0, &cast_spell(slot, target_object(victim)));
                assert_object(&e, source, &card, 0, 0, Zone::Stack, 1);
                complete(&mut e, 16, |_| None).require_exercised();
                assert_object(&e, source, &card, 0, 0, Zone::Graveyard, 2);
                assert_object(&e, victim, &target, 1, 1, Zone::Graveyard, 1);
                assert_object(&e, illegal, &illegal_target, 1, 1, Zone::Battlefield, 0);
                assert_object(&e, bystander, &target, 0, 0, Zone::Battlefield, 0);
                assert_eq!(
                    e.state.players.iter().map(|p| p.life).collect::<Vec<_>>(),
                    vec![20, 20]
                );
                assert_main_priority(&e, 0);
                exercised.insert(card);
            }
        }
    }
    exercised
}

fn assert_row_source(e: &GameEngine, source: u32, card: &str, ability: Option<u32>) {
    let (zone, generation) = if ability.is_some() {
        (Zone::Battlefield, 0)
    } else {
        (Zone::Graveyard, 2)
    };
    assert_object(e, source, card, 0, 0, zone, generation);
}

fn pool(e: &GameEngine, player: usize) -> [u32; 6] {
    let p = &e.state.players[player].mana_pool;
    [p.white, p.blue, p.black, p.red, p.green, p.colorless]
}

fn reject_unchanged(e: &mut GameEngine, player: i32, command: &RuledCommand) {
    let before = format!("{:?}", e.state);
    assert!(
        e.apply_command(player, command).is_err(),
        "expected rejected command"
    );
    assert_eq!(
        format!("{:?}", e.state),
        before,
        "rejected command changed state"
    );
}

fn row_source(e: &mut GameEngine, card: &str, ability: Option<u32>) -> u32 {
    if ability.is_some() {
        inject_permanent_on_battlefield(e, 0, card)
    } else {
        inject_card_into_hand(e, 0, card)
    }
}

fn row_command(
    e: &GameEngine,
    source: u32,
    card: &str,
    ability: Option<u32>,
    targets: Vec<TargetRef>,
) -> RuledCommand {
    match ability {
        Some(index) => activate_ability_for(e, source, index, targets),
        None => cast_spell(hand_index_for_card(e, 0, card), targets),
    }
}

fn set_hand_size(e: &mut GameEngine, player: usize, count: usize) {
    while e.state.players[player].hand.len() > count {
        let oid = e.state.players[player].hand.pop().unwrap();
        e.state.objects.get_mut(&oid).unwrap().zone = Zone::Graveyard;
        e.state.players[player].graveyard.push(oid);
    }
    while e.state.players[player].hand.len() < count {
        inject_card_into_hand(e, player, "forest");
    }
}
