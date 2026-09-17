//! Registry and presentation conformance for issue #336.
//!
//! The eight generated Standard identities must register with their complete typed payloads:
//! CR 611.2a/514.2 the asymmetric -4/-0 pump on the Adventure face; CR 601.2f/118.7a the
//! Wizard-gated generic cost reduction; CR 602.2/601.2h the printed-cost activated draw; CR 404/701.13
//! the {2}, {T} graveyard exile; CR 603.6/701.14 the optional fight; CR 603.6/404.2 the optional
//! graveyard return; CR 603.6/701.8 the optional artifact-or-enchantment destroy; and
//! CR 111.10a/603.6 the Equipment entry Treasure alongside its +1/+1 and Equip.

use tricerules_cards::primitives::{
    BattlefieldCreatureCountFilter, CardTypeFilter, EffectSubject, GameCondition,
    GraveyardDestination, GraveyardFilter, GraveyardOwner, PermanentTypeFilter, PlayerRecipient,
    RelativePlayerSet, SpellCostModifier, StaticAbilityDef, TargetController, TargetFilter,
    TargetKind, ZoneCardFilter,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, Amount, CardRegistry, CharacteristicDefiningAbility, Color,
    Keyword, Layout, ManaCost, SpellEffectKind, TriggerCondition,
};

#[test]
fn issue_336_registers_the_eight_reviewed_identities() {
    let registry = CardRegistry::global();
    let expected = [
        ("affectionate_indrik", "Affectionate Indrik", 1usize),
        ("arcane_epiphany", "Arcane Epiphany", 1),
        ("gold_pan", "Gold Pan", 1),
        ("graveshifter", "Graveshifter", 1),
        ("magic_pot", "Magic Pot", 1),
        ("reclamation_sage", "Reclamation Sage", 1),
        ("spectral_sailor", "Spectral Sailor", 1),
        (
            "obyras_attendants_desperate_parry",
            "Obyra's Attendants // Desperate Parry",
            2,
        ),
    ];
    for (id, name, faces) in expected {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing reviewed card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        assert_eq!(definition.face_count(), faces);
    }
    assert_eq!(
        registry
            .get("obyras_attendants_desperate_parry")
            .expect("Obyra")
            .layout,
        Layout::Adventure
    );
    assert_eq!(
        registry.get("affectionate_indrik").expect("Indrik").layout,
        Layout::Normal
    );
}

#[test]
fn issue_336_desperate_parry_carries_the_minus_four_minus_zero_pump() {
    let definition = CardRegistry::global()
        .get("obyras_attendants_desperate_parry")
        .expect("Obyra's Attendants // Desperate Parry");
    let front = definition.face(0).expect("front face");
    assert_eq!(front.name, "Obyra's Attendants");
    assert_eq!(front.face_id.as_str(), "obyra_s_attendants");
    assert_eq!(front.mana_cost.to_string(), "{4}{U}");
    assert_eq!(front.types, ["Creature", "Faerie", "Wizard"]);
    assert_eq!((front.power, front.toughness), (Some(3), Some(4)));
    assert_eq!(front.colors(), vec![Color::Blue]);
    assert_eq!(front.keywords, [Keyword::Flying]);
    assert!(front.spell_effect.is_empty());

    let adventure = definition.face(1).expect("adventure face");
    assert_eq!(adventure.name, "Desperate Parry");
    assert_eq!(adventure.face_id.as_str(), "desperate_parry");
    assert_eq!(adventure.mana_cost.to_string(), "{1}{U}");
    assert_eq!(adventure.types, ["Instant", "Adventure"]);
    assert_eq!(adventure.colors(), vec![Color::Blue]);
    assert_eq!(
        adventure.spell_effect,
        [SpellEffectKind::PumpTarget {
            power: -4,
            toughness: 0,
            scale: None,
            subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
        }]
    );
    assert!(
        adventure.targeting.is_none(),
        "the single targeted instruction uses the implicit one-target contract"
    );
}

