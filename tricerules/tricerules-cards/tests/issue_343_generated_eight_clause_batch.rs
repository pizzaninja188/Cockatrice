//! Registry and presentation conformance for issue #343.
//!
//! The eight generated Standard identities must register with their complete typed payloads:
//! CR 701.8 the damage-marked opposing-creature destruction; CR 611.2a/514.2/702.15 the +2/+2
//! lifelink pump; CR 602.2/122.1 the parameterized activated +1/+1 counter; CR 603.2/202.3 the
//! mana-value-four cast counter; CR 701.9/111.10a the looting Treasure activation; CR 610.3 the
//! artifact linked exile; CR 404/401 the any-graveyard bottom move; and CR 603.6a/701.3 the
//! Equipment landfall pump of the attached creature.

use tricerules_cards::primitives::{
    CastTriggerPlayer, DiscardQuantity, EffectSubject, GraveyardDestination, GraveyardFilter,
    GraveyardOwner, PermanentEventFilter, PermanentTypeFilter, PlayerRecipient, SpellCastFilter,
    TargetController, TargetFilter, TargetKind,
};
use tricerules_cards::{
    AbilityPresentation, Amount, CardRegistry, Color, CounterKind, Keyword, SpellEffectKind,
    TriggerCondition,
};

#[test]
fn issue_343_registers_the_eight_reviewed_identities() {
    let registry = CardRegistry::global();
    for (id, name) in [
        ("adventuring_gear", "Adventuring Gear"),
        ("collectors_vault", "Collector's Vault"),
        ("give_in_to_violence", "Give In to Violence"),
        ("hoverstone_pilgrim", "Hoverstone Pilgrim"),
        ("lurking_lizards", "Lurking Lizards"),
        ("stingblade_assassin", "Stingblade Assassin"),
        ("toadstool_admirer", "Toadstool Admirer"),
        ("white_auracite", "White Auracite"),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing reviewed card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
    }
}

#[test]
fn issue_343_stingblade_assassin_destroys_a_damage_marked_opposing_creature() {
    let definition = CardRegistry::global()
        .get("stingblade_assassin")
        .expect("Stingblade Assassin");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}{B}");
    assert_eq!(face.types, ["Creature", "Faerie", "Assassin"]);
    assert_eq!((face.power, face.toughness), (Some(3), Some(1)));
    assert_eq!(face.colors(), vec![Color::Black]);
    assert_eq!(face.keywords, [Keyword::Flash, Keyword::Flying]);
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Stingblade Assassin must have exactly one triggered ability");
    };
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(!ability.may);
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::Opponent,
                was_dealt_damage_this_turn: Some(true),
                ..TargetFilter::default()
            })),
        }]
    );
    let targeting = ability.targeting.as_ref().expect("destroy target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("Stingblade Assassin must own exactly one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.effect_indices, [0]);
    assert_eq!(
        group.prompt,
        "Choose target creature an opponent controls that was dealt damage this turn"
    );
}

#[test]
fn issue_343_give_in_to_violence_pumps_and_grants_lifelink() {
    let definition = CardRegistry::global()
        .get("give_in_to_violence")
        .expect("Give In to Violence");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{B}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::Black]);
    let chosen = EffectSubject::Chosen(Box::new(TargetFilter::default_creature()));
    assert_eq!(
        face.spell_effect,
        [
            SpellEffectKind::PumpTarget {
                power: 2,
                toughness: 2,
                scale: None,
                subject: chosen.clone(),
            },
            SpellEffectKind::GrantKeywords {
                subject: chosen,
                keywords: vec![Keyword::Lifelink],
            },
        ]
    );
    assert!(
        face.targeting.is_none(),
        "the pump and grant share the implicit one-creature target contract"
    );
}

