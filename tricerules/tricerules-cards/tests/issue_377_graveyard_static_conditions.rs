//! Issue #377 registry conformance for the retained static graveyard-condition cohort.
//!
//! Seventeen Standard identities generate from the exact typed recipes: public graveyard state
//! (CR 404.2) continuously gates a layer-7c self modifier or a layer-7b base P/T (CR 611.3,
//! CR 613.4b), and the "unless" forms keep the complementary maximum bound. The cohort's
//! ordinary content (entry mill/surveil/tokens/life, upkeep surveil and optional mill, the
//! enters-or-attacks loot union, the hybrid activated surveil, and conditional Villain hexproof)
//! is covered by exact control-clause recipes. The other four cohort identities stay excluded
//! because a non-cohort printed clause still has no exact recipe; those cards must not appear in
//! the registry until their narrower blockers land.

use tricerules_cards::primitives::{
    CardTypeFilter, CombatRestriction, DrawDiscardOrder, PlayerRecipient, StaticAbilityDef,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, AbilitySourceZone, Amount, BattlefieldAggregate,
    CardRegistry, CastTriggerPlayer, GameCondition, GraveyardAggregate, Keyword,
    LibraryPartitionKind, ManaCost, RelativePlayerSet, SpellEffectKind, TriggerCondition,
    ZoneCardFilter,
};

fn permanent_card_filter() -> ZoneCardFilter {
    ZoneCardFilter {
        any_of: Some(
            [
                CardTypeFilter::Artifact,
                CardTypeFilter::Battle,
                CardTypeFilter::Creature,
                CardTypeFilter::Enchantment,
                CardTypeFilter::Land,
                CardTypeFilter::Planeswalker,
            ]
            .into_iter()
            .map(|card_type| ZoneCardFilter {
                card_type: Some(card_type),
                ..ZoneCardFilter::default()
            })
            .collect(),
        ),
        ..ZoneCardFilter::default()
    }
}

fn graveyard_condition(
    aggregate: GraveyardAggregate,
    filter: Option<ZoneCardFilter>,
    min: Option<u32>,
    max: Option<u32>,
) -> GameCondition {
    GameCondition::GraveyardAggregate {
        owners: RelativePlayerSet::Controller,
        aggregate,
        filter,
        min,
        max,
    }
}

fn self_modifier(definition: &StaticAbilityDef) -> (&GameCondition, i32, i32, &[Keyword]) {
    let StaticAbilityDef::ConditionalSelfModifier {
        condition,
        delta_power,
        delta_toughness,
        keywords,
        ..
    } = definition
    else {
        panic!("expected a conditional self modifier, got {definition:?}");
    };
    (condition, *delta_power, *delta_toughness, keywords)
}