#[test]
fn issue_336_arcane_epiphany_reduces_only_with_a_controlled_wizard() {
    let definition = CardRegistry::global()
        .get("arcane_epiphany")
        .expect("Arcane Epiphany");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}{U}{U}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::Blue]);
    assert_eq!(
        face.cost_modifiers,
        [SpellCostModifier::ConditionalGenericReduction {
            amount: 1,
            condition: GameCondition::BattlefieldCreatureCount {
                filter: BattlefieldCreatureCountFilter {
                    controllers: RelativePlayerSet::Controller,
                    subtype: Some("Wizard".into()),
                    ..BattlefieldCreatureCountFilter::default()
                },
                min: Some(1),
                max: None,
            },
        }]
    );
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(3),
        }]
    );
}

#[test]
fn issue_336_spectral_sailor_keeps_flash_flying_and_the_printed_draw_cost() {
    let definition = CardRegistry::global()
        .get("spectral_sailor")
        .expect("Spectral Sailor");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{U}");
    assert_eq!(face.types, ["Creature", "Spirit", "Pirate"]);
    assert_eq!((face.power, face.toughness), (Some(1), Some(1)));
    assert_eq!(face.colors(), vec![Color::Blue]);
    assert_eq!(face.keywords, [Keyword::Flash, Keyword::Flying]);
    assert!(face.triggered_abilities.is_empty());
    assert!(face.static_abilities.is_empty());

    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Spectral Sailor must have exactly one activated ability");
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(
        ability.costs,
        [AbilityCost::Mana(
            ManaCost::parse("{3}{U}").expect("fixed mana cost")
        )]
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        }]
    );
    assert!(ability.targeting.is_none());
}

#[test]
fn issue_336_magic_pot_exiles_any_graveyard_card_and_keeps_its_dies_treasure() {
    let definition = CardRegistry::global().get("magic_pot").expect("Magic Pot");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}");
    assert_eq!(face.types, ["Artifact", "Creature", "Goblin", "Construct"]);
    assert_eq!((face.power, face.toughness), (Some(1), Some(4)));

    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Magic Pot must have exactly one activated ability");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.costs,
        [
            AbilityCost::Mana(ManaCost::parse("{2}").expect("fixed mana cost")),
            AbilityCost::Tap,
        ]
    );
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
    let targeting = ability.targeting.as_ref().expect("exile target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("Magic Pot must own exactly one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target card from a graveyard");

    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Magic Pot must keep exactly one dies trigger");
    };
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfDies);
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        trigger.effect,
        [SpellEffectKind::CreateTokens {
            token: "treasure".into(),
            count: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        }]
    );
    assert!(trigger.targeting.is_none());
    assert!(!trigger.may);
}

#[test]
fn issue_336_affectionate_indrik_may_fight_a_creature_you_dont_control() {
    let definition = CardRegistry::global()
        .get("affectionate_indrik")
        .expect("Affectionate Indrik");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{5}{G}");
    assert_eq!(face.types, ["Creature", "Beast"]);
    assert_eq!((face.power, face.toughness), (Some(4), Some(4)));
    assert_eq!(face.colors(), vec![Color::Green]);
    assert!(face.activated_abilities.is_empty());
    assert!(face.static_abilities.is_empty());

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Affectionate Indrik must have exactly one triggered ability");
    };
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(
        ability.may,
        "CR 603.5: \"you may\" makes the ETB optional while choosing targets"
    );
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::Fight {
            first: EffectSubject::Source,
            second: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::NotYou,
                ..TargetFilter::default()
            })),
        }]
    );
    let targeting = ability.targeting.as_ref().expect("fight target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("Affectionate Indrik must own exactly one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target creature you don't control");
}

