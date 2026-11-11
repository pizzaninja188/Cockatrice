//! Actual-card scenarios for the reviewed fixed-bonus Equipment family.
//!
//! Expectations are the printed Oracle bonus and equip cost, independent of the registry
//! definitions. CR 301.5/702.6 cover Equipment and equip; CR 613.4c applies the attached P/T
//! modification; CR 704.5n detaches an Equipment whose host has left the battlefield; CR 702.6a
//! restricts equip to a sorcery-speed window; and CR 608.2b fizzles an equip whose target left
//! before resolution. Shuko's saved ruling confirms that its zero-cost equip may target its
//! current host repeatedly.
use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    cast: ManaGift,
    equip: ManaGift,
    power: u32,
    toughness: u32,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "+2_mace",
            cast: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            equip: ManaGift {
                c: 3,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
        },
        Case {
            id: "bone_saw",
            cast: ManaGift::default(),
            equip: ManaGift {
                c: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 0,
        },
        Case {
            id: "bronze_sword",
            cast: ManaGift {
                c: 1,
                ..Default::default()
            },
            equip: ManaGift {
                c: 3,
                ..Default::default()
            },
            power: 2,
            toughness: 0,
        },
        Case {
            id: "cultists_staff",
            cast: ManaGift {
                c: 2,
                ..Default::default()
            },
            equip: ManaGift {
                c: 3,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
        },
        Case {
            id: "greataxe",
            cast: ManaGift {
                c: 1,
                ..Default::default()
            },
            equip: ManaGift {
                c: 5,
                ..Default::default()
            },
            power: 4,
            toughness: 0,
        },
        Case {
            id: "greatsword",
            cast: ManaGift {
                c: 3,
                ..Default::default()
            },
            equip: ManaGift {
                c: 3,
                ..Default::default()
            },
            power: 3,
            toughness: 0,
        },
        Case {
            id: "honed_khopesh",
            cast: ManaGift {
                c: 1,
                ..Default::default()
            },
            equip: ManaGift {
                c: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        Case {
            id: "jackhammer",
            cast: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            equip: ManaGift {
                c: 2,
                ..Default::default()
            },
            power: 2,
            toughness: 0,
        },
        Case {
            id: "kite_shield",
            cast: ManaGift::default(),
            equip: ManaGift {
                c: 3,
                ..Default::default()
            },
            power: 0,
            toughness: 3,
        },
        Case {
            id: "leonin_scimitar",
            cast: ManaGift {
                c: 1,
                ..Default::default()
            },
            equip: ManaGift {
                c: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        Case {
            id: "marauders_axe",
            cast: ManaGift {
                c: 2,
                ..Default::default()
            },
            equip: ManaGift {
                c: 2,
                ..Default::default()
            },
            power: 2,
            toughness: 0,
        },
        Case {
            id: "ogres_cleaver",
            cast: ManaGift {
                c: 2,
                ..Default::default()
            },
            equip: ManaGift {
                c: 5,
                ..Default::default()
            },
            power: 5,
            toughness: 0,
        },
        Case {
            id: "riot_gear",
            cast: ManaGift {
                c: 2,
                ..Default::default()
            },
            equip: ManaGift {
                c: 2,
                ..Default::default()
            },
            power: 1,
            toughness: 2,
        },
        Case {
            id: "shuko",
            cast: ManaGift {
                c: 1,
                ..Default::default()
            },
            equip: ManaGift::default(),
            power: 1,
            toughness: 0,
        },
        Case {
            id: "slagwurm_armor",
            cast: ManaGift {
                c: 1,
                ..Default::default()
            },
            equip: ManaGift {
                c: 3,
                ..Default::default()
            },
            power: 0,
            toughness: 6,
        },
        Case {
            id: "torch_gauntlet",
            cast: ManaGift {
                c: 2,
                ..Default::default()
            },
            equip: ManaGift {
                c: 2,
                ..Default::default()
            },
            power: 2,
            toughness: 0,
        },
        Case {
            id: "trusty_machete",
            cast: ManaGift {
                c: 1,
                ..Default::default()
            },
            equip: ManaGift {
                c: 2,
                ..Default::default()
            },
            power: 2,
            toughness: 1,
        },
        Case {
            id: "vanquishers_axe",
            cast: ManaGift {
                c: 1,
                ..Default::default()
            },
            equip: ManaGift {
                c: 2,
                ..Default::default()
            },
            power: 2,
            toughness: 0,
        },
        Case {
            id: "veterans_sidearm",
            cast: ManaGift {
                c: 2,
                ..Default::default()
            },
            equip: ManaGift {
                c: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        Case {
            id: "vulshok_battlegear",
            cast: ManaGift {
                c: 3,
                ..Default::default()
            },
            equip: ManaGift {
                c: 3,
                ..Default::default()
            },
            power: 3,
            toughness: 3,
        },
        Case {
            id: "warlords_axe",
            cast: ManaGift {
                c: 3,
                ..Default::default()
            },
            equip: ManaGift {
                c: 4,
                ..Default::default()
            },
            power: 3,
            toughness: 1,
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let pool = &e.state.players[0].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
}

fn assert_pool_empty(e: &GameEngine, id: &str) {
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{id}: exact cost consumed");
}

/// Rebuild a fresh `ManaGift` from a case field; the shared helper type is intentionally not
/// `Copy`, so a case that pays the same equip cost twice needs an independent gift.
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

/// Inject, cast and resolve one Equipment, returning its ObjectId.
fn cast_equipment(e: &mut GameEngine, id: &str, cast: ManaGift) -> u32 {
    give_mana(e, 0, cast);
    let slot = inject_card_into_hand(e, 0, id);
    let hand_slot = hand_index_for_card(e, 0, id);
    semantic::accepted(e, 0, &cast_spell(hand_slot, vec![]));
    assert_pool_empty(e, id);
    resolve_entire_stack_two_player(e);
    assert_eq!(
        e.state.objects[&slot].zone,
        Zone::Battlefield,
        "{id}: resolves onto the battlefield"
    );
    assert_eq!(
        e.state.objects[&slot].attached_to, None,
        "{id}: enters unattached"
    );
    assert!(
        !e.state.players[0].hand.contains(&slot),
        "{id}: the cast card leaves its owner's hand"
    );
    slot
}

fn equip(e: &mut GameEngine, equipment: u32, creature: u32, mana: ManaGift, id: &str) {
    give_mana(e, 0, mana);
    apply_ability(e, 0, equipment, 0, target_object(creature)).expect("equip");
    pass_both_players(e);
    assert_pool_empty(e, id);
    assert_eq!(
        e.state.objects[&equipment].attached_to,
        Some(AttachmentRecipient::Object(creature)),
        "{id}: equipment attached to the chosen creature"
    );
}

#[test]
fn equipment_batch_cards_grant_exact_printed_bonus_and_follow_reattachment() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(770_000 + case_index as u64);
        let opponent_hand = e.state.players[1].hand.len();
        let opponent_library = e.state.players[1].library.len();
        let owner_graveyard = e.state.players[0].graveyard.len();

        let equipment = cast_equipment(&mut e, case.id, gift(&case.cast));
        let first = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let second = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");

        assert_eq!(
            e.state.players[1].hand.len(),
            opponent_hand,
            "{}: opponent hand is private and unchanged",
            case.id
        );
        assert_eq!(
            e.state.players[1].library.len(),
            opponent_library,
            "{}: opponent library is unchanged",
            case.id
        );
        assert_eq!(
            e.state.players[0].graveyard.len(),
            owner_graveyard,
            "{}: the resolved Equipment is not in the graveyard",
            case.id
        );

        equip(&mut e, equipment, first, gift(&case.equip), case.id);
        assert_eq!(
            e.effective_power(first),
            Some(2 + case.power),
            "{}: exact power bonus",
            case.id
        );
        assert_eq!(
            e.effective_toughness(first),
            Some(2 + case.toughness),
            "{}: exact toughness bonus",
            case.id
        );

        equip(&mut e, equipment, second, gift(&case.equip), case.id);
        assert_eq!(
            e.effective_power(first),
            Some(2),
            "{}: old host loses the power bonus",
            case.id
        );
        assert_eq!(
            e.effective_toughness(first),
            Some(2),
            "{}: old host loses the toughness bonus",
            case.id
        );
        assert_eq!(
            e.effective_power(second),
            Some(2 + case.power),
            "{}: new host gains the power bonus",
            case.id
        );
        assert_eq!(
            e.effective_toughness(second),
            Some(2 + case.toughness),
            "{}: new host gains the toughness bonus",
            case.id
        );
        assert_eq!(
            e.state.objects[&equipment].zone,
            Zone::Battlefield,
            "{}: the Equipment remains on the battlefield",
            case.id
        );
        assert_eq!(
            e.state.players[1].hand.len(),
            opponent_hand,
            "{}: re-equipping does not reveal or move opponent cards",
            case.id
        );
    }
}

#[test]
fn equipment_batch_cards_reject_opponent_creature_targets() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(771_000 + case_index as u64);
        let equipment = cast_equipment(&mut e, case.id, gift(&case.cast));
        let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");

        let ability_key = u64::from(equipment) << 32;
        let legal = e.initial_response_batch();
        let published = &legal.legal_by_player[&0].valid_targets_by_ability[&ability_key];
        assert_eq!(
            published.groups.len(),
            1,
            "{}: one equip target group",
            case.id
        );
        assert!(
            published.groups[0].valid_permanent_ids.contains(&own),
            "{}: a controlled creature is a legal equip target",
            case.id
        );
        assert!(
            !published.groups[0].valid_permanent_ids.contains(&opponent),
            "{}: an opponent's creature is never a legal equip target",
            case.id
        );

        give_mana(&mut e, 0, gift(&case.equip));
        let before = pool(&e);
        assert!(
            apply_ability(&mut e, 0, equipment, 0, target_object(opponent)).is_err(),
            "{}: opponent's creature is not a legal equip target",
            case.id
        );
        assert_eq!(
            pool(&e),
            before,
            "{}: rejected equip spends no mana",
            case.id
        );
        assert_eq!(
            e.state.objects[&equipment].attached_to, None,
            "{}: rejected equip leaves the Equipment unattached",
            case.id
        );
        assert!(
            e.state.stack.is_empty(),
            "{}: rejected equip adds no stack object",
            case.id
        );
    }
}

#[test]
fn equipment_batch_cards_equip_only_at_sorcery_speed() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(772_000 + case_index as u64);
        let equipment = cast_equipment(&mut e, case.id, gift(&case.cast));
        let host = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        end_active_turn(&mut e, 0);
        give_mana(&mut e, 0, gift(&case.equip));
        let before = pool(&e);
        assert!(
            apply_ability(&mut e, 0, equipment, 0, target_object(host)).is_err(),
            "{}: equip is illegal outside a sorcery-speed window",
            case.id
        );
        assert_eq!(
            pool(&e),
            before,
            "{}: rejected equip outside the window spends no mana",
            case.id
        );
        assert_eq!(
            e.state.objects[&equipment].attached_to, None,
            "{}: rejected equip leaves the Equipment unattached",
            case.id
        );
        assert!(
            e.state.stack.is_empty(),
            "{}: rejected equip adds no stack object",
            case.id
        );
    }
}

#[test]
fn equipment_batch_cards_fizzle_when_equip_target_leaves() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(774_000 + case_index as u64);
        let equipment = cast_equipment(&mut e, case.id, gift(&case.cast));
        let host = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        give_mana(&mut e, 0, gift(&case.equip));
        apply_ability(&mut e, 0, equipment, 0, target_object(host)).expect("activate equip");

        // The target leaves the battlefield before the equip ability resolves (CR 608.2b).
        e.state.players[0]
            .battlefield
            .retain(|object| *object != host);
        e.state.players[0].graveyard.push(host);
        e.state.objects.get_mut(&host).expect("host").zone = Zone::Graveyard;
        *e.state.zone_change_generation.entry(host).or_default() += 1;
        resolve_entire_stack_two_player(&mut e);

        assert_eq!(
            e.state.objects[&host].zone,
            Zone::Graveyard,
            "{}: the stale target remains where it moved",
            case.id
        );
        assert_eq!(
            e.state.objects[&equipment].attached_to, None,
            "{}: a fizzled equip does not attach",
            case.id
        );
        assert_eq!(
            e.state.objects[&equipment].zone,
            Zone::Battlefield,
            "{}: the Equipment remains on the battlefield",
            case.id
        );
        assert!(
            e.state.stack.is_empty(),
            "{}: the equip ability has left the stack",
            case.id
        );
    }
}

