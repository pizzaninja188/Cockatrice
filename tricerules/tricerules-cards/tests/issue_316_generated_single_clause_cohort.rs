//! Registry and presentation conformance for issue #316.
//!
//! The six exact generator templates must register the seven reviewed Standard identities with
//! their complete typed payloads: CR 115 target predicates, CR 603 entry and cast triggers, CR 701
//! destroy/exile actions, CR 702.7 first strike, and the Adventure/Omen two-face structure.

use tricerules_cards::primitives::{
    CastTriggerPlayer, EffectSubject, GraveyardDestination, GraveyardFilter, GraveyardOwner,
    PermanentTypeFilter, PowerComparison, SpellCastFilter, SpellEffectKind, TargetFilter,
    TargetGroupDef, TargetKind, TargetSchema,
};
use tricerules_cards::{
    AbilityPresentation, CardRegistry, CharacteristicDefiningAbility, Color, Keyword, Layout,
    TriggerCondition,
};

fn single_group(targeting: &tricerules_cards::primitives::TargetingDef) -> &TargetGroupDef {
    let [group] = targeting.groups.as_slice() else {
        panic!("expected exactly one authored target group");
    };
    group
}

#[test]
fn issue_316_registers_the_seven_reviewed_identities() {
    let registry = CardRegistry::global();
    for (id, name, face_count, layout) in [
        (
            "threadbind_clique_rip_the_seams",
            "Threadbind Clique // Rip the Seams",
            2,
            Layout::Adventure,
        ),
        (
            "chomping_changeling",
            "Chomping Changeling",
            1,
            Layout::Normal,
        ),
        (
            "disruptive_stormbrood_petty_revenge",
            "Disruptive Stormbrood // Petty Revenge",
            2,
            Layout::Omen,
        ),
        ("griffnaut_tracker", "Griffnaut Tracker", 1, Layout::Normal),
        ("firebrand_archer", "Firebrand Archer", 1, Layout::Normal),
        ("kindled_fury", "Kindled Fury", 1, Layout::Normal),
        ("sure_strike", "Sure Strike", 1, Layout::Normal),
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
fn issue_316_rip_the_seams_adventure_face_destroys_only_tapped_creatures() {
    let definition = CardRegistry::global()
        .get("threadbind_clique_rip_the_seams")
        .expect("Threadbind Clique // Rip the Seams");
    assert_eq!(definition.layout, Layout::Adventure);

    let creature = definition.primary_face();
    assert_eq!(creature.face_id.as_str(), "threadbind_clique");
    assert_eq!(creature.mana_cost.to_string(), "{3}{U}");
    assert_eq!(creature.types, ["Creature", "Faerie"]);
    assert_eq!((creature.power, creature.toughness), (Some(3), Some(3)));
    assert_eq!(creature.colors(), vec![Color::Blue]);
    assert_eq!(creature.keywords, [Keyword::Flying]);
    assert!(creature.spell_effect.is_empty());
    assert!(creature.triggered_abilities.is_empty());

    let adventure = definition.face(1).expect("Rip the Seams face");
    assert_eq!(adventure.face_id.as_str(), "rip_the_seams");
    assert_eq!(adventure.mana_cost.to_string(), "{2}{W}");
    assert_eq!(adventure.types, ["Instant", "Adventure"]);
    assert_eq!(adventure.colors(), vec![Color::White]);
    assert_eq!(
        adventure.spell_effect,
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                tapped: Some(true),
                ..TargetFilter::default()
            })),
        }]
    );
    let targeting = adventure.targeting.as_ref().expect("Rip the Seams targets");
    let group = single_group(targeting);
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target tapped creature");
    assert_eq!(group.effect_indices, [0]);
    assert!(!group.same_graveyard);
    assert!(TargetSchema::compile(&adventure.spell_effect, Some(targeting)).is_ok());
}