#[test]
fn issue_377_registers_the_seventeen_retained_identities() {
    let registry = CardRegistry::global();
    for (id, name, face_id, mana_cost, types, keywords) in [
        (
            "akawalli,_the_seething_tower",
            "Akawalli, the Seething Tower",
            "akawalli_the_seething_tower",
            "{1}{B}{G}",
            vec!["Creature", "Fungus"],
            vec![],
        ),
        (
            "basking_capybara",
            "Basking Capybara",
            "basking_capybara",
            "{1}{G}",
            vec!["Creature", "Capybara"],
            vec![],
        ),
        (
            "billowing_shriekmass",
            "Billowing Shriekmass",
            "billowing_shriekmass",
            "{3}{B}",
            vec!["Creature", "Spirit"],
            vec![Keyword::Flying],
        ),
        (
            "cephalid_inkmage",
            "Cephalid Inkmage",
            "cephalid_inkmage",
            "{2}{U}",
            vec!["Creature", "Octopus", "Wizard"],
            vec![],
        ),
        (
            "didact_echo",
            "Didact Echo",
            "didact_echo",
            "{4}{U}",
            vec!["Creature", "Spirit", "Cleric"],
            vec![],
        ),
        (
            "doc_ock,_sinister_scientist",
            "Doc Ock, Sinister Scientist",
            "doc_ock_sinister_scientist",
            "{4}{U}",
            vec!["Creature", "Human", "Scientist", "Villain"],
            vec![],
        ),
        (
            "dreadwing_scavenger",
            "Dreadwing Scavenger",
            "dreadwing_scavenger",
            "{1}{U}{B}",
            vec!["Creature", "Nightmare", "Bird"],
            vec![Keyword::Flying],
        ),
        (
            "echo_of_dusk",
            "Echo of Dusk",
            "echo_of_dusk",
            "{1}{B}",
            vec!["Creature", "Vampire", "Spirit"],
            vec![],
        ),
        (
            "first-time_flyer",
            "First-Time Flyer",
            "first_time_flyer",
            "{1}{U}",
            vec!["Creature", "Human", "Pilot", "Ally"],
            vec![Keyword::Flying],
        ),
        (
            "frilled_cave-wurm",
            "Frilled Cave-Wurm",
            "frilled_cave_wurm",
            "{3}{U}",
            vec!["Creature", "Salamander", "Wurm"],
            vec![],
        ),
        (
            "ghitu_lavarunner",
            "Ghitu Lavarunner",
            "ghitu_lavarunner",
            "{R}",
            vec!["Creature", "Human", "Wizard"],
            vec![],
        ),
        (
            "mind_drill_assailant",
            "Mind Drill Assailant",
            "mind_drill_assailant",
            "{2}{U/B}{U/B}",
            vec!["Creature", "Rat", "Warlock"],
            vec![],
        ),
        (
            "mindwhisker",
            "Mindwhisker",
            "mindwhisker",
            "{2}{U}",
            vec!["Creature", "Rat", "Wizard"],
            vec![],
        ),
        (
            "patchwork_beastie",
            "Patchwork Beastie",
            "patchwork_beastie",
            "{G}",
            vec!["Artifact", "Creature", "Beast"],
            vec![],
        ),
        (
            "the_lion-turtle",
            "The Lion-Turtle",
            "the_lion_turtle",
            "{1}{G}{U}",
            vec!["Creature", "Elder", "Cat", "Turtle"],
            vec![Keyword::Reach, Keyword::Vigilance],
        ),
        (
            "the_swarmweaver",
            "The Swarmweaver",
            "the_swarmweaver",
            "{2}{B}{G}",
            vec!["Artifact", "Creature", "Scarecrow"],
            vec![],
        ),
        (
            "wildfire_wickerfolk",
            "Wildfire Wickerfolk",
            "wildfire_wickerfolk",
            "{R}{G}",
            vec!["Artifact", "Creature", "Scarecrow"],
            vec![Keyword::Haste],
        ),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing retained card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        assert_eq!(definition.layout, tricerules_cards::Layout::Normal);
        assert_eq!(definition.face_count(), 1);
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), face_id);
        assert_eq!(face.mana_cost.to_string(), mana_cost);
        assert_eq!(face.types, types);
        assert_eq!(face.keywords, keywords);
    }
}

#[test]
fn issue_377_excluded_identities_stay_unregistered() {
    let registry = CardRegistry::global();
    for blocked in [
        // Cohort identities that stay behind a narrower non-cohort clause blocker.
        "killmonger,_scourge_of_wakanda",
        "most_decrepit_old_bird_speak_secrets",
        "the_ancient_one",
        "waterlogged_hulk_watertight_gondola",
        // The two identities the cohort description excludes outright.
        "cavernous_maw",
        "undercover_skrull",
    ] {
        assert!(
            registry.get(blocked).is_none(),
            "{blocked} must stay excluded until its blocker lands"
        );
    }
}

