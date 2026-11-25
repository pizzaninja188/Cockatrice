//! Registry conformance for the queue-v44 reuse-only cohorts.
#![allow(clippy::type_complexity)]
use tricerules_cards::primitives::{
    AbilityCost, CastTriggerPlayer, Color, CreatureTypeChange, EffectSubject, Keyword,
    RelativePlayerSet, SearchDestination, SpellEffectKind, StaticAbilityDef, TargetKind,
    TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, CardDefinition, Layout};

fn definition(id: &str) -> &'static CardDefinition {
    tricerules_cards::registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
}

fn mana_cost(cost: &AbilityCost) -> String {
    match cost {
        AbilityCost::Mana(cost) => cost.to_string(),
        other => panic!("expected a mana cost, got {other:?}"),
    }
}

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    body: Option<(u32, u32)>,
}

fn identity(card: &Card) -> &'static tricerules_cards::CardFace {
    let definition = definition(card.id);
    assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
    assert_eq!(
        tricerules_cards::registry::global().id_for_name(card.name),
        Some(card.id),
        "{}",
        card.id
    );
    let f = definition.primary_face();
    assert_eq!(f.face_id.as_str(), card.id, "{} face id", card.id);
    assert_eq!(f.name, card.name, "{} name", card.id);
    assert_eq!(f.mana_cost.to_string(), card.mana, "{} mana", card.id);
    assert_eq!(
        f.types.iter().map(String::as_str).collect::<Vec<_>>(),
        card.types,
        "{} types",
        card.id
    );
    assert_eq!(
        (f.power, f.toughness),
        card.body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
        "{} body",
        card.id
    );
    f
}

const VERGES: [(&str, &str); 6] = [
    ("bleachbone_verge", "Bleachbone Verge"),
    ("blazemire_verge", "Blazemire Verge"),
    ("hushwood_verge", "Hushwood Verge"),
    ("sunbillow_verge", "Sunbillow Verge"),
    ("thornspire_verge", "Thornspire Verge"),
    ("wastewood_verge", "Wastewood Verge"),
];

