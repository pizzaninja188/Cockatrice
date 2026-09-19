//! Issue #425 registry conformance for the six retained `Choose one or both —` identities.
//!
//! Every generated identity must expose its exact printed mana cost, types, and two-mode
//! `Choose one or both —` structure (CR 700.2). Modal-only bullets never leak into ordinary
//! `spell_effect` slots, and the four cohort cards whose printed modes lack shipped vocabulary
//! stay unregistered.

use tricerules_cards::primitives::{
    Amount, CardTypeFilter, EffectSubject, GraveyardDestination, GraveyardFilter,
    PermanentTypeFilter, PlayerRecipient, PowerToughnessCharacteristic, PtScale, PtScaleBasis,
    SearchDestination, SearchZoneSelection, SpellEffectKind, TargetController, TargetFilter,
    TargetKind, TargetSchema, ZoneCardFilter,
};
use tricerules_cards::{CardRegistry, ModalDef};

fn modal(card_id: &str) -> ModalDef {
    let definition = CardRegistry::global()
        .get(card_id)
        .unwrap_or_else(|| panic!("{card_id} is registered"));
    let face = definition.primary_face();
    assert!(
        face.spell_effect.is_empty(),
        "{card_id} keeps every bullet out of the ordinary spell-effect slot"
    );
    assert!(
        face.targeting.is_none(),
        "{card_id} keeps every bullet out of the face-level targeting slot"
    );
    let modal = face
        .modal_spell
        .clone()
        .unwrap_or_else(|| panic!("{card_id} is a modal spell"));
    for mode in &modal.modes {
        TargetSchema::compile(&mode.effects, mode.targeting.as_ref()).unwrap_or_else(|error| {
            panic!("{card_id} mode {} target schema: {error}", mode.mode_id)
        });
    }
    modal
}

#[test]
fn issue_425_registers_the_six_choose_both_identities() {
    let registry = CardRegistry::global();
    for (id, name, mana_cost, types) in [
        (
            "amazing_acrobatics",
            "Amazing Acrobatics",
            "{1}{U}{U}",
            &["Instant"][..],
        ),
        (
            "avengers_disassembled",
            "Avengers Disassembled",
            "{1}{R}{R}",
            &["Sorcery"][..],
        ),
        ("decoy_ploy", "Decoy Ploy", "{1}{B}", &["Instant"][..]),
        ("epic_fight", "Epic Fight", "{2}{G}", &["Sorcery"][..]),
        (
            "pinecone_strike",
            "Pinecone Strike",
            "{1}{R}",
            &["Instant"][..],
        ),
        (
            "scour_for_scrap",
            "Scour for Scrap",
            "{3}{U}",
            &["Instant"][..],
        ),
    ] {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {name}"));
        assert_eq!(definition.name, name, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id, "{id}");
        assert_eq!(face.mana_cost.to_string(), mana_cost, "{id}");
        assert_eq!(
            face.types.iter().map(String::as_str).collect::<Vec<_>>(),
            types,
            "{id}"
        );
        assert!(face.keywords.is_empty(), "{id}");
        let modal = modal(id);
        assert_eq!((modal.min_modes, modal.max_modes), (1, 2), "{id}");
        assert!(modal.all_modes_cast_cost.is_none(), "{id}");
        assert_eq!(modal.modes.len(), 2, "{id}");
        assert_eq!(
            modal
                .modes
                .iter()
                .map(|mode| mode.mode_id.as_str())
                .collect::<Vec<_>>(),
            ["mode_01", "mode_02"],
            "{id}"
        );
    }

    for excluded in [
        "choreographed_sparks",
        "expose_the_culprit",
        "go_ninja_go",
        "perfect_intimidation",
    ] {
        assert!(
            registry.get(excluded).is_none(),
            "{excluded} must stay unregistered while a printed mode lacks shipped vocabulary"
        );
    }
}

#[test]
fn issue_425_tap_mode_is_bounded_to_one_or_two_creatures() {
    let acrobatics = modal("amazing_acrobatics");
    assert_eq!(
        acrobatics.modes[0].effects,
        [SpellEffectKind::CounterTargetSpell {
            spell_filter: tricerules_cards::primitives::StackSpellFilter::default(),
            unless_controller_pays: None,
            unless_controller_pays_by_cast_cost: None,
        }]
    );
    assert_eq!(
        acrobatics.modes[1].effects,
        [SpellEffectKind::Tap {
            subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
        }]
    );
    let group = &acrobatics.modes[1]
        .targeting
        .as_ref()
        .expect("tap group")
        .groups[0];
    assert_eq!((group.min, group.max), (1, 2));
    assert_eq!(group.prompt, "Choose one or two target creatures");
    assert_eq!(group.effect_indices, vec![0]);
    assert!(!group.same_graveyard);
}

