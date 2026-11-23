//! Registry conformance for the queue-v62 batch A cohorts.
//!
//! Each included card is a normal-layout single-face, Commander-legal instant or sorcery whose
//! typed definition is one of three reviewed shapes: one creature shrink with one printed rider,
//! one filtered creature destruction with an optional cycling rider, or one creature destruction
//! followed by surveil or scry. Each fingerprint is SHA-256 of the exact pinned normalized Oracle
//! text.

use tricerules_cards::primitives::{
    AbilityCost, AbilitySourceZone, Amount, EffectSubject, LibraryPartitionKind, LifeAmount,
    PowerComparison, SpellEffectKind, TargetFilter, TargetKind,
};
use tricerules_cards::{registry, CardFace, Layout};

fn face(id: &str, name: &str, mana: &str, types: &[&str]) -> &'static CardFace {
    let definition = registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"));
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    assert_eq!(registry::global().id_for_name(name), Some(id), "{id}");
    let f = definition.primary_face();
    assert_eq!(f.face_id.as_str(), id.replace(',', ""), "{id} face id");
    assert_eq!(f.name, name, "{id} name");
    assert_eq!(f.mana_cost.to_string(), mana, "{id} mana");
    assert_eq!(
        f.types.iter().map(String::as_str).collect::<Vec<_>>(),
        types,
        "{id} types"
    );
    f
}

fn chosen_creature<'a>(subject: &'a EffectSubject, id: &str) -> &'a TargetFilter {
    let EffectSubject::Chosen(filter) = subject else {
        panic!("{id}: expected a chosen subject, got {subject:?}");
    };
    assert_eq!(filter.kind, TargetKind::Creature, "{id}");
    filter
}

#[test]
fn q62_shrink_map_definitions() {
    for (id, name, mana, power, toughness) in [
        (
            "constricting_tendrils",
            "Constricting Tendrils",
            "{U}",
            -3,
            0,
        ),
        ("dazzling_lights", "Dazzling Lights", "{U}", -3, 0),
        ("jaces_scrutiny", "Jace's Scrutiny", "{1}{U}", -4, 0),
        ("ulcerate", "Ulcerate", "{B}", -3, -3),
        ("fungal_infection", "Fungal Infection", "{B}", -1, -1),
    ] {
        let f = face(id, name, mana, &["Instant"]);
        let SpellEffectKind::PumpTarget {
            power: actual_power,
            toughness: actual_toughness,
            subject,
            ..
        } = &f.spell_effect[0]
        else {
            panic!("{id}: first effect must shrink: {:?}", f.spell_effect);
        };
        assert_eq!(
            (*actual_power, *actual_toughness),
            (power, toughness),
            "{id}"
        );
        chosen_creature(subject, id);
        let groups = &f.targeting.as_ref().expect("targeting").groups;
        assert_eq!(groups.len(), 1, "{id}");
        assert_eq!((groups[0].min, groups[0].max), (1, 1), "{id}");
        assert_eq!(groups[0].effect_indices, [0], "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        match id {
            "constricting_tendrils" => {
                assert_eq!(f.spell_effect.len(), 1, "{id}");
                let [cycling] = f.activated_abilities.as_slice() else {
                    panic!("{id}: exactly one cycling ability");
                };
                assert_eq!(cycling.source_zone, AbilitySourceZone::Hand, "{id}");
                assert!(matches!(
                    cycling.costs.as_slice(),
                    [AbilityCost::Mana(cost), AbilityCost::DiscardSelf]
                        if cost.to_string() == "{2}"
                ));
                assert!(matches!(
                    cycling.effect.as_slice(),
                    [SpellEffectKind::Draw {
                        count: Amount::Fixed(1),
                        ..
                    }]
                ));
            }
            "dazzling_lights" => assert!(matches!(
                f.spell_effect[1],
                SpellEffectKind::LibraryPartition {
                    count: 2,
                    kind: LibraryPartitionKind::Surveil,
                    ..
                }
            )),
            "jaces_scrutiny" => assert!(matches!(
                &f.spell_effect[1],
                SpellEffectKind::CreateTokens {
                    token,
                    count: Amount::Fixed(1),
                    ..
                } if token == "clue"
            )),
            "ulcerate" => assert!(matches!(
                f.spell_effect[1],
                SpellEffectKind::LoseLife {
                    amount: LifeAmount::Fixed(3),
                    ..
                }
            )),
            "fungal_infection" => assert!(matches!(
                &f.spell_effect[1],
                SpellEffectKind::CreateTokens {
                    token,
                    count: Amount::Fixed(1),
                    ..
                } if token == "saproling_g_1_1"
            )),
            _ => unreachable!(),
        }
        if id != "constricting_tendrils" {
            assert_eq!(f.spell_effect.len(), 2, "{id}");
        }
    }
}