#[test]
fn issue_343_toadstool_admirer_keeps_ward_and_the_activated_counter() {
    let definition = CardRegistry::global()
        .get("toadstool_admirer")
        .expect("Toadstool Admirer");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{G}");
    assert_eq!(face.types, ["Creature", "Ouphe"]);
    assert_eq!((face.power, face.toughness), (Some(1), Some(1)));
    assert_eq!(face.colors(), vec![Color::Green]);

    let [activated] = face.activated_abilities.as_slice() else {
        panic!("Toadstool Admirer must keep exactly one activated ability");
    };
    assert_eq!(
        activated.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        activated.costs,
        [tricerules_cards::AbilityCost::Mana(
            tricerules_cards::ManaCost::parse("{3}{G}").expect("static cost")
        )]
    );
    assert!(activated.targeting.is_none());
    assert_eq!(
        activated.effect,
        [SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(1),
            subject: EffectSubject::Source,
        }]
    );

    let [ward] = face.triggered_abilities.as_slice() else {
        panic!("Toadstool Admirer must keep exactly one triggered ability");
    };
    assert_eq!(
        ward.trigger,
        TriggerCondition::WheneverSelfBecomesTarget {
            source: tricerules_cards::primitives::TargetingSourceFilter::SpellOrAbility,
            source_controller: CastTriggerPlayer::Opponent,
        }
    );
    assert_eq!(ward.presentation, AbilityPresentation::OracleLines(vec![1]));
}

#[test]
fn issue_343_lurking_lizards_counts_mana_value_four_or_greater() {
    let definition = CardRegistry::global()
        .get("lurking_lizards")
        .expect("Lurking Lizards");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{G}");
    assert_eq!(face.types, ["Creature", "Lizard", "Villain"]);
    assert_eq!((face.power, face.toughness), (Some(1), Some(3)));
    assert_eq!(face.colors(), vec![Color::Green]);
    assert_eq!(face.keywords, [Keyword::Trample]);
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Lurking Lizards must have exactly one triggered ability");
    };
    assert_eq!(
        ability.trigger,
        TriggerCondition::WheneverPlayerCastsSpell {
            caster: CastTriggerPlayer::Controller,
            filter: SpellCastFilter {
                min_mana_value: Some(4),
                ..SpellCastFilter::default()
            },
            ordinal: None,
            ordinal_scope: Default::default(),
        }
    );
    assert!(!ability.may);
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(1),
            subject: EffectSubject::Source,
        }]
    );
}

#[test]
fn issue_343_collectors_vault_loots_then_creates_a_treasure() {
    let definition = CardRegistry::global()
        .get("collectors_vault")
        .expect("Collector's Vault");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{2}");
    assert_eq!(face.types, ["Artifact"]);
    assert_eq!(face.face_id.as_str(), "collector_s_vault");
    assert_eq!(face.colors(), Vec::<Color>::new());
    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Collector's Vault must have exactly one activated ability");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.costs,
        [
            tricerules_cards::AbilityCost::Mana(
                tricerules_cards::ManaCost::parse("{2}").expect("static cost")
            ),
            tricerules_cards::AbilityCost::Tap,
        ]
    );
    assert!(ability.targeting.is_none());
    assert_eq!(
        ability.effect,
        [
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            },
            SpellEffectKind::Discard {
                who: PlayerRecipient::Controller,
                quantity: DiscardQuantity::Exact(1),
            },
            SpellEffectKind::CreateTokens {
                token: "treasure".into(),
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None,
            },
        ]
    );
}

#[test]
fn issue_343_white_auracite_linked_exiles_an_opposing_nonland() {
    let definition = CardRegistry::global()
        .get("white_auracite")
        .expect("White Auracite");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{2}{W}{W}");
    assert_eq!(face.types, ["Artifact"]);
    assert_eq!(face.colors(), vec![Color::White]);
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("White Auracite must have exactly one triggered ability");
    };
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(!ability.may);
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::ExileUntilSourceLeaves {
            target: TargetFilter {
                kind: TargetKind::AnyPermanent,
                controller: TargetController::Opponent,
                excluded_permanent_types: vec![PermanentTypeFilter::Land],
                ..TargetFilter::default()
            },
        }]
    );
    let targeting = ability.targeting.as_ref().expect("exile target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("White Auracite must own exactly one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.effect_indices, [0]);
    assert_eq!(
        group.prompt,
        "Choose target nonland permanent an opponent controls"
    );

    let [activated] = face.activated_abilities.as_slice() else {
        panic!("White Auracite must keep the {{W}} mana ability");
    };
    assert_eq!(
        activated.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
}

