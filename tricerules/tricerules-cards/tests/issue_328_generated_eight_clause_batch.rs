//! Registry and presentation conformance for issue #328.
//!
//! The eight exact generator templates must register the reviewed Standard identities with their
//! complete typed payloads: CR 508.1/701.13 the attacking-only exile; CR 118.12a/602.2b/701.21 the
//! sacrifice-as-cost enchantment destruction; CR 503.1/603.2b/120.3 the upkeep controller-relative
//! self-damage; CR 701.25 the tap-for-surveil private partition; CR 611.2a/514.2 the tap-to-grant
//! haste activation; CR 110.4a the permanent-card graveyard return; CR 509.1b/611.2c the bounded
//! can't-block spell; and CR 611.2a/514.2/701.22 the shared-target pump + first strike + scry one.

use tricerules_cards::primitives::{
    CardTypeFilter, CombatRestriction, CombatRestrictionScope, CombatRole, EffectSubject,
    GraveyardDestination, GraveyardFilter, GraveyardOwner, PermanentTypeFilter, PlayerRecipient,
    TargetFilter, TargetGroupDef, TargetKind, TargetingDef, ZoneCardFilter,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, Amount, CardRegistry, CastTriggerPlayer, Color, Keyword,
    Layout, LibraryPartitionKind, SpellEffectKind, TriggerCondition,
};

fn single_group(targeting: &TargetingDef) -> &TargetGroupDef {
    let [group] = targeting.groups.as_slice() else {
        panic!("expected exactly one authored target group");
    };
    group
}

#[test]
fn issue_328_registers_the_eight_reviewed_identities() {
    let registry = CardRegistry::global();
    for (id, name, face_count, layout) in [
        ("not_on_my_watch", "Not on My Watch", 1, Layout::Normal),
        ("felidar_cub", "Felidar Cub", 1, Layout::Normal),
        ("ravenous_giant", "Ravenous Giant", 1, Layout::Normal),
        ("rune-sealed_wall", "Rune-Sealed Wall", 1, Layout::Normal),
        ("axgard_cavalry", "Axgard Cavalry", 1, Layout::Normal),
        ("elvish_regrower", "Elvish Regrower", 1, Layout::Normal),
        (
            "bellowing_bruiser_beat_a_path",
            "Bellowing Bruiser // Beat a Path",
            2,
            Layout::Adventure,
        ),
        ("kindled_heroism", "Kindled Heroism", 1, Layout::Normal),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing reviewed card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        assert_eq!(definition.layout, layout);
        assert_eq!(definition.face_count(), face_count);
    }
}

#[test]
fn issue_328_not_on_my_watch_exiles_only_attackers() {
    let definition = CardRegistry::global()
        .get("not_on_my_watch")
        .expect("Not on My Watch");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "not_on_my_watch");
    assert_eq!(face.mana_cost.to_string(), "{1}{W}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::White]);
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::Exile {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                combat_role: Some(CombatRole::Attacking),
                ..TargetFilter::default()
            })),
        }]
    );
    assert!(face.targeting.is_none());
}

#[test]
fn issue_328_felidar_cub_sacrifices_itself_for_an_enchantment() {
    let definition = CardRegistry::global()
        .get("felidar_cub")
        .expect("Felidar Cub");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "felidar_cub");
    assert_eq!(face.mana_cost.to_string(), "{1}{W}");
    assert_eq!(face.types, ["Creature", "Cat", "Beast"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(2)));
    assert_eq!(face.colors(), vec![Color::White]);

    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Felidar Cub must have exactly one activated ability");
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(ability.costs, vec![AbilityCost::SacrificeSelf]);
    assert_eq!(
        ability.effect,
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::AnyPermanent,
                permanent_types: vec![PermanentTypeFilter::Enchantment],
                ..TargetFilter::default()
            })),
        }]
    );
    assert!(ability.targeting.is_none());
}