#[test]
fn q62_destroy_filtered_map_definitions() {
    for (id, name, mana, prevention, power, max_mana_value) in [
        ("easy_prey", "Easy Prey", "{1}{B}", false, None, Some(2u32)),
        ("swat", "Swat", "{1}{B}{B}", false, Some(2), None),
        ("reprisal", "Reprisal", "{1}{W}", true, Some(4), None),
        ("smother", "Smother", "{1}{B}", true, None, Some(3)),
        (
            "radiants_judgment",
            "Radiant's Judgment",
            "{2}{W}",
            false,
            Some(4),
            None,
        ),
    ] {
        let f = face(id, name, mana, &["Instant"]);
        let [effect] = f.spell_effect.as_slice() else {
            panic!("{id}: exactly one destruction effect");
        };
        let subject = match (prevention, effect) {
            (false, SpellEffectKind::Destroy { subject }) => subject,
            (true, SpellEffectKind::DestroyPreventingRegeneration { subject }) => subject,
            _ => panic!("{id}: wrong destruction primitive: {effect:?}"),
        };
        let filter = chosen_creature(subject, id);
        let expected_power = power.map(|value| {
            if id == "swat" {
                PowerComparison::AtMost(value)
            } else {
                PowerComparison::AtLeast(value)
            }
        });
        assert_eq!(filter.power, expected_power, "{id}");
        assert_eq!(filter.max_mana_value, max_mana_value, "{id}");
        let groups = &f.targeting.as_ref().expect("targeting").groups;
        assert_eq!(groups.len(), 1, "{id}");
        assert_eq!((groups[0].min, groups[0].max), (1, 1), "{id}");
        assert_eq!(groups[0].effect_indices, [0], "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        let cycling = matches!(id, "easy_prey" | "swat" | "radiants_judgment");
        if cycling {
            let [ability] = f.activated_abilities.as_slice() else {
                panic!("{id}: exactly one cycling ability");
            };
            assert_eq!(ability.source_zone, AbilitySourceZone::Hand, "{id}");
            assert!(matches!(
                ability.costs.as_slice(),
                [AbilityCost::Mana(cost), AbilityCost::DiscardSelf]
                    if cost.to_string() == "{2}"
            ));
            assert!(matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::Draw {
                    count: Amount::Fixed(1),
                    ..
                }]
            ));
        } else {
            assert!(f.activated_abilities.is_empty(), "{id}");
        }
    }
}