#[test]
fn issue_425_avengers_disassembled_sweeps_and_searches_for_the_lands_controller() {
    let avengers = modal("avengers_disassembled");
    assert_eq!(
        avengers.modes[0].effects,
        [SpellEffectKind::DamageAll {
            amount: Amount::Fixed(3),
            players: tricerules_cards::primitives::RelativePlayerSet::All,
            kind: TargetFilter::default_creature(),
        }]
    );
    assert!(avengers.modes[0].targeting.is_none());

    assert_eq!(
        avengers.modes[1].effects,
        [
            SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::AnyPermanent,
                    permanent_types: vec![PermanentTypeFilter::Land],
                    ..TargetFilter::default()
                })),
            },
            SpellEffectKind::SearchLibrary {
                who: PlayerRecipient::ControllerOfTargetGroup { group_index: 0 },
                optional: true,
                count: 1,
                count_by_cast_cost: None,
                filter: Some(ZoneCardFilter {
                    card_type: Some(CardTypeFilter::BasicLand),
                    ..ZoneCardFilter::default()
                }),
                slots: Vec::new(),
                zones: SearchZoneSelection::default(),
                destination: SearchDestination::Battlefield { tapped: true },
                conditional_destination: None,
                shuffle: true,
                reveal: false,
                result_id: None,
            },
        ]
    );
    let group = &avengers.modes[1]
        .targeting
        .as_ref()
        .expect("land group")
        .groups[0];
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target land");
    assert_eq!(group.effect_indices, vec![0]);
}

#[test]
fn issue_425_decoy_ploy_returns_only_the_printed_subtype() {
    let decoy = modal("decoy_ploy");
    for (mode, subtype, prompt) in [
        (
            &decoy.modes[0],
            "Villain",
            "Choose target Villain card from your graveyard",
        ),
        (
            &decoy.modes[1],
            "Hero",
            "Choose target Hero card from your graveyard",
        ),
    ] {
        assert_eq!(
            mode.effects,
            [SpellEffectKind::MoveGraveyardCards {
                filter: GraveyardFilter {
                    card: Some(ZoneCardFilter {
                        required_subtypes: vec![subtype.into()],
                        ..ZoneCardFilter::default()
                    }),
                    ..GraveyardFilter::default()
                },
                destination: GraveyardDestination::Hand,
                linked_exile_id: None,
            }],
            "{subtype}"
        );
        let group = &mode.targeting.as_ref().expect("return group").groups[0];
        assert_eq!((group.min, group.max), (1, 1), "{subtype}");
        assert_eq!(group.prompt, prompt, "{subtype}");
    }
}

#[test]
fn issue_425_epic_fight_doubles_both_characteristics_on_one_target() {
    let epic = modal("epic_fight");
    assert_eq!(
        epic.modes[0].effects,
        [
            SpellEffectKind::PumpTarget {
                power: 0,
                toughness: 0,
                scale: Some(PtScale {
                    basis: PtScaleBasis::Subject(PowerToughnessCharacteristic::Power),
                    power_per_unit: 1,
                    toughness_per_unit: 0,
                }),
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            },
            SpellEffectKind::PumpTarget {
                power: 0,
                toughness: 0,
                scale: Some(PtScale {
                    basis: PtScaleBasis::Subject(PowerToughnessCharacteristic::Toughness),
                    power_per_unit: 0,
                    toughness_per_unit: 1,
                }),
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            },
        ]
    );
    let group = &epic.modes[0]
        .targeting
        .as_ref()
        .expect("double group")
        .groups[0];
    assert_eq!(group.effect_indices, vec![0, 1]);
    assert_eq!(group.prompt, "Choose target creature");

    assert_eq!(
        epic.modes[1].effects,
        [SpellEffectKind::Fight {
            first: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..TargetFilter::default()
            })),
            second: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::Opponent,
                ..TargetFilter::default()
            })),
        }]
    );
}

#[test]
fn issue_425_pinecone_strike_binds_the_rider_and_the_artifact_token() {
    let pinecone = modal("pinecone_strike");
    assert_eq!(
        pinecone.modes[0].effects,
        [
            SpellEffectKind::DamageTarget {
                amount: Amount::Fixed(3),
                target: TargetFilter::default_creature(),
            },
            SpellEffectKind::ExileIfWouldDieThisTurn {
                target: TargetFilter::default_creature(),
            },
        ]
    );
    let group = &pinecone.modes[0]
        .targeting
        .as_ref()
        .expect("damage group")
        .groups[0];
    assert_eq!(group.effect_indices, vec![0, 1]);

    assert_eq!(
        pinecone.modes[1].effects,
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::AnyPermanent,
                permanent_types: vec![PermanentTypeFilter::Artifact],
                token: Some(true),
                ..TargetFilter::default()
            })),
        }]
    );
    assert_eq!(
        pinecone.modes[1]
            .targeting
            .as_ref()
            .expect("artifact-token group")
            .groups[0]
            .prompt,
        "Choose target artifact token"
    );
}

#[test]
fn issue_425_scour_for_scrap_reveals_the_searched_artifact() {
    let scour = modal("scour_for_scrap");
    assert_eq!(
        scour.modes[0].effects,
        [SpellEffectKind::SearchLibrary {
            who: PlayerRecipient::Controller,
            optional: false,
            count: 1,
            count_by_cast_cost: None,
            filter: Some(ZoneCardFilter {
                card_type: Some(CardTypeFilter::Artifact),
                ..ZoneCardFilter::default()
            }),
            slots: Vec::new(),
            zones: SearchZoneSelection::default(),
            destination: SearchDestination::Hand,
            conditional_destination: None,
            shuffle: true,
            reveal: true,
            result_id: None,
        }]
    );
    assert!(scour.modes[0].targeting.is_none());
    assert_eq!(
        scour.modes[1].effects,
        [SpellEffectKind::MoveGraveyardCards {
            filter: GraveyardFilter {
                card: Some(ZoneCardFilter {
                    card_type: Some(CardTypeFilter::Artifact),
                    ..ZoneCardFilter::default()
                }),
                ..GraveyardFilter::default()
            },
            destination: GraveyardDestination::Hand,
            linked_exile_id: None,
        }]
    );
}