#[test]
fn issue_328_ravenous_giant_damages_its_controller_each_upkeep() {
    let definition = CardRegistry::global()
        .get("ravenous_giant")
        .expect("Ravenous Giant");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "ravenous_giant");
    assert_eq!(face.mana_cost.to_string(), "{2}{R}{R}");
    assert_eq!(face.types, ["Creature", "Giant"]);
    assert_eq!((face.power, face.toughness), (Some(5), Some(5)));
    assert_eq!(face.colors(), vec![Color::Red]);

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Ravenous Giant must have exactly one triggered ability");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.trigger,
        TriggerCondition::AtBeginningOfUpkeep {
            player: CastTriggerPlayer::Controller,
        }
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::DamagePlayer {
            amount: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
        }]
    );
    assert!(ability.targeting.is_none());
    assert!(!ability.may);
}

#[test]
fn issue_328_rune_sealed_wall_taps_to_surveil_one() {
    let definition = CardRegistry::global()
        .get("rune-sealed_wall")
        .expect("Rune-Sealed Wall");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "rune_sealed_wall");
    assert_eq!(face.mana_cost.to_string(), "{2}{U}");
    assert_eq!(face.types, ["Artifact", "Creature", "Wall"]);
    assert_eq!((face.power, face.toughness), (Some(0), Some(6)));
    assert_eq!(face.colors(), vec![Color::Blue]);
    assert_eq!(face.keywords, [Keyword::Defender]);

    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Rune-Sealed Wall must have exactly one activated ability");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(ability.costs, vec![AbilityCost::Tap]);
    assert_eq!(
        ability.effect,
        [SpellEffectKind::LibraryPartition {
            count: 1,
            top_min: 0,
            top_max: None,
            kind: LibraryPartitionKind::Surveil,
        }]
    );
    assert!(ability.targeting.is_none());
}

#[test]
fn issue_328_axgard_cavalry_taps_to_grant_haste() {
    let definition = CardRegistry::global()
        .get("axgard_cavalry")
        .expect("Axgard Cavalry");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "axgard_cavalry");
    assert_eq!(face.mana_cost.to_string(), "{1}{R}");
    assert_eq!(face.types, ["Creature", "Dwarf", "Berserker"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(2)));
    assert_eq!(face.colors(), vec![Color::Red]);

    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Axgard Cavalry must have exactly one activated ability");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(ability.costs, vec![AbilityCost::Tap]);
    assert_eq!(
        ability.effect,
        [SpellEffectKind::GrantKeywords {
            subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            keywords: vec![Keyword::Haste],
        }]
    );
    let targeting = ability.targeting.as_ref().expect("Axgard Cavalry targets");
    let group = single_group(targeting);
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target creature");
    assert_eq!(group.effect_indices, [0]);
}

#[test]
fn issue_328_elvish_regrower_returns_a_permanent_card() {
    let definition = CardRegistry::global()
        .get("elvish_regrower")
        .expect("Elvish Regrower");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "elvish_regrower");
    assert_eq!(face.mana_cost.to_string(), "{2}{G}{G}");
    assert_eq!(face.types, ["Creature", "Elf", "Druid"]);
    assert_eq!((face.power, face.toughness), (Some(4), Some(3)));
    assert_eq!(face.colors(), vec![Color::Green]);

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Elvish Regrower must have exactly one triggered ability");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        ability.effect,
        [SpellEffectKind::MoveGraveyardCards {
            filter: GraveyardFilter {
                owner: GraveyardOwner::Controller,
                card: Some(ZoneCardFilter {
                    excluded_card_types: vec![CardTypeFilter::Instant, CardTypeFilter::Sorcery],
                    ..ZoneCardFilter::default()
                }),
                ..GraveyardFilter::default()
            },
            destination: GraveyardDestination::Hand,
            linked_exile_id: None,
        }]
    );
    assert!(ability.targeting.is_none());
}