#[test]
fn q44_verge_lands_maps_definitions() {
    for (id, name) in VERGES {
        let f = identity(&Card {
            id,
            name,
            mana: "",
            types: &["Land"],
            body: None,
        });
        let [first, second] = f.activated_abilities.as_slice() else {
            panic!("{id}: two mana abilities");
        };
        assert_eq!(first.costs, vec![AbilityCost::Tap], "{id}");
        assert_eq!(second.costs, vec![AbilityCost::Tap], "{id}");
        assert_eq!(
            first.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(
            second.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert!(matches!(
            first.effect.as_slice(),
            [SpellEffectKind::ProduceMana { options, .. }] if options.len() == 1
        ));
        assert!(matches!(
            second.effect.as_slice(),
            [SpellEffectKind::ProduceMana { options, .. }] if options.len() == 1
        ));
        assert_eq!(second.conditions.len(), 1, "{id}: printed condition");
        assert!(
            matches!(
                &second.conditions[0],
                tricerules_cards::primitives::GameCondition::BattlefieldAggregate { filter, .. }
                    if filter.controllers == RelativePlayerSet::Controller
                        && filter.any_of.as_ref().is_some_and(|branches| branches.len() == 2)
            ),
            "{id}: two basic subtypes"
        );
    }
}

#[test]
fn q44_tainted_lands_maps_definitions() {
    for (id, name) in [
        ("tainted_field", "Tainted Field"),
        ("tainted_isle", "Tainted Isle"),
        ("tainted_peak", "Tainted Peak"),
        ("tainted_wood", "Tainted Wood"),
    ] {
        let f = identity(&Card {
            id,
            name,
            mana: "",
            types: &["Land"],
            body: None,
        });
        let [colorless, colored] = f.activated_abilities.as_slice() else {
            panic!("{id}: two mana abilities");
        };
        assert_eq!(colorless.costs, vec![AbilityCost::Tap], "{id}");
        assert_eq!(colored.costs, vec![AbilityCost::Tap], "{id}");
        assert!(matches!(
            colorless.effect.as_slice(),
            [SpellEffectKind::ProduceMana { options, .. }] if options[0].c == 1
        ));
        assert!(matches!(
            colored.effect.as_slice(),
            [SpellEffectKind::ProduceMana { options, .. }] if options.len() == 2
        ));
        assert_eq!(colored.conditions.len(), 1, "{id}");
        assert!(
            matches!(
                &colored.conditions[0],
                tricerules_cards::primitives::GameCondition::BattlefieldAggregate { filter, .. }
                    if filter.controllers == RelativePlayerSet::Controller
                        && filter.matching.required_subtypes == ["Swamp"]
            ),
            "{id}: requires a Swamp"
        );
    }
}

#[test]
fn q44_monuments_maps_definitions() {
    let cases = [
        (
            "atarka_monument",
            "Atarka Monument",
            "{4}{R}{G}",
            [Color::Red, Color::Green],
        ),
        (
            "dromoka_monument",
            "Dromoka Monument",
            "{4}{G}{W}",
            [Color::Green, Color::White],
        ),
        (
            "kolaghan_monument",
            "Kolaghan Monument",
            "{4}{B}{R}",
            [Color::Black, Color::Red],
        ),
        (
            "ojutai_monument",
            "Ojutai Monument",
            "{4}{W}{U}",
            [Color::White, Color::Blue],
        ),
        (
            "silumgar_monument",
            "Silumgar Monument",
            "{4}{U}{B}",
            [Color::Blue, Color::Black],
        ),
    ];
    for (id, name, animation_cost, colors) in cases {
        let f = identity(&Card {
            id,
            name,
            mana: "{3}",
            types: &["Artifact"],
            body: None,
        });
        let [mana, animation] = f.activated_abilities.as_slice() else {
            panic!("{id}: mana and animation abilities");
        };
        assert_eq!(mana.costs, vec![AbilityCost::Tap], "{id}");
        assert!(matches!(
            mana.effect.as_slice(),
            [SpellEffectKind::ProduceMana { options, .. }] if options.len() == 2
        ));
        assert_eq!(animation.costs.len(), 1, "{id}: one mana cost");
        assert_eq!(mana_cost(&animation.costs[0]), animation_cost, "{id}");
        assert!(
            matches!(
                animation.effect.as_slice(),
                [SpellEffectKind::AnimateSelf {
                    base_power: 4,
                    base_toughness: 4,
                    colors: Some(actual),
                    creature_types: CreatureTypeChange::Replace(types),
                    keywords,
                    duration: tricerules_cards::primitives::ResolvingEffectDuration::UntilEndOfTurn,
                    ..
                }] if actual.as_slice() == colors.as_slice()
                    && types.as_slice() == ["Dragon"]
                    && keywords.as_slice() == [Keyword::Flying]
            ),
            "{id}: printed animation"
        );
    }
}

#[test]
fn q44_hybrid_evasion_maps_definitions() {
    let cases = [
        (
            "sootwalkers",
            "Sootwalkers",
            "{2}{B/R}{B/R}",
            &["Creature", "Elemental", "Rogue"][..],
            Color::White,
        ),
        (
            "wanderbrine_rootcutters",
            "Wanderbrine Rootcutters",
            "{2}{U/B}{U/B}",
            &["Creature", "Merfolk", "Rogue"][..],
            Color::Green,
        ),
        (
            "barrenton_cragtreads",
            "Barrenton Cragtreads",
            "{2}{W/U}{W/U}",
            &["Creature", "Kithkin", "Scout"][..],
            Color::Red,
        ),
        (
            "mudbrawler_raiders",
            "Mudbrawler Raiders",
            "{2}{R/G}{R/G}",
            &["Creature", "Goblin", "Warrior"][..],
            Color::Blue,
        ),
        (
            "ravens_run_dragoon",
            "Raven's Run Dragoon",
            "{2}{G/W}{G/W}",
            &["Creature", "Elf", "Knight"][..],
            Color::Black,
        ),
    ];
    for (id, name, mana, types, color) in cases {
        let f = identity(&Card {
            id,
            name,
            mana,
            types,
            body: Some((3, 3)),
        });
        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{id}: one static ability");
        };
        assert_eq!(static_ability.ability_id.as_str(), "static_01");
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert!(
            matches!(
                &static_ability.definition,
                StaticAbilityDef::SelfCombatRestriction { restriction, condition: None }
                    if restriction.cant_be_blocked_by.len() == 1
                        && restriction.cant_be_blocked_by[0].kind == TargetKind::Creature
                        && restriction.cant_be_blocked_by[0].is_color == Some(color)
            ),
            "{id}: printed blocker color"
        );
    }
}

#[test]
fn q44_keyword_auras_maps_definitions() {
    let cases = [
        (
            "alexis_cloak",
            "Alexi's Cloak",
            "{1}{U}",
            &[Keyword::Shroud][..],
            true,
            3,
        ),
        (
            "buoyancy",
            "Buoyancy",
            "{1}{U}",
            &[Keyword::Flying][..],
            true,
            3,
        ),
        (
            "eternal_warrior",
            "Eternal Warrior",
            "{R}",
            &[Keyword::Vigilance][..],
            false,
            2,
        ),
        (
            "reflexes",
            "Reflexes",
            "{R}",
            &[Keyword::FirstStrike][..],
            false,
            2,
        ),
        (
            "zephyr_net",
            "Zephyr Net",
            "{1}{U}",
            &[Keyword::Defender, Keyword::Flying][..],
            false,
            2,
        ),
    ];
    for (id, name, mana, keywords, flash, line) in cases {
        let f = identity(&Card {
            id,
            name,
            mana,
            types: &["Enchantment", "Aura"],
            body: None,
        });
        assert_eq!(f.keywords.contains(&Keyword::Flash), flash, "{id}");
        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{id}: one static ability");
        };
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![line])
        );
        assert!(
            matches!(
                &static_ability.definition,
                StaticAbilityDef::AttachedModifier { keywords: actual, .. } if actual == keywords
            ),
            "{id}: attached keywords"
        );
        assert!(matches!(
            f.spell_effect.as_slice(),
            [SpellEffectKind::AuraAttach { .. }]
        ));
    }
}

