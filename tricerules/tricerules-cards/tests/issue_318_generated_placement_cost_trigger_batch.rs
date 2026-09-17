//! Registry and presentation conformance for issue #318.
//!
//! The five exact generator templates must register the five reviewed Standard identities with
//! their complete typed payloads: CR 400.3/608.2d owner-choice library placement, CR 601.2f
//! target-matching cost reduction, CR 111.1/701.6 the registered 1/1 black Rat token, CR
//! 611.2a/514.2 until-end-of-turn keyword grants, and CR 603.2/120.3 self-damage-to-opponent
//! draw triggers.

use tricerules_cards::primitives::{
    CombatRole, EffectSubject, LibraryPlacement, SpellCostModifier, TargetFilter, TargetGroupDef,
    TargetMatchFilter, TargetSchema,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, AbilitySourceZone, Amount, CardRegistry, Color, Layout,
    ManaCost, SpellEffectKind, TriggerCondition,
};

fn single_group(targeting: &tricerules_cards::primitives::TargetingDef) -> &TargetGroupDef {
    let [group] = targeting.groups.as_slice() else {
        panic!("expected exactly one authored target group");
    };
    group
}

#[test]
fn issue_318_registers_the_five_reviewed_identities() {
    let registry = CardRegistry::global();
    for (id, name, face_count, layout) in [
        ("misleading_motes", "Misleading Motes", 1, Layout::Normal),
        ("run_behind", "Run Behind", 1, Layout::Normal),
        ("edgewall_pack", "Edgewall Pack", 1, Layout::Normal),
        ("stratosoarer", "Stratosoarer", 1, Layout::Normal),
        ("thieving_otter", "Thieving Otter", 1, Layout::Normal),
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
fn issue_318_misleading_motes_offers_the_owners_placement_choice() {
    let definition = CardRegistry::global()
        .get("misleading_motes")
        .expect("Misleading Motes");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "misleading_motes");
    assert_eq!(face.mana_cost.to_string(), "{3}{U}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::Blue]);
    assert!(face.cost_modifiers.is_empty());
    assert!(face.triggered_abilities.is_empty());
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::PutInOwnersLibrary {
            subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            placement: LibraryPlacement::OwnerChoiceTopOrBottom,
        }]
    );
    let targeting = face
        .targeting
        .as_ref()
        .expect("Misleading Motes targets a creature");
    let group = single_group(targeting);
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target creature");
    assert_eq!(group.effect_indices, [0]);
    assert!(!group.same_graveyard);
    assert!(group.distinct_from.is_empty());
    assert!(TargetSchema::compile(&face.spell_effect, Some(targeting)).is_ok());
}

#[test]
fn issue_318_run_behind_reduces_only_for_attacking_targets() {
    let definition = CardRegistry::global()
        .get("run_behind")
        .expect("Run Behind");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "run_behind");
    assert_eq!(face.mana_cost.to_string(), "{3}{U}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::Blue]);
    assert_eq!(
        face.cost_modifiers,
        [SpellCostModifier::TargetMatchGenericReduction {
            amount: 1,
            filter: TargetMatchFilter::Battlefield(TargetFilter {
                kind: tricerules_cards::primitives::TargetKind::Creature,
                combat_role: Some(CombatRole::Attacking),
                ..TargetFilter::default()
            }),
        }]
    );
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::PutInOwnersLibrary {
            subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            placement: LibraryPlacement::OwnerChoiceTopOrBottom,
        }]
    );
    let targeting = face
        .targeting
        .as_ref()
        .expect("Run Behind targets a creature");
    let group = single_group(targeting);
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target creature");
    assert_eq!(group.effect_indices, [0]);
    assert!(TargetSchema::compile(&face.spell_effect, Some(targeting)).is_ok());
}

#[test]
fn issue_318_edgewall_pack_keeps_menace_and_creates_the_blockless_rat() {
    let definition = CardRegistry::global()
        .get("edgewall_pack")
        .expect("Edgewall Pack");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "edgewall_pack");
    assert_eq!(face.mana_cost.to_string(), "{3}{R}");
    assert_eq!(face.types, ["Creature", "Dog"]);
    assert_eq!((face.power, face.toughness), (Some(3), Some(3)));
    assert_eq!(face.colors(), vec![Color::Red]);
    assert_eq!(face.keywords, [tricerules_cards::Keyword::Menace]);
    assert!(face.spell_effect.is_empty());
    assert!(face.activated_abilities.is_empty());

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Edgewall Pack must have exactly one ETB trigger");
    };
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(!ability.may);
    assert!(ability.intervening_if.is_none());
    assert!(ability.targeting.is_none());
    assert_eq!(
        ability.effect,
        [SpellEffectKind::CreateTokens {
            token: "rat_b_1_1_cant_block".into(),
            count: Amount::Fixed(1),
            who: tricerules_cards::primitives::PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        }]
    );
}

