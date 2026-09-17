//! Registry and presentation conformance for issue #333.
//!
//! The six generated Standard identities must register with their complete typed payloads:
//! CR 701.9/701.20 the mandatory target-opponent public reveal and nonland discard; CR 603.6a
//! the Landfall gain-one-life trigger; CR 603.6a/611.2c the other-creature-enters self pump;
//! CR 605/106 the two- and three-mana-any-one-color mana abilities; and CR 613.4c the
//! count-scaled self +1/+0 for each artifact its controller controls.

use tricerules_cards::primitives::{
    BattlefieldPermanentFilter, CardTypeFilter, CountExpression, EffectSubject, HandCardAction,
    HandCardChooser, HandChoiceVisibility, PermanentEventFilter, PermanentTypeFilter,
    RelativePlayerSet, StaticAbilityDef, TargetFilter, TargetKind,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, Amount, CardRegistry, CastTriggerPlayer, Color, Keyword,
    Layout, ManaAmount, SpellEffectKind, TriggerCondition,
};

const ISSUE_333_CARDS: &[(&str, &str)] = &[
    ("pilfer", "Pilfer"),
    ("eumidian_terrabotanist", "Eumidian Terrabotanist"),
    ("loporrit_scout", "Loporrit Scout"),
    ("transdimensional_bovine", "Transdimensional Bovine"),
    ("gilded_lotus", "Gilded Lotus"),
    ("guidelight_synergist", "Guidelight Synergist"),
];

fn artifact_count_filter() -> BattlefieldPermanentFilter {
    BattlefieldPermanentFilter {
        token: None,
        any_of: None,
        controllers: RelativePlayerSet::Controller,
        card_type: Some(CardTypeFilter::Artifact),
        color: None,
        name: None,
        required_subtypes: Vec::new(),
        exclude_source: false,
    }
}

fn any_one_color_options(per_color: u32) -> Vec<ManaAmount> {
    [
        ManaAmount {
            w: per_color,
            ..ManaAmount::default()
        },
        ManaAmount {
            u: per_color,
            ..ManaAmount::default()
        },
        ManaAmount {
            b: per_color,
            ..ManaAmount::default()
        },
        ManaAmount {
            r: per_color,
            ..ManaAmount::default()
        },
        ManaAmount {
            g: per_color,
            ..ManaAmount::default()
        },
    ]
    .to_vec()
}

#[test]
fn issue_333_registers_the_six_reviewed_identities() {
    let registry = CardRegistry::global();
    for (id, name) in ISSUE_333_CARDS {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing reviewed card {id}"));
        assert_eq!(definition.name, *name);
        assert_eq!(registry.id_for_name(name), Some(*id));
        assert_eq!(definition.layout, Layout::Normal);
        assert_eq!(definition.face_count(), 1);
        assert_eq!(definition.primary_face().face_id.as_str(), *id);
    }
}

#[test]
fn issue_333_pilfer_targets_an_opponent_for_a_public_nonland_discard() {
    let definition = CardRegistry::global().get("pilfer").expect("Pilfer");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{B}");
    assert_eq!(face.types, ["Sorcery"]);
    assert_eq!(face.colors(), vec![Color::Black]);
    assert!(face.triggered_abilities.is_empty());
    assert!(face.activated_abilities.is_empty());

    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::ChooseHandCards {
            action: HandCardAction::Discard,
            count: 1,
            target: TargetFilter {
                kind: TargetKind::OpponentPlayer,
                ..TargetFilter::default()
            },
            chooser: HandCardChooser::Controller,
            card_filter: Some(CardTypeFilter::Nonland),
            optional: false,
            visibility: HandChoiceVisibility::PublicReveal,
        }]
    );

    let targeting = face.targeting.as_ref().expect("Pilfer target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("Pilfer must own exactly one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target opponent");
    assert_eq!(group.effect_indices, [0]);
}

#[test]
fn issue_333_eumidian_terrabotanist_gains_one_life_on_a_controlled_land_entry() {
    let definition = CardRegistry::global()
        .get("eumidian_terrabotanist")
        .expect("Eumidian Terrabotanist");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{G}");
    assert_eq!(face.types, ["Creature", "Insect", "Druid"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(3)));
    assert_eq!(face.colors(), vec![Color::Green]);

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Eumidian Terrabotanist must have exactly one triggered ability");
    };
    assert_eq!(
        ability.trigger,
        TriggerCondition::WheneverPermanentEntersBattlefield {
            controller: CastTriggerPlayer::Controller,
            filter: PermanentEventFilter {
                permanent_type: Some(PermanentTypeFilter::Land),
                ..PermanentEventFilter::default()
            },
            creature_filter: None,
        }
    );
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::GainLife {
            amount: Amount::Fixed(1),
        }]
    );
    assert!(ability.targeting.is_none());
    assert!(!ability.may);
    assert!(ability.intervening_if.is_none());
}

