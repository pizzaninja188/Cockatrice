//! Issue #371 registry conformance for the eight retained graveyard-count token identities.
//!
//! Each card is generated from one exact typed recipe: public graveyard card counts (CR 404.2)
//! feed `CountExpression::GraveyardCards`, inclusive thresholds feed
//! `GameCondition::GraveyardAggregate`, the printed "instead" substitution uses the shipped
//! mandatory `FirstApplicable` branch over the face cast snapshot (CR 601.2/608.2), and the
//! activated/triggered surfaces keep their printed timing and costs. Broodspinner stays excluded:
//! its distinct-card-types quantity has no token-count consumer (blocker #364).

use tricerules_cards::primitives::{
    CardResultAction, CardResultSource, CardTypeFilter, GraveyardDestination, PlayerRecipient,
    ResolutionBranchRequirement, ResolutionBranchSelection, ResolutionCost, ZoneCardFilter,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, AbilitySourceZone, Amount, CardRegistry, CastTriggerPlayer,
    Color, GameCondition, GraveyardAggregate, ManaCost, RelativePlayerSet, SpellCastOrigin,
    SpellEffectKind, TriggerCondition,
};

fn creature_filter() -> ZoneCardFilter {
    ZoneCardFilter {
        card_type: Some(CardTypeFilter::Creature),
        ..ZoneCardFilter::default()
    }
}

fn artifact_or_creature_filter() -> ZoneCardFilter {
    ZoneCardFilter {
        any_of: Some(vec![
            ZoneCardFilter {
                card_type: Some(CardTypeFilter::Artifact),
                ..ZoneCardFilter::default()
            },
            creature_filter(),
        ]),
        ..ZoneCardFilter::default()
    }
}

fn graveyard_count(filter: ZoneCardFilter) -> Amount {
    Amount::Count(
        tricerules_cards::primitives::CountExpression::GraveyardCards {
            owners: RelativePlayerSet::Controller,
            filter: Some(filter),
        },
    )
}

fn graveyard_gate(
    aggregate: GraveyardAggregate,
    filter: Option<ZoneCardFilter>,
    min: u32,
) -> GameCondition {
    GameCondition::GraveyardAggregate {
        owners: RelativePlayerSet::Controller,
        aggregate,
        filter,
        min: Some(min),
        max: None,
    }
}

fn create_tokens(token: &str, count: Amount, tapped: bool) -> SpellEffectKind {
    SpellEffectKind::CreateTokens {
        token: token.into(),
        count,
        who: PlayerRecipient::Controller,
        tapped,
        sacrifice_timing: None,
    }
}