#[test]
fn q44_color_hate_counters_maps_definitions() {
    let cases = [
        ("deathgrip", "Deathgrip", "{B}{B}", "{B}{B}", Color::Green),
        ("douse", "Douse", "{2}{U}", "{1}{U}", Color::Red),
        ("lifeforce", "Lifeforce", "{G}{G}", "{G}{G}", Color::Black),
    ];
    for (id, name, mana, cost, color) in cases {
        let f = identity(&Card {
            id,
            name,
            mana,
            types: &["Enchantment"],
            body: None,
        });
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: one counter ability");
        };
        assert_eq!(ability.costs.len(), 1, "{id}: mana-only activation");
        assert_eq!(mana_cost(&ability.costs[0]), cost, "{id}");
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::CounterTargetSpell { spell_filter, .. }]
                    if spell_filter.is_color == Some(color)
            ),
            "{id}: printed color filter"
        );
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
    }
}

#[test]
fn q44_basic_land_pump_maps_definitions() {
    let cases = [
        (
            "kird_ape",
            "Kird Ape",
            "{R}",
            &["Creature", "Ape"][..],
            (1, 1),
            "Forest",
            (1, 2),
        ),
        (
            "loam_lion",
            "Loam Lion",
            "{W}",
            &["Creature", "Cat"][..],
            (1, 1),
            "Forest",
            (1, 2),
        ),
        (
            "mire_kavu",
            "Mire Kavu",
            "{3}{R}",
            &["Creature", "Kavu"][..],
            (3, 2),
            "Swamp",
            (1, 1),
        ),
    ];
    for (id, name, mana, types, body, subtype, (dp, dt)) in cases {
        let f = identity(&Card {
            id,
            name,
            mana,
            types,
            body: Some(body),
        });
        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{id}: one static ability");
        };
        assert!(
            matches!(
                &static_ability.definition,
                StaticAbilityDef::ConditionalSelfModifier {
                    condition: tricerules_cards::primitives::GameCondition::BattlefieldAggregate {
                        filter,
                        ..
                    },
                    delta_power,
                    delta_toughness,
                    ..
                } if filter.controllers == RelativePlayerSet::Controller
                    && filter.matching.required_subtypes == [subtype]
                    && (*delta_power, *delta_toughness) == (dp, dt)
            ),
            "{id}: printed subtype pump"
        );
    }
}

#[test]
fn q44_haste_return_maps_definitions() {
    let cases = [
        (
            "glitterfang",
            "Glitterfang",
            "{R}",
            &["Creature", "Spirit"][..],
            (1, 1),
        ),
        (
            "viashino_cutthroat",
            "Viashino Cutthroat",
            "{2}{R}{R}",
            &["Creature", "Lizard"][..],
            (5, 3),
        ),
        (
            "viashino_sandstalker",
            "Viashino Sandstalker",
            "{1}{R}{R}",
            &["Creature", "Lizard", "Warrior"][..],
            (4, 2),
        ),
        (
            "viashino_sandscout",
            "Viashino Sandscout",
            "{1}{R}",
            &["Creature", "Lizard", "Scout"][..],
            (2, 1),
        ),
    ];
    for (id, name, mana, types, body) in cases {
        let f = identity(&Card {
            id,
            name,
            mana,
            types,
            body: Some(body),
        });
        assert_eq!(f.keywords, vec![Keyword::Haste], "{id}: haste");
        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{id}: one end-step trigger");
        };
        assert_eq!(
            trigger.trigger,
            TriggerCondition::AtBeginningOfEndStep {
                player: CastTriggerPlayer::AnyPlayer
            },
            "{id}: any player's end step"
        );
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert!(
            matches!(
                trigger.effect.as_slice(),
                [SpellEffectKind::ReturnToOwnersHand {
                    subject: EffectSubject::Source
                }]
            ),
            "{id}: returns itself"
        );
    }
}