#[test]
fn issue_333_loporrit_scout_pumps_itself_for_another_creature_entry() {
    let definition = CardRegistry::global()
        .get("loporrit_scout")
        .expect("Loporrit Scout");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{2}{G}");
    assert_eq!(face.types, ["Creature", "Rabbit", "Scout"]);
    assert_eq!((face.power, face.toughness), (Some(3), Some(2)));
    assert_eq!(face.colors(), vec![Color::Green]);

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Loporrit Scout must have exactly one triggered ability");
    };
    assert_eq!(
        ability.trigger,
        TriggerCondition::WheneverPermanentEntersBattlefield {
            controller: CastTriggerPlayer::Controller,
            filter: PermanentEventFilter {
                permanent_type: Some(PermanentTypeFilter::Creature),
                exclude_source: true,
                ..PermanentEventFilter::default()
            },
            creature_filter: None,
        }
    );
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::PumpTarget {
            power: 1,
            toughness: 1,
            scale: None,
            subject: EffectSubject::Source,
        }]
    );
    assert!(ability.targeting.is_none());
}

#[test]
fn issue_333_transdimensional_bovine_taps_for_two_mana_of_any_one_color() {
    let definition = CardRegistry::global()
        .get("transdimensional_bovine")
        .expect("Transdimensional Bovine");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{2}{G}");
    assert_eq!(face.types, ["Creature", "Ox", "Avatar"]);
    assert_eq!((face.power, face.toughness), (Some(0), Some(4)));
    assert_eq!(face.colors(), vec![Color::Green]);
    assert_eq!(face.keywords, [Keyword::Flying]);

    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Transdimensional Bovine must have exactly one activated ability");
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(ability.costs, [AbilityCost::Tap]);
    assert!(ability.targeting.is_none());
    assert_eq!(
        ability.effect,
        [SpellEffectKind::ProduceMana {
            options: any_one_color_options(2),
            restriction: None,
            conditional: None,
        }]
    );
}

#[test]
fn issue_333_gilded_lotus_taps_for_three_mana_of_any_one_color() {
    let definition = CardRegistry::global()
        .get("gilded_lotus")
        .expect("Gilded Lotus");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{5}");
    assert_eq!(face.types, ["Artifact"]);
    assert!(face.colors().is_empty());
    assert!(face.keywords.is_empty());

    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Gilded Lotus must have exactly one activated ability");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(ability.costs, [AbilityCost::Tap]);
    assert_eq!(
        ability.effect,
        [SpellEffectKind::ProduceMana {
            options: any_one_color_options(3),
            restriction: None,
            conditional: None,
        }]
    );
}

#[test]
fn issue_333_guidelight_synergist_scales_power_with_its_artifacts() {
    let definition = CardRegistry::global()
        .get("guidelight_synergist")
        .expect("Guidelight Synergist");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}{W}");
    assert_eq!(face.types, ["Artifact", "Creature", "Robot", "Artificer"]);
    assert_eq!((face.power, face.toughness), (Some(0), Some(4)));
    assert_eq!(face.colors(), vec![Color::White]);
    assert_eq!(face.keywords, [Keyword::Flying]);

    let [ability] = face.static_abilities.as_slice() else {
        panic!("Guidelight Synergist must have exactly one static ability");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.definition,
        StaticAbilityDef::CountScaledSelfPt {
            count: CountExpression::BattlefieldPermanents {
                filter: artifact_count_filter(),
            },
            power_per_match: 1,
            toughness_per_match: 0,
        }
    );
}

#[test]
fn issue_333_fingerprint_rows_match_the_presentation_registry() {
    let registry = CardRegistry::global();
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, card_name) in ISSUE_333_CARDS {
        let presentation = registry
            .presentation_face(id, id)
            .unwrap_or_else(|| panic!("missing presentation metadata for {id}"));
        assert_eq!(presentation.card_name, *card_name);
        assert_eq!(presentation.face_name, *card_name);
        assert_eq!(presentation.oracle_text_sha256.len(), 64);

        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        assert!(
            row.ends_with(&presentation.oracle_text_sha256),
            "fingerprint drift for {id}: {row}"
        );
    }
}