#[test]
fn issue_328_bellowing_bruiser_is_an_adventure_with_the_cant_block_face() {
    let definition = CardRegistry::global()
        .get("bellowing_bruiser_beat_a_path")
        .expect("Bellowing Bruiser // Beat a Path");
    assert_eq!(definition.layout, Layout::Adventure);
    assert_eq!(definition.face_count(), 2);

    let front = definition.face(0).expect("creature face");
    assert_eq!(front.face_id.as_str(), "bellowing_bruiser");
    assert_eq!(front.name, "Bellowing Bruiser");
    assert_eq!(front.mana_cost.to_string(), "{4}{R}");
    assert_eq!(front.types, ["Creature", "Ogre"]);
    assert_eq!((front.power, front.toughness), (Some(4), Some(4)));
    assert_eq!(front.keywords, [Keyword::Haste]);
    assert!(front.spell_effect.is_empty());

    let adventure = definition.face(1).expect("adventure face");
    assert_eq!(adventure.face_id.as_str(), "beat_a_path");
    assert_eq!(adventure.name, "Beat a Path");
    assert_eq!(adventure.mana_cost.to_string(), "{2}{R}");
    assert_eq!(adventure.types, ["Sorcery", "Adventure"]);
    assert_eq!(
        adventure.spell_effect,
        [SpellEffectKind::ApplyCombatRestriction {
            scope: CombatRestrictionScope::Chosen(TargetFilter::default_creature()),
            restriction: CombatRestriction {
                cant_block: true,
                ..CombatRestriction::default()
            },
        }]
    );
    let targeting = adventure
        .targeting
        .as_ref()
        .expect("Beat a Path targets up to two creatures");
    let group = single_group(targeting);
    assert_eq!((group.min, group.max), (0, 2));
    assert_eq!(group.prompt, "Choose up to two target creatures");
    assert_eq!(group.effect_indices, [0]);
}

#[test]
fn issue_328_kindled_heroism_pumps_grants_first_strike_and_scries() {
    let definition = CardRegistry::global()
        .get("kindled_heroism")
        .expect("Kindled Heroism");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "kindled_heroism");
    assert_eq!(face.mana_cost.to_string(), "{R}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::Red]);
    assert_eq!(
        face.spell_effect,
        [
            SpellEffectKind::PumpTarget {
                power: 1,
                toughness: 0,
                scale: None,
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            },
            SpellEffectKind::GrantKeywords {
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                keywords: vec![Keyword::FirstStrike],
            },
            SpellEffectKind::Scry {
                count: Amount::Fixed(1),
            },
        ]
    );
    let targeting = face
        .targeting
        .as_ref()
        .expect("Kindled Heroism targets a creature");
    let group = single_group(targeting);
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target creature");
    assert_eq!(
        group.effect_indices,
        [0, 1],
        "the shared group covers the pump and keyword grant but not the untargeted scry"
    );
}

#[test]
fn issue_328_fingerprint_rows_match_the_presentation_registry() {
    let registry = CardRegistry::global();
    let cases: &[(&str, &str, &str, &str)] = &[
        (
            "not_on_my_watch",
            "not_on_my_watch",
            "Not on My Watch",
            "Not on My Watch",
        ),
        ("felidar_cub", "felidar_cub", "Felidar Cub", "Felidar Cub"),
        (
            "ravenous_giant",
            "ravenous_giant",
            "Ravenous Giant",
            "Ravenous Giant",
        ),
        (
            "rune-sealed_wall",
            "rune_sealed_wall",
            "Rune-Sealed Wall",
            "Rune-Sealed Wall",
        ),
        (
            "axgard_cavalry",
            "axgard_cavalry",
            "Axgard Cavalry",
            "Axgard Cavalry",
        ),
        (
            "elvish_regrower",
            "elvish_regrower",
            "Elvish Regrower",
            "Elvish Regrower",
        ),
        (
            "bellowing_bruiser_beat_a_path",
            "bellowing_bruiser",
            "Bellowing Bruiser // Beat a Path",
            "Bellowing Bruiser",
        ),
        (
            "bellowing_bruiser_beat_a_path",
            "beat_a_path",
            "Bellowing Bruiser // Beat a Path",
            "Beat a Path",
        ),
        (
            "kindled_heroism",
            "kindled_heroism",
            "Kindled Heroism",
            "Kindled Heroism",
        ),
    ];
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, face_id, card_name, face_name) in cases {
        let presentation = registry
            .presentation_face(id, face_id)
            .unwrap_or_else(|| panic!("missing presentation metadata for {id}/{face_id}"));
        assert_eq!(presentation.card_name, *card_name);
        assert_eq!(presentation.face_name, *face_name);
        assert_eq!(presentation.oracle_text_sha256.len(), 64);

        let row = fingerprints
            .lines()
            .find(|line| {
                line.starts_with(&format!("{id}\t")) && line.contains(&format!("\t{face_id}\t"))
            })
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}/{face_id}"));
        assert!(
            row.ends_with(&presentation.oracle_text_sha256),
            "fingerprint drift for {id}/{face_id}: {row}"
        );
    }
}
