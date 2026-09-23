//! Direct-RON conformance for the five complete Standard unlocks from issue #341.
//!
//! Oracle identities were matched to the pinned Scryfall snapshot and rechecked against current
//! card records/rulings. Ballyrush Banneret's ruling confirms a Kithkin Soldier matches once;
//! Dragonlord's Servant and Geyser Drake reduce only generic mana and do not change mana value;
//! Uncle Iroh's Firebending mana uses the stack and lasts through end of combat. CR 105.2,
//! 118.7/118.7a, 601.2f, 611.3, and 702.73a govern the color, cost, static, and Changeling cases.

use tricerules_cards::primitives::{
    Amount, CardTypeFilter, GameCondition, ManaAmount, ManaRetention, RelativePlayerSet,
    SpellCostFilter, SpellEffectKind, StaticAbilityDef, TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, Color, Keyword, Layout};

#[test]
fn static_spell_cost_filter_accepts_distinct_subtype_alternatives() {
    let card = r#"(id: "reducer_source_probe", name: "Reducer Source Probe", face_id: "reducer_source_probe",
        types: ["Creature", "Kithkin", "Soldier"], power: 2, toughness: 2,
        static_abilities: [(ability_id: "static_01", presentation: Fallback,
            definition: SpellGenericReduction(casters: Controller,
                spell_filter: Some((any_of: Some([
                    (required_subtypes: ["Kithkin"]),
                    (required_subtypes: ["Soldier"]),
                ]))), amount: 1))])"#;
    let definition: StaticAbilityDef = ron::from_str(
        r#"SpellGenericReduction(casters: Controller,
            spell_filter: Some((any_of: Some([
                (required_subtypes: ["Kithkin"]),
                (required_subtypes: ["Soldier"]),
            ]))), amount: 1)"#,
    )
    .expect("the static reduction schema should parse a subtype filter");
    let roundtrip = ron::to_string(&definition).expect("the typed filter should serialize");
    assert!(
        roundtrip.contains("spell_filter"),
        "the filter must be retained as typed ability data instead of silently discarded"
    );

    let StaticAbilityDef::SpellGenericReduction {
        casters,
        spell_filter,
        ..
    } = &definition
    else {
        panic!("expected a source-owned spell-cost reduction");
    };
    assert_eq!(*casters, RelativePlayerSet::Controller);
    assert_eq!(
        spell_filter.as_ref(),
        Some(&SpellCostFilter {
            any_of: Some(vec![
                SpellCostFilter {
                    required_subtypes: vec!["Kithkin".into()],
                    ..Default::default()
                },
                SpellCostFilter {
                    required_subtypes: vec!["Soldier".into()],
                    ..Default::default()
                },
            ]),
            ..Default::default()
        })
    );

    CardRegistry::from_chunks_and_tokens(&[card], &[])
        .expect("the registry should accept a validated type/subtype/color cost filter");
}

#[test]
fn static_spell_cost_filter_rejects_empty_and_duplicate_alternatives() {
    for filter in [
        "Some(())",
        "Some((required_subtypes: [\"\"]))",
        "Some((any_of: Some([(required_subtypes: [\"Dragon\"]), (required_subtypes: [\"Dragon\"])])))",
    ] {
        let card = format!(
            r#"(id: "reducer_source_probe", name: "Reducer Source Probe", face_id: "reducer_source_probe",
                types: ["Creature"], power: 2, toughness: 2,
                static_abilities: [(ability_id: "static_01", presentation: Fallback,
                    definition: SpellGenericReduction(casters: Controller, spell_filter: {filter}, amount: 1))])"#
        );
        assert!(
            CardRegistry::from_chunks_and_tokens(&[&card], &[]).is_err(),
            "invalid filter must be rejected: {filter}"
        );
    }
}
struct ReviewedIdentity {
    id: &'static str,
    name: &'static str,
    face_id: &'static str,
    cost: &'static str,
    types: &'static [&'static str],
    power_toughness: Option<(u32, u32)>,
}

const REVIEWED_IDENTITIES: [ReviewedIdentity; 5] = [
    ReviewedIdentity {
        id: "ballyrush_banneret",
        name: "Ballyrush Banneret",
        face_id: "ballyrush_banneret",
        cost: "{1}{W}",
        types: &["Creature", "Kithkin", "Soldier"],
        power_toughness: Some((2, 1)),
    },
    ReviewedIdentity {
        id: "dragonlords_servant",
        name: "Dragonlord's Servant",
        face_id: "dragonlords_servant",
        cost: "{1}{R}",
        types: &["Creature", "Goblin", "Shaman"],
        power_toughness: Some((1, 3)),
    },
    ReviewedIdentity {
        id: "geyser_drake",
        name: "Geyser Drake",
        face_id: "geyser_drake",
        cost: "{2}{U}",
        types: &["Creature", "Drake"],
        power_toughness: Some((2, 3)),
    },
    ReviewedIdentity {
        id: "voyager_quickwelder",
        name: "Voyager Quickwelder",
        face_id: "voyager_quickwelder",
        cost: "{2}{W}",
        types: &["Artifact", "Creature", "Robot", "Artificer"],
        power_toughness: Some((2, 4)),
    },
    ReviewedIdentity {
        id: "uncle_iroh",
        name: "Uncle Iroh",
        face_id: "uncle_iroh",
        cost: "{1}{R/G}{R/G}",
        types: &["Creature", "Human", "Noble", "Ally"],
        power_toughness: Some((4, 2)),
    },
];

fn filter_subtype(subtype: &str) -> SpellCostFilter {
    SpellCostFilter {
        required_subtypes: vec![subtype.to_string()],
        ..Default::default()
    }
}

