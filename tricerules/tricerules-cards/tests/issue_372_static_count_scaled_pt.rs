//! Issue #372 registry conformance for the four retained count-scaled static P/T identities.
//!
//! Xande and Moon-Vigil Adherents author `StaticAbilityDef::CountScaledSelfPt` over a public
//! graveyard count (CR 404.2, 613.4c); Song of Stupefaction authors the attached-scope
//! `AttachedModifier` count; Neo Exdeath authors a `CountScaledPowerToughness` CDA that defines
//! only power (CR 208.2a, 604.3). The ordinary printed clauses are covered by exact recipes too:
//! the Aura enchant line, Song's optional entry mill, and Exdeath's end-step transform trigger.
//! Avatar Destiny's attached count recipe is catalog-tested, but the identity stays unretained
//! because its dies clause needs an unshipped attached-object last-known-power amount and a
//! source-bound graveyard-to-hand return. Cid, Timeless Artificer stays blocked by #368.

use tricerules_cards::primitives::{
    BattlefieldCreatureCountFilter, CardTypeFilter, CountExpression, GraveyardAggregate,
    QuantityTerm, RelativePlayerSet, SpellEffectKind, StaticAbilityDef, TargetFilter, TargetKind,
    TypeLineAddition, ZoneCardFilter,
};
use tricerules_cards::{
    AbilityPresentation, CardRegistry, CharacteristicDefiningAbility, Keyword, TriggerCondition,
};

fn graveyard_creature_card_filter() -> ZoneCardFilter {
    ZoneCardFilter {
        card_type: Some(CardTypeFilter::Creature),
        ..ZoneCardFilter::default()
    }
}

fn permanent_card_filter() -> ZoneCardFilter {
    ZoneCardFilter {
        excluded_card_types: vec![CardTypeFilter::Instant, CardTypeFilter::Sorcery],
        ..ZoneCardFilter::default()
    }
}

#[test]
fn issue_372_registers_the_four_completed_identities() {
    let registry = CardRegistry::global();
    for (id, name, face_id, mana_cost, types, supertypes, stats, keywords) in [
        (
            "xande,_dark_mage",
            "Xande, Dark Mage",
            "xande_dark_mage",
            "{2}{U}{B}",
            vec!["Creature", "Human", "Wizard"],
            vec!["Legendary"],
            (Some(3), Some(3)),
            vec![Keyword::Menace],
        ),
        (
            "moon-vigil_adherents",
            "Moon-Vigil Adherents",
            "moon_vigil_adherents",
            "{2}{G}{G}",
            vec!["Creature", "Elf", "Druid"],
            vec![],
            (Some(0), Some(0)),
            vec![Keyword::Trample],
        ),
        (
            "song_of_stupefaction",
            "Song of Stupefaction",
            "song_of_stupefaction",
            "{1}{U}",
            vec!["Enchantment", "Aura"],
            vec![],
            (None, None),
            vec![],
        ),
        (
            "exdeath,_void_warlock_neo_exdeath,_dimensions_end",
            "Exdeath, Void Warlock // Neo Exdeath, Dimension's End",
            "exdeath_void_warlock",
            "{1}{B}{G}",
            vec!["Creature", "Spirit", "Warlock"],
            vec!["Legendary"],
            (Some(3), Some(3)),
            vec![],
        ),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing retained card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), face_id);
        assert_eq!(face.mana_cost.to_string(), mana_cost);
        assert_eq!(face.types, types);
        assert_eq!(face.supertypes, supertypes);
        assert_eq!((face.power, face.toughness), stats);
        assert_eq!(face.keywords, keywords);
    }
}

#[test]
fn issue_372_excludes_the_unretained_identities() {
    let registry = CardRegistry::global();
    for name in ["Avatar Destiny", "Cid, Timeless Artificer"] {
        assert_eq!(
            registry.id_for_name(name),
            None,
            "{name} has an unsupported clause and must stay unretained"
        );
    }
}

#[test]
fn issue_372_xande_payload_and_presentation_are_exact() {
    let definition = CardRegistry::global()
        .get("xande,_dark_mage")
        .expect("Xande, Dark Mage");
    let face = definition.primary_face();
    let [ability] = face.static_abilities.as_slice() else {
        panic!("Xande must have exactly one static ability");
    };
    assert_eq!(ability.ability_id.as_str(), "static_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.definition,
        StaticAbilityDef::CountScaledSelfPt {
            count: CountExpression::GraveyardCards {
                owners: RelativePlayerSet::Controller,
                filter: Some(ZoneCardFilter {
                    excluded_card_types: vec![CardTypeFilter::Creature, CardTypeFilter::Land],
                    ..ZoneCardFilter::default()
                }),
            },
            power_per_match: 1,
            toughness_per_match: 1,
        }
    );
}

