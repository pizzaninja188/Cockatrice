//! Actual-card scenarios for the queue-v20 changeling keyword cohort.
//!
//! Expectations are printed Oracle values. CR 702.73 makes changeling a
//! characteristic-defining ability that gives the creature every creature type
//! in every zone; CR 604.3/613 apply the live Goblin lord's continuous effects.
use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    power: u32,
    toughness: u32,
    artifact: bool,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "impostor_of_the_sixth_pride",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            power: 3,
            toughness: 1,
            artifact: false,
        },
        Case {
            id: "universal_automaton",
            mana: ManaGift {
                c: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
            artifact: true,
        },
        Case {
            id: "woodland_changeling",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
            artifact: false,
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn cast_changeling(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(e, 0, gift(&case.mana));
    let slot = inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact full cost", case.id);
    resolve_entire_stack_two_player(e);
    slot
}

#[test]
fn changeling_keyword_batch_cards_cast_with_exact_printed_characteristics() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_400_000 + index as u64);
        let creature = cast_changeling(&mut e, &case);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
            case.id
        );
        let characteristics = e.characteristics(creature).expect("characteristics");
        assert_eq!(
            characteristics.has_type("Artifact"),
            case.artifact,
            "{}: artifact status",
            case.id
        );
        assert!(characteristics.has_type("Creature"), "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn changeling_keyword_batch_cards_have_every_creature_type() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_401_000 + index as u64);
        let creature = cast_changeling(&mut e, &case);
        let characteristics = e.characteristics(creature).expect("characteristics");
        for creature_type in [
            "Goblin",
            "Elf",
            "Dragon",
            "Soldier",
            "Zombie",
            "Merfolk",
            "Beast",
            "Shapeshifter",
        ] {
            assert!(
                characteristics.has_type(creature_type),
                "{}: changeling grants {creature_type}",
                case.id
            );
        }
    }
}

#[test]
fn changeling_keyword_batch_cards_satisfy_a_live_goblin_lord() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_402_000 + index as u64);
        let changeling = cast_changeling(&mut e, &case);
        give_mana(
            &mut e,
            0,
            ManaGift {
                r: 2,
                c: 1,
                ..Default::default()
            },
        );
        let chieftain = inject_card_into_hand(&mut e, 0, "goblin_chieftain");
        let hand_slot = hand_index_for_card(&e, 0, "goblin_chieftain");
        e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
            .expect("cast Goblin Chieftain");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&chieftain].zone,
            tricerules_core::Zone::Battlefield
        );
        assert_eq!(
            (
                e.effective_power(changeling),
                e.effective_toughness(changeling)
            ),
            (Some(case.power + 1), Some(case.toughness + 1)),
            "{}: the Goblin lord pumps the changeling",
            case.id
        );
        assert!(
            e.effective_has_keyword(changeling, Keyword::Haste),
            "{}: the Goblin lord grants haste",
            case.id
        );
    }
}

#[test]
fn changeling_keyword_batch_cards_are_artifacts_only_when_printed() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_403_000 + index as u64);
        let creature = cast_changeling(&mut e, &case);
        let is_artifact = e
            .characteristics(creature)
            .expect("characteristics")
            .has_type("Artifact");
        assert_eq!(
            is_artifact, case.artifact,
            "{}: changeling does not add card types",
            case.id
        );
    }
}

#[test]
fn changeling_keyword_batch_cards_have_every_creature_type_in_every_zone() {
    for (index, case) in cases().into_iter().enumerate() {
        for (zone_index, zone) in [Zone::Hand, Zone::Library, Zone::Graveyard]
            .into_iter()
            .enumerate()
        {
            let mut e = engine(10_404_000 + (index * 10 + zone_index) as u64);
            let object = match zone {
                Zone::Hand => inject_card_into_hand(&mut e, 0, case.id),
                Zone::Library => inject_library_card(&mut e, 0, case.id),
                Zone::Graveyard => inject_graveyard_card(&mut e, 0, case.id),
                other => panic!("unexpected zone {other:?}"),
            };
            assert_eq!(e.state.objects[&object].zone, zone, "{}", case.id);
            let characteristics = e
                .characteristics(object)
                .unwrap_or_else(|| panic!("{}: missing characteristics in {zone:?}", case.id));
            assert_eq!(
                characteristics.has_type("Artifact"),
                case.artifact,
                "{}: card types are stable in {zone:?}",
                case.id
            );
            for creature_type in ["Goblin", "Elf", "Dragon", "Soldier"] {
                assert!(
                    characteristics.has_type(creature_type),
                    "{}: changeling grants {creature_type} in {zone:?}",
                    case.id
                );
            }
        }
    }
}
