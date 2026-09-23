use tricerules_cards::primitives::{
    EffectSubject, Keyword, SpellEffectKind, TargetController, TargetKind,
};
use tricerules_cards::CardRegistry;

#[test]
fn issue_misc48_two_target_power_damage_spells_are_registered() {
    let registry = CardRegistry::global();
    for (id, name, mana) in [
        ("rabid_gnaw", "Rabid Gnaw", "{1}{R}"),
        ("diplomatic_relations", "Diplomatic Relations", "{2}{G}"),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing registered card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id);
        assert_eq!(face.mana_cost.to_string(), mana);
        assert_eq!(face.types, ["Instant"]);
    }
}

#[test]
fn issue_misc48_maps_buffs_and_power_damage_to_the_same_creature_targets() {
    let registry = CardRegistry::global();

    let gnaw = registry
        .get("rabid_gnaw")
        .expect("Rabid Gnaw")
        .primary_face();
    assert!(matches!(
        gnaw.spell_effect.as_slice(),
        [
            SpellEffectKind::PumpTarget {
                power: 1,
                toughness: 0,
                subject: EffectSubject::Chosen(own_filter),
                ..
            },
            SpellEffectKind::CreatureDealsDamageEqualToPower {
                source: damage_source,
                target: damage_target,
            }
        ] if own_filter.kind == TargetKind::Creature
            && own_filter.controller == TargetController::You
            && damage_source.kind == TargetKind::Creature
            && damage_source.controller == TargetController::You
            && damage_target.kind == TargetKind::Creature
            && damage_target.controller == TargetController::NotYou
    ));
    let gnaw_groups = &gnaw
        .targeting
        .as_ref()
        .expect("Rabid Gnaw targeting")
        .groups;
    assert_eq!(gnaw_groups.len(), 2);
    assert_eq!(gnaw_groups[0].effect_indices, [0, 1]);
    assert_eq!(gnaw_groups[1].effect_indices, [1]);
    assert_eq!(gnaw_groups[1].distinct_from, [0]);

    let relations = registry
        .get("diplomatic_relations")
        .expect("Diplomatic Relations")
        .primary_face();
    assert!(matches!(
        relations.spell_effect.as_slice(),
        [
            SpellEffectKind::PumpTarget {
                power: 1,
                toughness: 0,
                subject: EffectSubject::Chosen(own_filter),
                ..
            },
            SpellEffectKind::GrantKeywords {
                subject: EffectSubject::Chosen(vigilant_filter),
                keywords,
            },
            SpellEffectKind::CreatureDealsDamageEqualToPower {
                source: damage_source,
                target: damage_target,
            }
        ] if own_filter.kind == TargetKind::Creature
            && own_filter.controller == TargetController::You
            && vigilant_filter.kind == TargetKind::Creature
            && vigilant_filter.controller == TargetController::You
            && keywords == &[Keyword::Vigilance]
            && damage_source.kind == TargetKind::Creature
            && damage_source.controller == TargetController::You
            && damage_target.kind == TargetKind::Creature
            && damage_target.controller == TargetController::Opponent
    ));
    let relations_groups = &relations
        .targeting
        .as_ref()
        .expect("Diplomatic Relations targeting")
        .groups;
    assert_eq!(relations_groups.len(), 2);
    assert_eq!(relations_groups[0].effect_indices, [0, 1, 2]);
    assert_eq!(relations_groups[1].effect_indices, [2]);
    assert_eq!(relations_groups[1].distinct_from, [0]);
}

#[test]
fn issue_misc48_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "rabid_gnaw",
            "Rabid Gnaw",
            "c11b2a7386026851fabcb2d3930f5b908912f3ae55c4c5fa62c9375a5a209f54",
        ),
        (
            "diplomatic_relations",
            "Diplomatic Relations",
            "68bb870a4cc37fbe82f4f24bb473a07518a8c412d42fcaf63cf4e455eb963456",
        ),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[1], name);
        assert_eq!(
            fields[4], fingerprint,
            "pinned Oracle fingerprint drift for {id}"
        );
    }
}
