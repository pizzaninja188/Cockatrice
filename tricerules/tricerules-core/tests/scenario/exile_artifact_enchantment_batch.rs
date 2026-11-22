//! Actual-card scenarios for the reviewed `Exile target artifact or enchantment.` family.
//!
//! Expectations are printed Oracle values. CR 115.1a and 601.2c require one
//! artifact-or-enchantment target as the spell is cast; CR 608.2b revalidates it;
//! CR 701.13 moves the permanent to its owner's exile. Artifact and enchantment
//! creatures remain legal targets because the printed filter has no creature
//! exclusion.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "revoke_existence",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "fate_forgotten",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "fade_into_antiquity",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "altars_light",
            mana: ManaGift {
                w: 2,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "ironwrights_cleansing",
            mana: ManaGift {
                w: 2,
                c: 2,
                ..Default::default()
            },
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

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[card]), deck_with("forest", &[])]);
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
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn cast_at(e: &mut GameEngine, case: &Case, target: u32) -> u32 {
    ensure_in_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("{}: cast: {error}", case.id));
    assert_eq!(e.state.stack.len(), 1, "{}: spell is on the stack", case.id);
    e.state.stack[0].id
}

#[test]
fn exile_artifact_enchantment_batch_cards_apply_the_printed_effect() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(5_030_000 + index as u64, case.id);
        let artifact_creature = inject_permanent_on_battlefield(&mut e, 1, "ornithopter");
        cast_at(&mut e, &case, artifact_creature);
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&artifact_creature].zone,
            Zone::Exile,
            "{}: an artifact creature is a legal artifact target",
            case.id
        );
        assert!(
            e.state.players[1].exile.contains(&artifact_creature),
            "{}: the permanent is exiled to its owner's exile",
            case.id
        );
        assert!(
            !e.state.players[1].graveyard.contains(&artifact_creature),
            "{}: exile is not destruction",
            case.id
        );

        let mut e = engine(5_030_500 + index as u64, case.id);
        let enchantment = inject_permanent_on_battlefield(&mut e, 1, "crusade");
        cast_at(&mut e, &case, enchantment);
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&enchantment].zone,
            Zone::Exile,
            "{}: an enchantment is a legal target",
            case.id
        );
        assert!(
            e.state.players[1].exile.contains(&enchantment),
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn exile_artifact_enchantment_batch_cards_reject_illegal_targets() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(5_031_000 + index as u64, case.id);
        let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        ensure_in_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let before = pool(&e);
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
                .is_err(),
            "{}: a plain creature is not an artifact or enchantment",
            case.id
        );
        assert_eq!(
            pool(&e),
            before,
            "{}: rejected cast spends no mana",
            case.id
        );
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn exile_artifact_enchantment_batch_cards_handle_stale_or_expired_state() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(5_032_000 + index as u64, case.id);
        let artifact = inject_permanent_on_battlefield(&mut e, 1, "bonesplitter");
        let spell = cast_at(&mut e, &case, artifact);
        e.state.players[1]
            .battlefield
            .retain(|oid| *oid != artifact);
        e.state.objects.get_mut(&artifact).expect("artifact").zone = Zone::Graveyard;
        e.state.players[1].graveyard.push(artifact);
        *e.state.zone_change_generation.entry(artifact).or_insert(0) += 1;
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&artifact].zone,
            Zone::Graveyard,
            "{}: a stale artifact stays where it moved",
            case.id
        );
        assert!(
            !e.state.players[1].exile.contains(&artifact),
            "{}: a stale target is not exiled",
            case.id
        );
        assert_eq!(
            e.state.objects[&spell].zone,
            Zone::Graveyard,
            "{}: the spell did not resolve",
            case.id
        );
        assert!(e.state.players[0].graveyard.contains(&spell), "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