#[test]
fn equipment_batch_slagwurm_host_departure_keeps_equipment_unattached() {
    let mut e = engine(773_001);
    let equipment = cast_equipment(
        &mut e,
        "slagwurm_armor",
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let host = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    equip(
        &mut e,
        equipment,
        host,
        ManaGift {
            c: 3,
            ..Default::default()
        },
        "slagwurm_armor",
    );
    assert_eq!(e.effective_power(host), Some(2), "exact power is unchanged");
    assert_eq!(e.effective_toughness(host), Some(8), "exact +0/+6 bonus");

    // The attached +0/+6 makes the host 2/8; eight marked damage is lethal.
    e.state.objects.get_mut(&host).expect("host").damage = 8;
    pass_both_players(&mut e);
    assert_eq!(
        e.state.objects[&host].zone,
        Zone::Graveyard,
        "the host is destroyed by lethal damage"
    );
    assert_eq!(
        e.state.objects[&equipment].attached_to, None,
        "the Equipment detaches when its host leaves"
    );
    assert_eq!(
        e.state.objects[&equipment].zone,
        Zone::Battlefield,
        "the Equipment stays on the battlefield"
    );
}

#[test]
fn equipment_batch_shuko_zero_cost_equip_can_target_its_current_host_repeatedly() {
    let mut e = engine(773_002);
    let shuko = cast_equipment(
        &mut e,
        "shuko",
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let host = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    equip(&mut e, shuko, host, ManaGift::default(), "shuko");
    equip(&mut e, shuko, host, ManaGift::default(), "shuko");
    assert_eq!(
        e.state.objects[&shuko].attached_to,
        Some(AttachmentRecipient::Object(host))
    );
    assert_eq!(e.effective_power(host), Some(3));
    assert_eq!(e.effective_toughness(host), Some(2));
}