#[test]
fn issue_372_moon_vigil_payload_and_presentation_are_exact() {
    let definition = CardRegistry::global()
        .get("moon-vigil_adherents")
        .expect("Moon-Vigil Adherents");
    let face = definition.primary_face();
    let [ability] = face.static_abilities.as_slice() else {
        panic!("Moon-Vigil Adherents must have exactly one static ability");
    };
    assert_eq!(ability.ability_id.as_str(), "static_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.definition,
        StaticAbilityDef::CountScaledSelfPt {
            count: CountExpression::Affine {
                constant: 0,
                terms: vec![
                    QuantityTerm {
                        coefficient: 1,
                        quantity: CountExpression::BattlefieldCreatures {
                            filter: BattlefieldCreatureCountFilter {
                                controllers: RelativePlayerSet::Controller,
                                ..BattlefieldCreatureCountFilter::default()
                            },
                        },
                    },
                    QuantityTerm {
                        coefficient: 1,
                        quantity: CountExpression::GraveyardCards {
                            owners: RelativePlayerSet::Controller,
                            filter: Some(graveyard_creature_card_filter()),
                        },
                    },
                ],
            },
            power_per_match: 1,
            toughness_per_match: 1,
        }
    );
}

#[test]
fn issue_372_song_payload_and_presentation_are_exact() {
    let definition = CardRegistry::global()
        .get("song_of_stupefaction")
        .expect("Song of Stupefaction");
    let face = definition.primary_face();
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::AuraAttach {
            target: TargetFilter {
                any_of: Some(vec![
                    TargetFilter {
                        kind: TargetKind::Creature,
                        ..TargetFilter::default()
                    },
                    TargetFilter {
                        kind: TargetKind::AnyPermanent,
                        required_subtypes: vec!["Vehicle".to_string()],
                        ..TargetFilter::default()
                    },
                ]),
                ..TargetFilter::default()
            },
        }]
    );

    let [triggered] = face.triggered_abilities.as_slice() else {
        panic!("Song of Stupefaction must have exactly one triggered ability");
    };
    assert_eq!(
        triggered.trigger,
        TriggerCondition::WhenSelfEntersBattlefield
    );
    assert_eq!(
        triggered.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(triggered.may);
    assert_eq!(
        triggered.effect,
        [SpellEffectKind::Mill {
            count: tricerules_cards::Amount::Fixed(2),
            who: tricerules_cards::primitives::PlayerRecipient::Controller,
        }]
    );

    let [static_ability] = face.static_abilities.as_slice() else {
        panic!("Song of Stupefaction must have exactly one static ability");
    };
    assert_eq!(
        static_ability.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(
        static_ability.definition,
        StaticAbilityDef::AttachedModifier {
            condition: None,
            add_types: TypeLineAddition::default(),
            set_types: None,
            set_name: None,
            set_colors: None,
            delta_power: 0,
            delta_toughness: 0,
            count: Some(CountExpression::GraveyardCards {
                owners: RelativePlayerSet::Controller,
                filter: Some(permanent_card_filter()),
            }),
            power_per_match: -1,
            toughness_per_match: 0,
            set_power: None,
            set_toughness: None,
            remove_all_abilities: false,
            keywords: Vec::new(),
            triggered_abilities: Vec::new(),
            activated_abilities: Vec::new(),
            restriction: Default::default(),
            doesnt_untap_during_untap_step: false,
            cant_untap: false,
        }
    );
}

#[test]
fn issue_372_exdeath_payload_and_presentation_are_exact() {
    let definition = CardRegistry::global()
        .get("exdeath,_void_warlock_neo_exdeath,_dimensions_end")
        .expect("Exdeath, Void Warlock // Neo Exdeath, Dimension's End");
    let [front, back] = definition.faces.as_slice() else {
        panic!("Exdeath must register exactly two faces");
    };

    let [etb, transform] = front.triggered_abilities.as_slice() else {
        panic!("the front face must have exactly two triggered abilities");
    };
    assert_eq!(etb.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(etb.presentation, AbilityPresentation::OracleLines(vec![1]));
    assert_eq!(
        etb.effect,
        [SpellEffectKind::GainLife {
            amount: tricerules_cards::Amount::Fixed(3),
        }]
    );
    assert_eq!(transform.ability_id.as_str(), "triggered_02");
    assert_eq!(
        transform.trigger,
        TriggerCondition::AtBeginningOfEndStep {
            player: tricerules_cards::CastTriggerPlayer::Controller,
        }
    );
    assert_eq!(
        transform.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(!transform.may);
    assert_eq!(
        transform.intervening_if,
        Some(tricerules_cards::GameCondition::GraveyardAggregate {
            owners: RelativePlayerSet::Controller,
            aggregate: GraveyardAggregate::CardCount,
            filter: Some(permanent_card_filter()),
            min: Some(6),
            max: None,
        })
    );
    assert_eq!(
        transform.effect,
        [SpellEffectKind::ChangeSourceFace {
            action: tricerules_cards::primitives::FaceChangeAction::Transform,
        }]
    );

    assert_eq!(back.face_id.as_str(), "neo_exdeath_dimension_s_end");
    assert_eq!(back.power, None, "the printed `*` power is CDA-defined");
    assert_eq!(back.toughness, Some(3));
    assert_eq!(back.keywords, [Keyword::Trample]);
    let [cda] = back.characteristic_defining_abilities.as_slice() else {
        panic!("the back face must have exactly one characteristic-defining ability");
    };
    assert_eq!(cda.presentation, AbilityPresentation::OracleLines(vec![2]));
    assert_eq!(
        cda.definition,
        CharacteristicDefiningAbility::CountScaledPowerToughness {
            count: CountExpression::GraveyardCards {
                owners: RelativePlayerSet::Controller,
                filter: Some(permanent_card_filter()),
            },
            power_per_match: 1,
            toughness_per_match: 0,
        }
    );
}