#[test]
fn issue_377_control_clause_payloads_are_exact() {
    let registry = CardRegistry::global();

    let billowing = registry.get("billowing_shriekmass").expect("Billowing");
    let [SpellEffectKind::Mill {
        count: Amount::Fixed(3),
        who: PlayerRecipient::Controller,
    }] = billowing.primary_face().triggered_abilities[0]
        .effect
        .as_slice()
    else {
        panic!("Billowing Shriekmass must mill exactly three cards on entry");
    };

    let cephalid = registry.get("cephalid_inkmage").expect("Cephalid Inkmage");
    assert_eq!(
        cephalid.primary_face().triggered_abilities[0].effect,
        [SpellEffectKind::LibraryPartition {
            count: 3,
            top_min: 0,
            top_max: None,
            kind: LibraryPartitionKind::Surveil,
        }]
    );

    let mindwhisker = registry.get("mindwhisker").expect("Mindwhisker");
    let upkeep = &mindwhisker.primary_face().triggered_abilities[0];
    assert_eq!(
        upkeep.trigger,
        TriggerCondition::AtBeginningOfUpkeep {
            player: CastTriggerPlayer::Controller,
        }
    );
    assert!(!upkeep.may);

    let beastie = registry
        .get("patchwork_beastie")
        .expect("Patchwork Beastie");
    let upkeep_mill = &beastie.primary_face().triggered_abilities[0];
    assert!(upkeep_mill.may, "the printed mill is optional");
    assert_eq!(
        upkeep_mill.effect,
        [SpellEffectKind::Mill {
            count: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
        }]
    );

    let lion_turtle = registry.get("the_lion-turtle").expect("The Lion-Turtle");
    assert_eq!(
        lion_turtle.primary_face().triggered_abilities[0].effect,
        [SpellEffectKind::GainLife {
            amount: Amount::Fixed(3),
        }]
    );

    let dreadwing = registry
        .get("dreadwing_scavenger")
        .expect("Dreadwing Scavenger");
    let loot = &dreadwing.primary_face().triggered_abilities;
    assert_eq!(loot.len(), 2, "the union authors one trigger per event");
    assert_eq!(loot[0].trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        loot[1].trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0,
        }
    );
    for ability in loot {
        assert_eq!(
            ability.effect,
            [SpellEffectKind::DrawDiscard {
                who: PlayerRecipient::Controller,
                draw_count: 1,
                discard_count: 1,
                order: DrawDiscardOrder::DrawThenDiscard,
                optional: false,
            }]
        );
    }

    let mind_drill = registry
        .get("mind_drill_assailant")
        .expect("Mind Drill Assailant");
    let activation = &mind_drill.primary_face().activated_abilities[0];
    assert_eq!(activation.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(
        activation.costs,
        [AbilityCost::Mana(
            ManaCost::parse("{2}{U/B}").expect("printed hybrid cost")
        )]
    );
    assert_eq!(
        activation.effect,
        [SpellEffectKind::LibraryPartition {
            count: 1,
            top_min: 0,
            top_max: None,
            kind: LibraryPartitionKind::Surveil,
        }]
    );

    let doc_ock = registry
        .get("doc_ock,_sinister_scientist")
        .expect("Doc Ock, Sinister Scientist");
    let abilities = &doc_ock.primary_face().static_abilities;
    assert_eq!(abilities.len(), 2, "base P/T then conditional hexproof");
    let StaticAbilityDef::ConditionalSelfModifier {
        condition,
        keywords,
        ..
    } = &abilities[1].definition
    else {
        panic!("Doc Ock's second static ability must be the conditional modifier");
    };
    assert_eq!(keywords, &[Keyword::Hexproof]);
    let GameCondition::BattlefieldAggregate {
        filter,
        aggregate,
        min,
        max,
    } = condition
    else {
        panic!("Doc Ock must gate on a battlefield aggregate: {condition:?}");
    };
    assert_eq!(*aggregate, BattlefieldAggregate::Count);
    assert_eq!((*min, *max), (Some(1), None));
    assert_eq!(filter.required_subtypes, ["Villain"]);
    assert!(
        filter.exclude_source,
        "the printed wording is another Villain"
    );

    let swarmweaver = registry.get("the_swarmweaver").expect("The Swarmweaver");
    assert_eq!(
        swarmweaver.primary_face().triggered_abilities[0].effect,
        [SpellEffectKind::CreateTokens {
            token: "insect_bg_1_1_flying".into(),
            count: Amount::Fixed(2),
            who: PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        }]
    );
    assert_eq!(swarmweaver.primary_face().static_abilities.len(), 4);
}

#[test]
fn issue_377_akawalli_authors_three_ordered_static_abilities() {
    let definition = CardRegistry::global()
        .get("akawalli,_the_seething_tower")
        .expect("Akawalli, the Seething Tower");
    let face = definition.primary_face();
    assert_eq!((face.power, face.toughness), (Some(3), Some(3)));
    let abilities = &face.static_abilities;
    assert_eq!(abilities.len(), 3, "unexpected payload: {abilities:?}");
    assert_eq!(
        abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        abilities[1].presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        abilities[2].presentation,
        AbilityPresentation::OracleLines(vec![2])
    );

    let (condition, power, toughness, keywords) = self_modifier(&abilities[0].definition);
    assert_eq!(
        condition,
        &graveyard_condition(
            GraveyardAggregate::CardCount,
            Some(permanent_card_filter()),
            Some(4),
            None,
        )
    );
    assert_eq!((power, toughness), (2, 2));
    assert_eq!(keywords, [Keyword::Trample]);

    let (condition, power, toughness, keywords) = self_modifier(&abilities[1].definition);
    assert_eq!(
        condition,
        &graveyard_condition(
            GraveyardAggregate::CardCount,
            Some(permanent_card_filter()),
            Some(8),
            None,
        )
    );
    assert_eq!((power, toughness), (2, 2));
    assert!(keywords.is_empty());

    let StaticAbilityDef::SelfCombatRestriction {
        restriction,
        condition,
    } = &abilities[2].definition
    else {
        panic!("Akawalli's Descend 8 line must close with the blocker limit");
    };
    assert_eq!(
        restriction,
        &CombatRestriction {
            maximum_blockers: Some(1),
            ..CombatRestriction::default()
        }
    );
    assert_eq!(
        condition,
        &Some(graveyard_condition(
            GraveyardAggregate::CardCount,
            Some(permanent_card_filter()),
            Some(8),
            None,
        ))
    );
}