#[test]
fn q62_destroy_surveil_scry_map_definitions() {
    for (id, name, mana, types, count, surveil) in [
        (
            "hes_dead,_jim",
            "He's Dead, Jim",
            "{3}{B}",
            ["Instant"].as_slice(),
            1u32,
            true,
        ),
        (
            "deadly_visit",
            "Deadly Visit",
            "{3}{B}{B}",
            ["Sorcery"].as_slice(),
            2,
            true,
        ),
        (
            "get_the_point",
            "Get the Point",
            "{3}{B}{R}",
            ["Instant"].as_slice(),
            1,
            false,
        ),
    ] {
        let f = face(id, name, mana, types);
        let [SpellEffectKind::Destroy { subject }, library] = f.spell_effect.as_slice() else {
            panic!("{id}: destroy then a library filter: {:?}", f.spell_effect);
        };
        let filter = chosen_creature(subject, id);
        assert_eq!(filter.power, None, "{id}: no power restriction");
        assert_eq!(
            filter.max_mana_value, None,
            "{id}: no mana-value restriction"
        );
        if surveil {
            assert!(matches!(
                library,
                SpellEffectKind::LibraryPartition {
                    count: actual,
                    kind: LibraryPartitionKind::Surveil,
                    ..
                } if *actual == count
            ));
        } else {
            assert!(matches!(
                library,
                SpellEffectKind::Scry {
                    count: Amount::Fixed(actual)
                } if *actual == count
            ));
        }
        let groups = &f.targeting.as_ref().expect("targeting").groups;
        assert_eq!(groups.len(), 1, "{id}");
        assert_eq!((groups[0].min, groups[0].max), (1, 1), "{id}");
        assert_eq!(groups[0].effect_indices, [0], "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
    }
}

#[test]
fn q62_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "constricting_tendrils",
            "Constricting Tendrils",
            "58c7a0de9a29aa433e78d03aa3c67ac293f03f34624bc9f9c7cca7ca6da7f858",
        ),
        (
            "dazzling_lights",
            "Dazzling Lights",
            "0208ba5f1accaf1410a28929778c059cbd79a8663e1af81f94e384c72bf4466a",
        ),
        (
            "jaces_scrutiny",
            "Jace's Scrutiny",
            "142394cd17c0de8d0635f452947b9ea09da73b3a7430cf7c4fd8ac10a1179ec2",
        ),
        (
            "ulcerate",
            "Ulcerate",
            "035bb34fd878b4d34879d17795f0f190ec35fb3357461cb1a7d401ada42067e3",
        ),
        (
            "fungal_infection",
            "Fungal Infection",
            "f192587c1a7927272aaf695ce357af3ad618e4afddb8139ec851c33b6a6634dc",
        ),
        (
            "easy_prey",
            "Easy Prey",
            "94e29fe76f8a4602b99e793b7f4ed9780cbb23b997291f40bad92cbe5fc96e32",
        ),
        (
            "swat",
            "Swat",
            "77c0d2f5381c4bb42ad95c45b7819162d5edf5b53b0677848fdb6e29b15cc3ee",
        ),
        (
            "reprisal",
            "Reprisal",
            "04e5d2b45a77e26a5e80dc3cfb7e7a174ce341244469613f2c71682cb9e48765",
        ),
        (
            "smother",
            "Smother",
            "02e1088e9efdc315f57bfc364b6250626869b6703e4cbca27d9853eb4d80ce1d",
        ),
        (
            "radiants_judgment",
            "Radiant's Judgment",
            "557aecb99b6bb58caa24efcbd8f524a9e8ca362ea6206ae31cd4bb1deec883ea",
        ),
        (
            "hes_dead,_jim",
            "He's Dead, Jim",
            "34fe42b59872632bd105daf4bc0dcb3280a5743591a52c361daf3133bad38578",
        ),
        (
            "deadly_visit",
            "Deadly Visit",
            "fe34a86b694ffe725f24fcd86cdee4a24271793e253d20d44eb21784f3200408",
        ),
        (
            "get_the_point",
            "Get the Point",
            "e7f8c82683f68244a707464566accbe0fbb33afe13f61230f664863483a592bb",
        ),
    ] {
        let row = format!(
            "{id}\t{name}\t{}\t{name}\t{fingerprint}",
            id.replace(',', "")
        );
        assert!(
            fingerprints.lines().any(|line| line == row),
            "{id}: missing pinned fingerprint row"
        );
        assert_eq!(registry::global().id_for_name(name), Some(id), "{id}");
    }
}