#[test]
fn issue_318_stratosoarer_grants_flying_and_keeps_basic_landcycling() {
    let definition = CardRegistry::global()
        .get("stratosoarer")
        .expect("Stratosoarer");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "stratosoarer");
    assert_eq!(face.mana_cost.to_string(), "{4}{U}");
    assert_eq!(face.types, ["Creature", "Elemental"]);
    assert_eq!((face.power, face.toughness), (Some(3), Some(5)));
    assert_eq!(face.colors(), vec![Color::Blue]);
    assert_eq!(face.keywords, [tricerules_cards::Keyword::Flying]);
    assert!(face.spell_effect.is_empty());

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Stratosoarer must have exactly one ETB trigger");
    };
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        ability.effect,
        [SpellEffectKind::GrantKeywords {
            subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            keywords: vec![tricerules_cards::Keyword::Flying],
        }]
    );
    let targeting = ability.targeting.as_ref().expect("flying ETB targets");
    let group = single_group(targeting);
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target creature");
    assert_eq!(group.effect_indices, [0]);

    let [cycling] = face.activated_abilities.as_slice() else {
        panic!("Stratosoarer must keep exactly one Basic landcycling ability");
    };
    assert_eq!(cycling.ability_id.as_str(), "activated_01");
    assert_eq!(
        cycling.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(cycling.source_zone, AbilitySourceZone::Hand);
    assert_eq!(
        cycling.costs,
        [
            AbilityCost::Mana(ManaCost::parse("{1}{U}").expect("printed landcycling cost")),
            AbilityCost::DiscardSelf,
        ]
    );
    assert_eq!(
        cycling.effect,
        [SpellEffectKind::SearchLibrary {
            who: tricerules_cards::primitives::PlayerRecipient::Controller,
            optional: false,
            count: 1,
            count_by_cast_cost: None,
            filter: Some(tricerules_cards::primitives::ZoneCardFilter {
                card_type: Some(tricerules_cards::primitives::CardTypeFilter::BasicLand),
                ..tricerules_cards::primitives::ZoneCardFilter::default()
            }),
            slots: Vec::new(),
            zones: tricerules_cards::primitives::SearchZoneSelection::default(),
            destination: tricerules_cards::primitives::SearchDestination::Hand,
            conditional_destination: None,
            shuffle: true,
            reveal: true,
            result_id: None,
        }]
    );
    assert!(cycling.targeting.is_none());
    assert_eq!(cycling.timing, tricerules_cards::ActivationTiming::Normal);
    assert!(cycling.activation_limit.is_none());
}

#[test]
fn issue_318_thieving_otter_draws_on_any_damage_to_an_opponent() {
    let definition = CardRegistry::global()
        .get("thieving_otter")
        .expect("Thieving Otter");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "thieving_otter");
    assert_eq!(face.mana_cost.to_string(), "{2}{U}");
    assert_eq!(face.types, ["Creature", "Otter"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(2)));
    assert_eq!(face.colors(), vec![Color::Blue]);
    assert!(face.keywords.is_empty());
    assert!(face.spell_effect.is_empty());
    assert!(face.static_abilities.is_empty());

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Thieving Otter must have exactly one damage trigger");
    };
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.trigger,
        TriggerCondition::WheneverSelfDealsDamageToOpponent
    );
    assert!(!ability.may);
    assert!(ability.intervening_if.is_none());
    assert!(ability.targeting.is_none());
    assert_eq!(
        ability.effect,
        [SpellEffectKind::Draw {
            who: tricerules_cards::primitives::PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        }]
    );
}

#[test]
fn issue_318_fingerprint_rows_match_the_presentation_registry() {
    let registry = CardRegistry::global();
    let cases: &[(&str, &str, &str, &str)] = &[
        (
            "misleading_motes",
            "misleading_motes",
            "Misleading Motes",
            "Misleading Motes",
        ),
        ("run_behind", "run_behind", "Run Behind", "Run Behind"),
        (
            "edgewall_pack",
            "edgewall_pack",
            "Edgewall Pack",
            "Edgewall Pack",
        ),
        (
            "stratosoarer",
            "stratosoarer",
            "Stratosoarer",
            "Stratosoarer",
        ),
        (
            "thieving_otter",
            "thieving_otter",
            "Thieving Otter",
            "Thieving Otter",
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