#[test]
fn issue_336_graveshifter_keeps_changeling_and_the_optional_return() {
    let definition = CardRegistry::global()
        .get("graveshifter")
        .expect("Graveshifter");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}{B}");
    assert_eq!(face.types, ["Creature", "Shapeshifter"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(2)));
    assert_eq!(face.colors(), vec![Color::Black]);
    assert!(face.activated_abilities.is_empty());
    assert!(face.static_abilities.is_empty());

    let [changing] = face.characteristic_defining_abilities.as_slice() else {
        panic!("Graveshifter must keep exactly its Changeling CDA");
    };
    assert_eq!(changing.ability_id.as_str(), "characteristic_01");
    assert_eq!(
        changing.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        changing.definition,
        CharacteristicDefiningAbility::Changeling
    );

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Graveshifter must have exactly one triggered ability");
    };
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(ability.may);
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::MoveGraveyardCards {
            filter: GraveyardFilter {
                owner: GraveyardOwner::Controller,
                card: Some(ZoneCardFilter {
                    card_type: Some(CardTypeFilter::Creature),
                    ..ZoneCardFilter::default()
                }),
                ..GraveyardFilter::default()
            },
            destination: GraveyardDestination::Hand,
            linked_exile_id: None,
        }]
    );
    let targeting = ability.targeting.as_ref().expect("return target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("Graveshifter must own exactly one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(
        group.prompt,
        "Choose target creature card from your graveyard"
    );
}

#[test]
fn issue_336_reclamation_sage_may_destroy_an_artifact_or_enchantment() {
    let definition = CardRegistry::global()
        .get("reclamation_sage")
        .expect("Reclamation Sage");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{2}{G}");
    assert_eq!(face.types, ["Creature", "Elf", "Shaman"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(1)));
    assert_eq!(face.colors(), vec![Color::Green]);
    assert!(face.activated_abilities.is_empty());
    assert!(face.static_abilities.is_empty());

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Reclamation Sage must have exactly one triggered ability");
    };
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(ability.may);
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
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
    let targeting = ability.targeting.as_ref().expect("destroy target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("Reclamation Sage must own exactly one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target artifact or enchantment");
}

#[test]
fn issue_336_gold_pan_keeps_treasure_anthem_and_equip() {
    let definition = CardRegistry::global().get("gold_pan").expect("Gold Pan");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{2}");
    assert_eq!(face.types, ["Artifact", "Equipment"]);
    assert!(face.triggered_abilities.len() == 1);
    assert!(face.static_abilities.len() == 1);
    assert!(face.activated_abilities.len() == 1);

    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Gold Pan must have exactly one ETB trigger");
    };
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(!trigger.may);
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        trigger.effect,
        [SpellEffectKind::CreateTokens {
            token: "treasure".into(),
            count: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        }]
    );
    assert!(trigger.targeting.is_none());

    let [static_ability] = face.static_abilities.as_slice() else {
        panic!("Gold Pan must keep exactly one attached modifier");
    };
    assert_eq!(
        static_ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(matches!(
        static_ability.definition,
        StaticAbilityDef::AttachedModifier {
            delta_power: 1,
            delta_toughness: 1,
            ..
        }
    ));

    let [equip] = face.activated_abilities.as_slice() else {
        panic!("Gold Pan must keep exactly one Equip activation");
    };
    assert_eq!(
        equip.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(
        equip.costs,
        [AbilityCost::Mana(
            ManaCost::parse("{1}").expect("fixed mana cost")
        )]
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
fn issue_336_fingerprint_rows_match_the_presentation_registry() {
    let registry = CardRegistry::global();
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    let expected_rows = [
        ("affectionate_indrik", "affectionate_indrik"),
        ("arcane_epiphany", "arcane_epiphany"),
        ("gold_pan", "gold_pan"),
        ("graveshifter", "graveshifter"),
        ("magic_pot", "magic_pot"),
        ("reclamation_sage", "reclamation_sage"),
        ("spectral_sailor", "spectral_sailor"),
        ("obyras_attendants_desperate_parry", "obyra_s_attendants"),
        ("obyras_attendants_desperate_parry", "desperate_parry"),
    ];
    for (id, face_id) in expected_rows {
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