#[test]
fn issue_377_descend_pumps_use_the_six_type_permanent_predicate() {
    for (id, power, toughness, keywords) in [
        ("basking_capybara", 3, 0, vec![]),
        ("echo_of_dusk", 1, 1, vec![Keyword::Lifelink]),
        ("frilled_cave-wurm", 2, 0, vec![]),
    ] {
        let definition = CardRegistry::global().get(id).expect(id);
        let [ability] = definition.primary_face().static_abilities.as_slice() else {
            panic!("{id} must have exactly one static ability");
        };
        let (condition, delta_power, delta_toughness, ability_keywords) =
            self_modifier(&ability.definition);
        assert_eq!((delta_power, delta_toughness), (power, toughness), "{id}");
        assert_eq!(ability_keywords, keywords, "{id}");
        assert_eq!(
            condition,
            &graveyard_condition(
                GraveyardAggregate::CardCount,
                Some(permanent_card_filter()),
                Some(4),
                None,
            ),
            "{id}"
        );
    }

    let didact_echo = CardRegistry::global()
        .get("didact_echo")
        .expect("Didact Echo");
    let face = didact_echo.primary_face();
    assert_eq!(
        face.triggered_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    let (condition, power, toughness, keywords) =
        self_modifier(&face.static_abilities[0].definition);
    assert_eq!((power, toughness), (0, 0));
    assert_eq!(keywords, [Keyword::Flying]);
    assert_eq!(
        condition,
        &graveyard_condition(
            GraveyardAggregate::CardCount,
            Some(permanent_card_filter()),
            Some(4),
            None,
        )
    );
}

#[test]
fn issue_377_nonpermanent_graveyard_predicates_keep_printed_filters() {
    let first_time_flyer = CardRegistry::global()
        .get("first-time_flyer")
        .expect("First-Time Flyer");
    let (condition, power, toughness, _) =
        self_modifier(&first_time_flyer.primary_face().static_abilities[0].definition);
    assert_eq!(
        condition,
        &graveyard_condition(
            GraveyardAggregate::CardCount,
            Some(ZoneCardFilter {
                required_subtypes: vec!["Lesson".into()],
                ..ZoneCardFilter::default()
            }),
            Some(1),
            None,
        )
    );
    assert_eq!((power, toughness), (1, 1));

    let ghitu_lavarunner = CardRegistry::global()
        .get("ghitu_lavarunner")
        .expect("Ghitu Lavarunner");
    let (condition, power, toughness, keywords) =
        self_modifier(&ghitu_lavarunner.primary_face().static_abilities[0].definition);
    assert_eq!((power, toughness), (1, 0));
    assert_eq!(keywords, [Keyword::Haste]);
    let GameCondition::GraveyardAggregate {
        filter: Some(filter),
        aggregate: GraveyardAggregate::CardCount,
        min: Some(2),
        max: None,
        ..
    } = condition
    else {
        panic!("Ghitu Lavarunner must count graveyard cards: {condition:?}");
    };
    let branches = filter.any_of.as_ref().expect("instant/sorcery union");
    assert_eq!(branches.len(), 2);
    assert_eq!(branches[0].card_type, Some(CardTypeFilter::Instant));
    assert_eq!(branches[1].card_type, Some(CardTypeFilter::Sorcery));

    let wildfire = CardRegistry::global()
        .get("wildfire_wickerfolk")
        .expect("Wildfire Wickerfolk");
    let (condition, power, toughness, keywords) =
        self_modifier(&wildfire.primary_face().static_abilities[0].definition);
    assert_eq!((power, toughness), (1, 1));
    assert_eq!(keywords, [Keyword::Trample]);
    assert_eq!(
        condition,
        &graveyard_condition(GraveyardAggregate::DistinctCardTypes, None, Some(4), None,)
    );
}