#[test]
fn q44_mirage_fetch_lands_maps_definitions() {
    for (id, name) in [
        ("flood_plain", "Flood Plain"),
        ("grasslands", "Grasslands"),
        ("mountain_valley", "Mountain Valley"),
        ("rocky_tar_pit", "Rocky Tar Pit"),
    ] {
        let f = identity(&Card {
            id,
            name,
            mana: "",
            types: &["Land"],
            body: None,
        });
        assert!(matches!(
            f.static_abilities.as_slice(),
            [ability]
                if matches!(
                    &ability.definition,
                    StaticAbilityDef::EntersTapped { .. }
                )
        ));
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: one search ability");
        };
        assert_eq!(
            ability.costs,
            vec![AbilityCost::Tap, AbilityCost::SacrificeSelf],
            "{id}"
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::SearchLibrary {
                    filter: Some(filter),
                    destination: SearchDestination::Battlefield { tapped: false },
                    ..
                }] if filter.any_of.as_ref().is_some_and(|branches| branches.len() == 2)
            ),
            "{id}: printed subtype pair enters untapped"
        );
    }
}

#[test]
fn q44_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    let cards: [(&str, &str); 39] = [
        ("bleachbone_verge", "Bleachbone Verge"),
        ("blazemire_verge", "Blazemire Verge"),
        ("hushwood_verge", "Hushwood Verge"),
        ("sunbillow_verge", "Sunbillow Verge"),
        ("thornspire_verge", "Thornspire Verge"),
        ("wastewood_verge", "Wastewood Verge"),
        ("tainted_field", "Tainted Field"),
        ("tainted_isle", "Tainted Isle"),
        ("tainted_peak", "Tainted Peak"),
        ("tainted_wood", "Tainted Wood"),
        ("atarka_monument", "Atarka Monument"),
        ("dromoka_monument", "Dromoka Monument"),
        ("kolaghan_monument", "Kolaghan Monument"),
        ("ojutai_monument", "Ojutai Monument"),
        ("silumgar_monument", "Silumgar Monument"),
        ("sootwalkers", "Sootwalkers"),
        ("wanderbrine_rootcutters", "Wanderbrine Rootcutters"),
        ("barrenton_cragtreads", "Barrenton Cragtreads"),
        ("mudbrawler_raiders", "Mudbrawler Raiders"),
        ("ravens_run_dragoon", "Raven's Run Dragoon"),
        ("alexis_cloak", "Alexi's Cloak"),
        ("buoyancy", "Buoyancy"),
        ("eternal_warrior", "Eternal Warrior"),
        ("reflexes", "Reflexes"),
        ("zephyr_net", "Zephyr Net"),
        ("deathgrip", "Deathgrip"),
        ("douse", "Douse"),
        ("lifeforce", "Lifeforce"),
        ("kird_ape", "Kird Ape"),
        ("loam_lion", "Loam Lion"),
        ("mire_kavu", "Mire Kavu"),
        ("glitterfang", "Glitterfang"),
        ("viashino_cutthroat", "Viashino Cutthroat"),
        ("viashino_sandstalker", "Viashino Sandstalker"),
        ("viashino_sandscout", "Viashino Sandscout"),
        ("flood_plain", "Flood Plain"),
        ("grasslands", "Grasslands"),
        ("mountain_valley", "Mountain Valley"),
        ("rocky_tar_pit", "Rocky Tar Pit"),
    ];
    for (id, name) in cards {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "{id}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id, "face id drift for {id}");
        assert_eq!(fields[3], name);
        assert_eq!(fields[4].len(), 64, "fingerprint length for {id}");
        assert!(
            fields[4].chars().all(|ch| ch.is_ascii_hexdigit()),
            "fingerprint digest for {id}"
        );
        assert_eq!(definition(id).primary_face().name, name, "{id}");
    }
}