#[test]
fn issue_371_registers_the_eight_completed_identities() {
    let registry = CardRegistry::global();
    for (id, name, face_id, mana_cost, supertypes, types, stats, keywords) in [
        (
            "aatchik,_emerald_radian",
            "Aatchik, Emerald Radian",
            "aatchik_emerald_radian",
            "{3}{B}{B}{G}",
            vec!["Legendary"],
            vec!["Creature", "Insect", "Druid"],
            Some((3, 3)),
            vec![],
        ),
        (
            "arnim_zola,_bio-fanatic",
            "Arnim Zola, Bio-Fanatic",
            "arnim_zola_bio_fanatic",
            "{2}{B}",
            vec!["Legendary"],
            vec!["Artifact", "Creature", "Scientist", "Villain"],
            Some((2, 3)),
            vec![],
        ),
        (
            "hydra_troopers",
            "HYDRA Troopers",
            "hydra_troopers",
            "{2}{B}",
            vec![],
            vec!["Creature", "Human", "Soldier", "Villain"],
            Some((3, 2)),
            vec![],
        ),
        (
            "kiora,_the_rising_tide",
            "Kiora, the Rising Tide",
            "kiora_the_rising_tide",
            "{2}{U}",
            vec!["Legendary"],
            vec!["Creature", "Merfolk", "Noble"],
            Some((3, 2)),
            vec![],
        ),
        (
            "lluwen,_imperfect_naturalist",
            "Lluwen, Imperfect Naturalist",
            "lluwen_imperfect_naturalist",
            "{B/G}{B/G}",
            vec!["Legendary"],
            vec!["Creature", "Elf", "Druid"],
            Some((1, 3)),
            vec![],
        ),
        (
            "morcants_eyes",
            "Morcant's Eyes",
            "morcant_s_eyes",
            "{1}{G}",
            vec![],
            vec!["Kindred", "Enchantment", "Elf"],
            None,
            vec![],
        ),
        (
            "revenge_of_the_rats",
            "Revenge of the Rats",
            "revenge_of_the_rats",
            "{2}{B}{B}",
            vec![],
            vec!["Sorcery"],
            None,
            vec![],
        ),
        (
            "the_final_days",
            "The Final Days",
            "the_final_days",
            "{2}{B}{B}",
            vec![],
            vec!["Sorcery"],
            None,
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
        assert_eq!(face.supertypes, supertypes);
        assert_eq!(face.types, types);
        assert_eq!(face.power.zip(face.toughness), stats);
        assert_eq!(face.keywords, keywords);
    }
}

#[test]
fn issue_371_excludes_broodspinner() {
    let registry = CardRegistry::global();
    assert!(
        registry.get("broodspinner").is_none(),
        "Broodspinner's distinct-card-types quantity is blocker #364 and must stay unretained"
    );
    assert_eq!(registry.id_for_name("Broodspinner"), None);
}

#[test]
fn issue_371_aatchik_payloads_are_exact() {
    let face = CardRegistry::global()
        .get("aatchik,_emerald_radian")
        .expect("Aatchik, Emerald Radian")
        .primary_face();
    assert_eq!(face.triggered_abilities.len(), 2);

    let etb = &face.triggered_abilities[0];
    assert_eq!(etb.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(etb.presentation, AbilityPresentation::OracleLines(vec![1]));
    assert_eq!(
        etb.effect,
        [create_tokens(
            "insect_g_1_1",
            graveyard_count(artifact_or_creature_filter()),
            false,
        )]
    );

    let dies = &face.triggered_abilities[1];
    assert_eq!(dies.presentation, AbilityPresentation::OracleLines(vec![2]));
    let TriggerCondition::WheneverCreatureDies { controller, filter } = &dies.trigger else {
        panic!("Aatchik's second ability must watch creature deaths");
    };
    assert_eq!(*controller, CastTriggerPlayer::Controller);
    assert_eq!(filter.required_subtypes, ["Insect"]);
    assert!(
        filter.exclude_source,
        "the printed wording is another Insect"
    );
    assert_eq!(
        dies.effect,
        [
            SpellEffectKind::PutCounters {
                counter: tricerules_cards::CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                subject: tricerules_cards::primitives::EffectSubject::Source,
            },
            SpellEffectKind::LoseLife {
                amount: tricerules_cards::primitives::LifeAmount::Fixed(1),
                who: PlayerRecipient::EachOpponent,
            },
        ]
    );
}

#[test]
fn issue_371_arnim_zola_activation_uses_the_shared_villain_gate() {
    let face = CardRegistry::global()
        .get("arnim_zola,_bio-fanatic")
        .expect("Arnim Zola, Bio-Fanatic")
        .primary_face();
    let ability = &face.activated_abilities[0];
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(
        ability.costs,
        [
            AbilityCost::Mana(ManaCost::parse("{3}").expect("printed mana cost")),
            AbilityCost::Tap,
        ]
    );
    assert!(ability.targeting.is_none());
    assert_eq!(
        ability.conditions,
        [graveyard_gate(
            GraveyardAggregate::CardCount,
            Some(creature_filter()),
            2,
        )]
    );
    assert_eq!(
        ability.effect,
        [create_tokens(
            "villain_b_2_1_menace",
            Amount::Fixed(1),
            true
        )]
    );
}

#[test]
fn issue_371_hydra_troopers_branches_villain_or_mill() {
    let face = CardRegistry::global()
        .get("hydra_troopers")
        .expect("HYDRA Troopers")
        .primary_face();
    let ability = &face.triggered_abilities[0];
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(!ability.may);
    let [SpellEffectKind::ChooseResolutionBranch {
        chooser,
        optional,
        selection,
        branches,
        otherwise,
    }] = ability.effect.as_slice()
    else {
        panic!("HYDRA Troopers must emit one mandatory first-applicable branch");
    };
    assert_eq!(*chooser, PlayerRecipient::Controller);
    assert!(!optional);
    assert_eq!(*selection, ResolutionBranchSelection::FirstApplicable);
    assert!(otherwise.is_empty());
    assert_eq!(branches.len(), 2);

    assert_eq!(branches[0].branch_id.as_str(), "villain");
    assert_eq!(branches[0].cost, ResolutionCost::None);
    assert_eq!(
        branches[0].requirement,
        ResolutionBranchRequirement::GameCondition(graveyard_gate(
            GraveyardAggregate::CardCount,
            Some(creature_filter()),
            2,
        ))
    );
    assert_eq!(
        branches[0].effects,
        [create_tokens(
            "villain_b_2_1_menace",
            Amount::Fixed(1),
            true
        )]
    );

    assert_eq!(branches[1].branch_id.as_str(), "otherwise");
    assert_eq!(branches[1].cost, ResolutionCost::None);
    assert_eq!(branches[1].requirement, ResolutionBranchRequirement::Always);
    assert_eq!(
        branches[1].effects,
        [SpellEffectKind::Mill {
            count: Amount::Fixed(2),
            who: PlayerRecipient::Controller,
        }]
    );
}

#[test]
fn issue_371_kiora_etb_and_threshold_attack_are_exact() {
    let face = CardRegistry::global()
        .get("kiora,_the_rising_tide")
        .expect("Kiora, the Rising Tide")
        .primary_face();
    assert_eq!(face.triggered_abilities.len(), 2);

    let etb = &face.triggered_abilities[0];
    assert_eq!(etb.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(etb.presentation, AbilityPresentation::OracleLines(vec![1]));
    assert_eq!(
        etb.effect,
        [SpellEffectKind::DrawDiscard {
            who: PlayerRecipient::Controller,
            draw_count: 2,
            discard_count: 2,
            order: tricerules_cards::primitives::DrawDiscardOrder::DrawThenDiscard,
            optional: false,
        }]
    );

    let attack = &face.triggered_abilities[1];
    assert_eq!(
        attack.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0,
        }
    );
    assert_eq!(
        attack.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(attack.may, "the printed Scion creation is optional");
    assert_eq!(
        attack.intervening_if,
        Some(GameCondition::GraveyardAggregate {
            owners: RelativePlayerSet::Controller,
            aggregate: GraveyardAggregate::CardCount,
            filter: None,
            min: Some(7),
            max: None,
        })
    );
    assert_eq!(
        attack.effect,
        [create_tokens("scion_of_the_deep", Amount::Fixed(1), false)]
    );
}

#[test]
fn issue_371_lluwen_and_morcant_payloads_are_exact() {
    let registry = CardRegistry::global();

    let lluwen = registry
        .get("lluwen,_imperfect_naturalist")
        .expect("Lluwen, Imperfect Naturalist")
        .primary_face();
    let activated = &lluwen.activated_abilities[0];
    assert_eq!(
        activated.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        activated.costs,
        [
            AbilityCost::Mana(ManaCost::parse("{2}{B/G}{B/G}{B/G}").expect("printed hybrid cost")),
            AbilityCost::Tap,
            AbilityCost::DiscardCard {
                filter: CardTypeFilter::Land,
            },
        ]
    );
    assert_eq!(
        activated.effect,
        [create_tokens(
            "worm_bg_1_1",
            graveyard_count(ZoneCardFilter {
                card_type: Some(CardTypeFilter::Land),
                ..ZoneCardFilter::default()
            }),
            false,
        )]
    );
    let etb = &lluwen.triggered_abilities[0];
    assert_eq!(etb.presentation, AbilityPresentation::OracleLines(vec![1]));
    assert_eq!(etb.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    let [SpellEffectKind::Mill { count, who }, SpellEffectKind::ChooseGraveyardCard {
        filter,
        destination,
        optional,
        from_result,
    }] = etb.effect.as_slice()
    else {
        panic!(
            "Lluwen's ETB must mill then optionally top a milled card: {:?}",
            etb.effect
        );
    };
    assert_eq!(*count, Amount::Fixed(4));
    assert_eq!(*who, PlayerRecipient::Controller);
    assert_eq!(*destination, GraveyardDestination::LibraryTop);
    assert!(optional, "the printed put-on-top is a you-may choice");
    assert_eq!(
        filter.any_of.as_ref().map(Vec::len),
        Some(2),
        "the choice is a creature or land card"
    );
    let from_result = from_result.as_ref().expect("milled cohort restriction");
    assert_eq!(from_result.source, CardResultSource::PreviousEffect);
    assert_eq!(from_result.action, CardResultAction::Mill);
    assert_eq!(from_result.players, RelativePlayerSet::Controller);
    assert_eq!(from_result.card_type, None);

    let morcant = registry
        .get("morcants_eyes")
        .expect("Morcant's Eyes")
        .primary_face();
    let upkeep = &morcant.triggered_abilities[0];
    assert_eq!(
        upkeep.trigger,
        TriggerCondition::AtBeginningOfUpkeep {
            player: CastTriggerPlayer::Controller,
        }
    );
    assert_eq!(
        upkeep.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    let sacrifice = &morcant.activated_abilities[0];
    assert_eq!(
        sacrifice.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(sacrifice.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(
        sacrifice.costs,
        [
            AbilityCost::Mana(ManaCost::parse("{4}{G}{G}").expect("printed mana cost")),
            AbilityCost::SacrificeSelf,
        ]
    );
    assert_eq!(
        sacrifice.timing,
        tricerules_cards::ActivationTiming::SorcerySpeed
    );
    assert_eq!(
        sacrifice.effect,
        [create_tokens(
            "elf_bg_2_2",
            graveyard_count(ZoneCardFilter {
                required_subtypes: vec!["Elf".into()],
                ..ZoneCardFilter::default()
            }),
            false,
        )]
    );
}

#[test]
fn issue_371_revenge_and_final_days_payloads_are_exact() {
    let registry = CardRegistry::global();

    let revenge = registry
        .get("revenge_of_the_rats")
        .expect("Revenge of the Rats")
        .primary_face();
    assert_eq!(
        revenge.flashback_cost.as_ref().map(ToString::to_string),
        Some("{2}{B}{B}".to_string())
    );
    assert_eq!(
        revenge.spell_effect,
        [create_tokens(
            "rat_b_1_1",
            graveyard_count(creature_filter()),
            true
        )]
    );

    let final_days = registry
        .get("the_final_days")
        .expect("The Final Days")
        .primary_face();
    assert_eq!(
        final_days.flashback_cost.as_ref().map(ToString::to_string),
        Some("{4}{B}{B}".to_string())
    );
    assert_eq!(
        final_days.cast_conditions,
        [GameCondition::CastOrigin {
            origin: SpellCastOrigin::Graveyard,
        }]
    );
    let [SpellEffectKind::ChooseResolutionBranch {
        chooser,
        optional,
        selection,
        branches,
        otherwise,
    }] = final_days.spell_effect.as_slice()
    else {
        panic!("The Final Days must emit one mandatory first-applicable branch");
    };
    assert_eq!(*chooser, PlayerRecipient::Controller);
    assert!(!optional);
    assert_eq!(*selection, ResolutionBranchSelection::FirstApplicable);
    assert!(otherwise.is_empty());
    assert_eq!(branches[0].branch_id.as_str(), "cast_from_graveyard");
    assert_eq!(
        branches[0].requirement,
        ResolutionBranchRequirement::GameCondition(GameCondition::CastSnapshot { index: 0 })
    );
    assert_eq!(
        branches[0].effects,
        [create_tokens(
            "horror_b_2_2",
            graveyard_count(creature_filter()),
            true,
        )]
    );
    assert_eq!(branches[1].branch_id.as_str(), "cast_otherwise");
    assert_eq!(branches[1].requirement, ResolutionBranchRequirement::Always);
    assert_eq!(
        branches[1].effects,
        [create_tokens("horror_b_2_2", Amount::Fixed(2), true)]
    );
}

#[test]
fn issue_371_tokens_are_registered_with_exact_characteristics() {
    let registry = CardRegistry::global();
    for (id, name, types, supertypes, colors, stats, keywords) in [
        (
            "insect_g_1_1",
            "Insect",
            vec!["Creature", "Insect"],
            vec![],
            vec![Color::Green],
            (1, 1),
            vec![],
        ),
        (
            "worm_bg_1_1",
            "Worm",
            vec!["Creature", "Worm"],
            vec![],
            vec![Color::Black, Color::Green],
            (1, 1),
            vec![],
        ),
        (
            "elf_bg_2_2",
            "Elf",
            vec!["Creature", "Elf"],
            vec![],
            vec![Color::Black, Color::Green],
            (2, 2),
            vec![],
        ),
        (
            "rat_b_1_1",
            "Rat",
            vec!["Creature", "Rat"],
            vec![],
            vec![Color::Black],
            (1, 1),
            vec![],
        ),
        (
            "horror_b_2_2",
            "Horror",
            vec!["Creature", "Horror"],
            vec![],
            vec![Color::Black],
            (2, 2),
            vec![],
        ),
        (
            "scion_of_the_deep",
            "Scion of the Deep",
            vec!["Creature", "Octopus"],
            vec!["Legendary"],
            vec![Color::Blue],
            (8, 8),
            vec![],
        ),
    ] {
        assert!(registry.is_token(id), "{id} must be a token identity");
        let token = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing token {id}"));
        assert_eq!(token.primary_face().name, name);
        assert_eq!(token.primary_face().types, types);
        assert_eq!(token.primary_face().supertypes, supertypes);
        assert_eq!(token.primary_face().colors_override, Some(colors));
        assert_eq!(
            token
                .primary_face()
                .power
                .zip(token.primary_face().toughness),
            Some(stats)
        );
        assert_eq!(token.primary_face().keywords, keywords);
    }

    // The printed Rat has no combat restriction, so the existing can't-block identity stays
    // unused by this cohort.
    let rat = registry.get("rat_b_1_1").expect("Rat token").primary_face();
    assert!(rat.static_abilities.is_empty());
    // The reusable Villain identity is shared rather than duplicated.
    assert!(registry.is_token("villain_b_2_1_menace"));

    // Guard the shared graveyard filter shape used by the Aatchik recipe against drift.
    let counted = graveyard_count(artifact_or_creature_filter());
    let Amount::Count(tricerules_cards::primitives::CountExpression::GraveyardCards {
        filter, ..
    }) = counted
    else {
        panic!("Aatchik must count graveyard cards");
    };
    assert_eq!(
        filter
            .as_ref()
            .and_then(|f| f.any_of.as_ref())
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn issue_371_fingerprint_rows_match_the_presentation_registry() {
    let registry = CardRegistry::global();
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, face_id) in [
        ("aatchik,_emerald_radian", "aatchik_emerald_radian"),
        ("arnim_zola,_bio-fanatic", "arnim_zola_bio_fanatic"),
        ("hydra_troopers", "hydra_troopers"),
        ("kiora,_the_rising_tide", "kiora_the_rising_tide"),
        (
            "lluwen,_imperfect_naturalist",
            "lluwen_imperfect_naturalist",
        ),
        ("morcants_eyes", "morcant_s_eyes"),
        ("revenge_of_the_rats", "revenge_of_the_rats"),
        ("the_final_days", "the_final_days"),
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