#[test]
fn issue_316_chomping_changeling_keeps_changeling_and_optional_destroy() {
    let definition = CardRegistry::global()
        .get("chomping_changeling")
        .expect("Chomping Changeling");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "chomping_changeling");
    assert_eq!(face.mana_cost.to_string(), "{2}{G}");
    assert_eq!(face.types, ["Creature", "Shapeshifter"]);
    assert_eq!((face.power, face.toughness), (Some(1), Some(2)));
    assert_eq!(face.colors(), vec![Color::Green]);
    assert!(face.keywords.is_empty());

    let [changeling] = face.characteristic_defining_abilities.as_slice() else {
        panic!("Chomping Changeling must keep exactly one Changeling CDA");
    };
    assert_eq!(changeling.ability_id.as_str(), "characteristic_01");
    assert_eq!(
        changeling.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        changeling.definition,
        CharacteristicDefiningAbility::Changeling
    );

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Chomping Changeling must have exactly one triggered ability");
    };
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(!ability.may);
    assert!(ability.intervening_if.is_none());
    assert_eq!(
        ability.effect,
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::AnyPermanent,
                permanent_types: vec![
                    PermanentTypeFilter::Artifact,
                    PermanentTypeFilter::Enchantment,
                ],
                ..TargetFilter::default()
            })),
        }]
    );
    let targeting = ability.targeting.as_ref().expect("ETB targets optionally");
    let group = single_group(targeting);
    assert_eq!((group.min, group.max), (0, 1));
    assert_eq!(
        group.prompt,
        "Choose up to one target artifact or enchantment"
    );
    assert_eq!(group.effect_indices, [0]);
    assert!(!group.same_graveyard);
    assert!(TargetSchema::compile(&ability.effect, Some(targeting)).is_ok());
}

#[test]
fn issue_316_stormbrood_omen_faces_carry_the_etb_and_power_bounded_destroy() {
    let definition = CardRegistry::global()
        .get("disruptive_stormbrood_petty_revenge")
        .expect("Disruptive Stormbrood // Petty Revenge");
    assert_eq!(definition.layout, Layout::Omen);

    let dragon = definition.primary_face();
    assert_eq!(dragon.face_id.as_str(), "disruptive_stormbrood");
    assert_eq!(dragon.mana_cost.to_string(), "{4}{G}");
    assert_eq!(dragon.types, ["Creature", "Dragon"]);
    assert_eq!((dragon.power, dragon.toughness), (Some(3), Some(3)));
    assert_eq!(dragon.colors(), vec![Color::Green]);
    assert_eq!(dragon.keywords, [Keyword::Flying]);
    let [ability] = dragon.triggered_abilities.as_slice() else {
        panic!("Disruptive Stormbrood must have exactly one ETB ability");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(!ability.may);
    assert_eq!(
        ability.effect,
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::AnyPermanent,
                permanent_types: vec![
                    PermanentTypeFilter::Artifact,
                    PermanentTypeFilter::Enchantment,
                ],
                ..TargetFilter::default()
            })),
        }]
    );
    let etb_targeting = ability.targeting.as_ref().expect("ETB targets optionally");
    let etb_group = single_group(etb_targeting);
    assert_eq!((etb_group.min, etb_group.max), (0, 1));
    assert_eq!(
        etb_group.prompt,
        "Choose up to one target artifact or enchantment"
    );

    let omen = definition.face(1).expect("Petty Revenge face");
    assert_eq!(omen.face_id.as_str(), "petty_revenge");
    assert_eq!(omen.mana_cost.to_string(), "{1}{B}");
    assert_eq!(omen.types, ["Sorcery", "Omen"]);
    assert_eq!(omen.colors(), vec![Color::Black]);
    assert!(omen.triggered_abilities.is_empty());
    assert_eq!(
        omen.spell_effect,
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                power: Some(PowerComparison::AtMost(3)),
                ..TargetFilter::default()
            })),
        }]
    );
    let targeting = omen.targeting.as_ref().expect("Petty Revenge targets");
    let group = single_group(targeting);
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target creature with power 3 or less");
    assert_eq!(group.effect_indices, [0]);
    assert!(TargetSchema::compile(&omen.spell_effect, Some(targeting)).is_ok());
}

#[test]
fn issue_316_griffnaut_tracker_exiles_up_to_two_from_one_graveyard() {
    let definition = CardRegistry::global()
        .get("griffnaut_tracker")
        .expect("Griffnaut Tracker");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "griffnaut_tracker");
    assert_eq!(face.mana_cost.to_string(), "{3}{W}");
    assert_eq!(face.types, ["Creature", "Human", "Detective"]);
    assert_eq!((face.power, face.toughness), (Some(3), Some(2)));
    assert_eq!(face.colors(), vec![Color::White]);
    assert_eq!(face.keywords, [Keyword::Flying]);

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Griffnaut Tracker must have exactly one triggered ability");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(!ability.may);
    assert!(ability.intervening_if.is_none());
    assert_eq!(
        ability.effect,
        [SpellEffectKind::MoveGraveyardCards {
            filter: GraveyardFilter {
                owner: GraveyardOwner::AnyPlayer,
                ..GraveyardFilter::default()
            },
            destination: GraveyardDestination::Exile,
            linked_exile_id: None,
        }]
    );
    let targeting = ability.targeting.as_ref().expect("ETB targets optionally");
    let group = single_group(targeting);
    assert_eq!((group.min, group.max), (0, 2));
    assert_eq!(
        group.prompt,
        "Choose up to two target cards from a single graveyard"
    );
    assert_eq!(group.effect_indices, [0]);
    assert!(group.same_graveyard);
    assert!(TargetSchema::compile(&ability.effect, Some(targeting)).is_ok());
}