#[test]
fn issue_343_hoverstone_pilgrim_keeps_flying_ward_and_any_graveyard_bottom() {
    let definition = CardRegistry::global()
        .get("hoverstone_pilgrim")
        .expect("Hoverstone Pilgrim");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{5}");
    assert_eq!(face.types, ["Artifact", "Creature", "Golem"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(5)));
    assert_eq!(face.keywords, [Keyword::Flying]);
    let [activated] = face.activated_abilities.as_slice() else {
        panic!("Hoverstone Pilgrim must have exactly one activated ability");
    };
    assert_eq!(
        activated.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(
        activated.effect,
        [SpellEffectKind::MoveGraveyardCards {
            filter: GraveyardFilter {
                owner: GraveyardOwner::AnyPlayer,
                ..GraveyardFilter::default()
            },
            destination: GraveyardDestination::LibraryBottom,
            linked_exile_id: None,
        }]
    );
    let targeting = activated
        .targeting
        .as_ref()
        .expect("graveyard target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("Hoverstone Pilgrim must own exactly one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.effect_indices, [0]);
    assert_eq!(group.prompt, "Choose target card from a graveyard");

    let [ward] = face.triggered_abilities.as_slice() else {
        panic!("Hoverstone Pilgrim must keep exactly one triggered ability");
    };
    assert_eq!(
        ward.trigger,
        TriggerCondition::WheneverSelfBecomesTarget {
            source: tricerules_cards::primitives::TargetingSourceFilter::SpellOrAbility,
            source_controller: CastTriggerPlayer::Opponent,
        }
    );
    assert_eq!(ward.presentation, AbilityPresentation::OracleLines(vec![2]));
}

#[test]
fn issue_343_adventuring_gear_keeps_equip_and_landfall_pump() {
    let definition = CardRegistry::global()
        .get("adventuring_gear")
        .expect("Adventuring Gear");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}");
    assert_eq!(face.types, ["Artifact", "Equipment"]);
    assert_eq!(face.colors(), Vec::<Color>::new());

    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Adventuring Gear must have exactly one triggered ability");
    };
    assert_eq!(
        trigger.trigger,
        TriggerCondition::WheneverPermanentEntersBattlefield {
            controller: CastTriggerPlayer::Controller,
            filter: PermanentEventFilter {
                permanent_type: Some(PermanentTypeFilter::Land),
                ..PermanentEventFilter::default()
            },
            creature_filter: None,
        }
    );
    assert!(!trigger.may);
    assert!(trigger.targeting.is_none());
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        trigger.effect,
        [SpellEffectKind::PumpTarget {
            power: 2,
            toughness: 2,
            scale: None,
            subject: EffectSubject::AttachedObject,
        }]
    );

    let [equip] = face.activated_abilities.as_slice() else {
        panic!("Adventuring Gear must keep exactly one Equip activation");
    };
    assert_eq!(
        equip.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        equip.effect,
        [SpellEffectKind::Equip {
            target: TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..TargetFilter::default()
            },
        }]
    );
}

#[test]
fn issue_343_fingerprint_rows_match_the_presentation_registry() {
    let registry = CardRegistry::global();
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, face_id) in [
        ("adventuring_gear", "adventuring_gear"),
        ("collectors_vault", "collector_s_vault"),
        ("give_in_to_violence", "give_in_to_violence"),
        ("hoverstone_pilgrim", "hoverstone_pilgrim"),
        ("lurking_lizards", "lurking_lizards"),
        ("stingblade_assassin", "stingblade_assassin"),
        ("toadstool_admirer", "toadstool_admirer"),
        ("white_auracite", "white_auracite"),
    ] {
        let presentation = registry
            .presentation_face(id, face_id)
            .unwrap_or_else(|| panic!("missing presentation metadata for {id}/{face_id}"));
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