fn reduction(
    filter: Option<SpellCostFilter>,
    condition: Option<GameCondition>,
) -> StaticAbilityDef {
    StaticAbilityDef::SpellGenericReduction {
        casters: RelativePlayerSet::Controller,
        spell_filter: filter,
        amount: Amount::Fixed(1),
        condition,
    }
}

#[test]
fn issue_341_batch_maps_definitions() {
    let registry = CardRegistry::global();
    for ReviewedIdentity {
        id,
        name,
        face_id,
        cost,
        types,
        power_toughness,
    } in REVIEWED_IDENTITIES
    {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.name, name, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id} name binding");
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(definition.face_count(), 1, "{id}");
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), face_id, "{id}");
        assert_eq!(face.mana_cost.to_string(), cost, "{id}");
        assert_eq!(
            face.types.iter().map(String::as_str).collect::<Vec<_>>(),
            types,
            "{id} type line"
        );
        assert_eq!(
            (face.power.zip(face.toughness)),
            power_toughness,
            "{id} P/T"
        );
        assert!(face.spell_effect.is_empty(), "{id}");
        assert!(face.activated_abilities.is_empty(), "{id}");
        assert!(face.custom_effect.is_none(), "{id}");
        assert!(face.modal_spell.is_none(), "{id}");
    }

    let banneret = registry.get("ballyrush_banneret").unwrap().primary_face();
    let [ability] = banneret.static_abilities.as_slice() else {
        panic!("Ballyrush Banneret must have exactly one reducer");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.definition,
        reduction(
            Some(SpellCostFilter {
                any_of: Some(vec![filter_subtype("Kithkin"), filter_subtype("Soldier")]),
                ..Default::default()
            }),
            None,
        )
    );
    assert!(banneret.triggered_abilities.is_empty());

    let servant = registry.get("dragonlords_servant").unwrap().primary_face();
    let [ability] = servant.static_abilities.as_slice() else {
        panic!("Dragonlord's Servant must have exactly one reducer");
    };
    assert_eq!(
        ability.definition,
        reduction(Some(filter_subtype("Dragon")), None)
    );
    assert!(servant.triggered_abilities.is_empty());

    let geyser = registry.get("geyser_drake").unwrap().primary_face();
    assert_eq!(geyser.keywords, [Keyword::Flying]);
    let [ability] = geyser.static_abilities.as_slice() else {
        panic!("Geyser Drake must have exactly one reducer");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.definition,
        reduction(
            None,
            Some(GameCondition::ActivePlayer {
                players: RelativePlayerSet::Opponents,
            }),
        )
    );

    let quickwelder = registry.get("voyager_quickwelder").unwrap().primary_face();
    let [ability] = quickwelder.static_abilities.as_slice() else {
        panic!("Voyager Quickwelder must have exactly one reducer");
    };
    assert_eq!(
        ability.definition,
        reduction(
            Some(SpellCostFilter {
                card_type: Some(CardTypeFilter::Artifact),
                ..Default::default()
            }),
            None,
        )
    );

    let iroh = registry.get("uncle_iroh").unwrap().primary_face();
    assert_eq!(iroh.supertypes, ["Legendary"]);
    let colors = iroh.colors();
    assert_eq!(colors.len(), 2);
    assert!(colors.contains(&Color::Red) && colors.contains(&Color::Green));
    let [ability] = iroh.static_abilities.as_slice() else {
        panic!("Uncle Iroh must have exactly one reducer");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.definition,
        reduction(Some(filter_subtype("Lesson")), None)
    );
    let [trigger] = iroh.triggered_abilities.as_slice() else {
        panic!("Uncle Iroh must have exactly one Firebending trigger");
    };
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        trigger.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0,
        }
    );
    assert_eq!(
        trigger.effect,
        [SpellEffectKind::AddMana {
            amount: ManaAmount {
                r: 1,
                ..Default::default()
            },
            retention: ManaRetention::EndOfCombat,
        }]
    );
}

#[test]
fn issue_341_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    let registry = CardRegistry::global();
    let expected_fingerprints = [
        (
            "ballyrush_banneret",
            "3e0d5aa8c09d8e02d7b155d66a9a9bfcfc17a4c0964e1ce391d8fe5c266cded8",
        ),
        (
            "dragonlords_servant",
            "d6a2ee457360f91d74f7366578d62f85e2729864cb155cd939e2e8f23e58d476",
        ),
        (
            "geyser_drake",
            "fadd4caf93d94217bea52794e7e391f0865c32a9eae30f16dd5981d4847cbd33",
        ),
        (
            "voyager_quickwelder",
            "4ec5b9e9396460eb97813593ad5a7fa958d8c029e05b3f7cd14360c0e629408f",
        ),
        (
            "uncle_iroh",
            "4abb64abe75aea1564f91fff1fd116318df1fb79572d46c6696da7084ed0157f",
        ),
    ];
    for (identity, (expected_id, expected_hash)) in
        REVIEWED_IDENTITIES.into_iter().zip(expected_fingerprints)
    {
        let ReviewedIdentity {
            id, name, face_id, ..
        } = identity;
        assert_eq!(id, expected_id);
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing pinned fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], face_id);
        assert_eq!(fields[3], name);
        assert_eq!(fields[4], expected_hash, "pinned SHA-256 for {id}");

        let metadata = registry
            .presentation_face(id, face_id)
            .unwrap_or_else(|| panic!("missing presentation metadata for {id}"));
        assert_eq!(metadata.card_name, name);
        assert_eq!(metadata.face_name, name);
        assert_eq!(metadata.oracle_text_sha256, fields[4]);
    }
}