#[test]
fn issue_316_firebrand_archer_pings_each_opponent_on_owner_noncreature_casts() {
    let definition = CardRegistry::global()
        .get("firebrand_archer")
        .expect("Firebrand Archer");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "firebrand_archer");
    assert_eq!(face.mana_cost.to_string(), "{1}{R}");
    assert_eq!(face.types, ["Creature", "Human", "Archer"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(1)));
    assert_eq!(face.colors(), vec![Color::Red]);
    assert!(face.keywords.is_empty());

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Firebrand Archer must have exactly one triggered ability");
    };
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.trigger,
        TriggerCondition::WheneverPlayerCastsSpell {
            caster: CastTriggerPlayer::Controller,
            filter: SpellCastFilter {
                card_type: Some(tricerules_cards::primitives::CardTypeFilter::Noncreature),
                ..SpellCastFilter::default()
            },
            ordinal: None,
            ordinal_scope: Default::default(),
        }
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::DamagePlayer {
            amount: tricerules_cards::Amount::Fixed(1),
            who: tricerules_cards::primitives::PlayerRecipient::EachOpponent,
        }]
    );
    assert!(ability.targeting.is_none());
    assert!(!ability.may);
    assert!(ability.intervening_if.is_none());
}

#[test]
fn issue_316_first_strike_pumps_share_one_mandatory_target_group() {
    let registry = CardRegistry::global();
    for (id, name, mana, expected_power) in [
        ("kindled_fury", "Kindled Fury", "{R}", 1),
        ("sure_strike", "Sure Strike", "{1}{R}", 3),
    ] {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id);
        assert_eq!(face.mana_cost.to_string(), mana);
        assert_eq!(face.types, ["Instant"]);
        assert_eq!(face.colors(), vec![Color::Red]);
        assert_eq!(
            face.spell_effect,
            [
                SpellEffectKind::PumpTarget {
                    power: expected_power,
                    toughness: 0,
                    scale: None,
                    subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                },
                SpellEffectKind::GrantKeywords {
                    subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                    keywords: vec![Keyword::FirstStrike],
                },
            ],
            "{name}"
        );
        let targeting = face
            .targeting
            .as_ref()
            .unwrap_or_else(|| panic!("{name} targets"));
        let group = single_group(targeting);
        assert_eq!((group.min, group.max), (1, 1));
        assert_eq!(group.prompt, "Choose target creature");
        assert_eq!(group.effect_indices, [0, 1]);
        assert!(!group.same_graveyard);
        assert!(TargetSchema::compile(&face.spell_effect, Some(targeting)).is_ok());
    }
}

#[test]
fn issue_316_fingerprint_rows_match_the_presentation_registry() {
    let registry = CardRegistry::global();
    let cases: &[(&str, &str, &str, &str)] = &[
        (
            "threadbind_clique_rip_the_seams",
            "threadbind_clique",
            "Threadbind Clique // Rip the Seams",
            "Threadbind Clique",
        ),
        (
            "threadbind_clique_rip_the_seams",
            "rip_the_seams",
            "Threadbind Clique // Rip the Seams",
            "Rip the Seams",
        ),
        (
            "disruptive_stormbrood_petty_revenge",
            "disruptive_stormbrood",
            "Disruptive Stormbrood // Petty Revenge",
            "Disruptive Stormbrood",
        ),
        (
            "disruptive_stormbrood_petty_revenge",
            "petty_revenge",
            "Disruptive Stormbrood // Petty Revenge",
            "Petty Revenge",
        ),
        (
            "chomping_changeling",
            "chomping_changeling",
            "Chomping Changeling",
            "Chomping Changeling",
        ),
        (
            "griffnaut_tracker",
            "griffnaut_tracker",
            "Griffnaut Tracker",
            "Griffnaut Tracker",
        ),
        (
            "firebrand_archer",
            "firebrand_archer",
            "Firebrand Archer",
            "Firebrand Archer",
        ),
        (
            "kindled_fury",
            "kindled_fury",
            "Kindled Fury",
            "Kindled Fury",
        ),
        ("sure_strike", "sure_strike", "Sure Strike", "Sure Strike"),
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
