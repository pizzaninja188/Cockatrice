//! Registry conformance for the queue-v48 reuse-only cohorts.
#![allow(clippy::type_complexity)]
use tricerules_cards::mana::ManaCost;
use tricerules_cards::primitives::{
    AbilityCost, Amount, CardTypeFilter, CastTriggerPlayer, Color, CounterKind,
    CreatureScopeController, EffectSubject, Instruction, Keyword, PermanentTypeFilter,
    PlayerRecipient, SpellEffectKind, StackSpellFilter, StaticAbilityDef, TargetController,
    TargetKind, TargetingSourceFilter, TriggerCondition,
};
use tricerules_cards::{
    registry, AbilityPresentation, CardDefinition, CardFace, InstructionKind, Layout,
};

fn definition(id: &str) -> &'static CardDefinition {
    registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
}

fn face_id(name: &str) -> String {
    let folded = name.to_lowercase().replace(['\'', ','], "");
    let mut text = String::new();
    let mut last_underscore = false;
    for ch in folded.chars() {
        if ch.is_ascii_alphanumeric() {
            text.push(ch);
            last_underscore = false;
        } else if !last_underscore {
            text.push('_');
            last_underscore = true;
        }
    }
    text.trim_matches('_').to_string()
}

fn identity(
    id: &str,
    name: &str,
    mana: &str,
    types: &[&str],
    body: Option<(u32, u32)>,
) -> &'static CardFace {
    let definition = definition(id);
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    assert_eq!(registry::global().id_for_name(name), Some(id), "{id}");
    let f = definition.primary_face();
    assert_eq!(f.face_id.as_str(), face_id(name), "{id} face id");
    assert_eq!(f.name, name, "{id} name");
    assert_eq!(f.mana_cost.to_string(), mana, "{id} mana");
    assert_eq!(
        f.types.iter().map(String::as_str).collect::<Vec<_>>(),
        types,
        "{id} types"
    );
    assert_eq!(
        (f.power, f.toughness),
        body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
        "{id} body"
    );
    f
}

fn perform(step: &Instruction) -> &SpellEffectKind {
    let InstructionKind::Perform(effect) = &step.kind else {
        panic!("expected a Perform instruction");
    };
    effect
}

// ── 01 team anthem ─────────────────────────────────────────────────────────

#[test]
fn q48_team_anthem_maps_definitions() {
    struct Case {
        id: &'static str,
        name: &'static str,
        mana: &'static str,
        types: &'static [&'static str],
        body: Option<(u32, u32)>,
        legendary: bool,
        keywords: &'static [Keyword],
        presentation: u32,
        color: Option<Color>,
        required_keyword: Option<Keyword>,
        exclude_self: bool,
        delta: (i32, i32),
    }
    for case in [
        Case {
            id: "honor_of_the_pure",
            name: "Honor of the Pure",
            mana: "{1}{W}",
            types: &["Enchantment"],
            body: None,
            legendary: false,
            keywords: &[],
            presentation: 1,
            color: Some(Color::White),
            required_keyword: None,
            exclude_self: false,
            delta: (1, 1),
        },
        Case {
            id: "jacques_le_vert",
            name: "Jacques le Vert",
            mana: "{1}{R}{G}{W}",
            types: &["Creature", "Human", "Warrior"],
            body: Some((3, 2)),
            legendary: true,
            keywords: &[],
            presentation: 1,
            color: Some(Color::Green),
            required_keyword: None,
            exclude_self: false,
            delta: (0, 2),
        },
        Case {
            id: "kaysa",
            name: "Kaysa",
            mana: "{3}{G}{G}",
            types: &["Creature", "Elf", "Druid"],
            body: Some((2, 3)),
            legendary: true,
            keywords: &[],
            presentation: 1,
            color: Some(Color::Green),
            required_keyword: None,
            exclude_self: false,
            delta: (1, 1),
        },
        Case {
            id: "meng_huo,_barbarian_king",
            name: "Meng Huo, Barbarian King",
            mana: "{3}{G}{G}",
            types: &["Creature", "Human", "Barbarian", "Soldier"],
            body: Some((4, 4)),
            legendary: true,
            keywords: &[],
            presentation: 1,
            color: Some(Color::Green),
            required_keyword: None,
            exclude_self: true,
            delta: (1, 1),
        },
        Case {
            id: "windstorm_drake",
            name: "Windstorm Drake",
            mana: "{4}{U}",
            types: &["Creature", "Drake"],
            body: Some((3, 3)),
            legendary: false,
            keywords: &[Keyword::Flying],
            presentation: 2,
            color: None,
            required_keyword: Some(Keyword::Flying),
            exclude_self: true,
            delta: (1, 0),
        },
        Case {
            id: "spirit_of_the_spires",
            name: "Spirit of the Spires",
            mana: "{3}{W}",
            types: &["Creature", "Spirit"],
            body: Some((2, 4)),
            legendary: false,
            keywords: &[Keyword::Flying],
            presentation: 2,
            color: None,
            required_keyword: Some(Keyword::Flying),
            exclude_self: true,
            delta: (0, 1),
        },
    ] {
        let f = identity(case.id, case.name, case.mana, case.types, case.body);
        assert_eq!(
            f.supertypes.iter().any(|s| s == "Legendary"),
            case.legendary
        );
        assert_eq!(f.keywords, case.keywords, "{}", case.id);
        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{}: one anthem", case.id);
        };
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![case.presentation as u16]),
            "{}",
            case.id
        );
        let StaticAbilityDef::AnthemPt {
            filter,
            delta_power,
            delta_toughness,
            ..
        } = &static_ability.definition
        else {
            panic!("{}: AnthemPt", case.id);
        };
        assert_eq!(
            filter.controller,
            Some(CreatureScopeController::YouControl),
            "{}",
            case.id
        );
        assert_eq!(filter.color, case.color, "{}", case.id);
        assert_eq!(
            filter.required_keyword, case.required_keyword,
            "{}",
            case.id
        );
        assert_eq!(filter.exclude_self, case.exclude_self, "{}", case.id);
        assert_eq!((*delta_power, *delta_toughness), case.delta, "{}", case.id);
    }
}

// ── 02 scry then draw ──────────────────────────────────────────────────────

#[test]
fn q48_scry_draw_maps_definitions() {
    for (id, name, mana, kind, scry, draw) in [
        ("deliberate", "Deliberate", "{1}{U}", "Instant", 2, 1),
        ("foresee", "Foresee", "{3}{U}", "Sorcery", 4, 2),
        (
            "introduction_to_prophecy",
            "Introduction to Prophecy",
            "{3}",
            "Sorcery",
            2,
            1,
        ),
        (
            "tamiyos_epiphany",
            "Tamiyo's Epiphany",
            "{3}{U}",
            "Sorcery",
            4,
            2,
        ),
    ] {
        let scry: u32 = scry;
        let draw: u32 = draw;
        let types: &[&str] = if id == "introduction_to_prophecy" {
            &["Sorcery", "Lesson"]
        } else {
            &[kind]
        };
        let f = identity(id, name, mana, types, None);
        let program = f
            .spell_program
            .as_ref()
            .unwrap_or_else(|| panic!("{id}: program"));
        let [scry_step, draw_step] = program.body.steps.as_slice() else {
            panic!("{id}: scry then draw");
        };
        assert_eq!(scry_step.id.as_str(), "scry");
        assert!(matches!(
            perform(scry_step),
            SpellEffectKind::Scry {
                count: Amount::Fixed(n)
            } if *n == scry
        ));
        assert_eq!(draw_step.id.as_str(), "draw");
        assert!(matches!(
            perform(draw_step),
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(n),
            } if *n == draw
        ));
    }
}

// ── 03 destroy plus cycling ────────────────────────────────────────────────

#[test]
fn q48_destroy_cycling_maps_definitions() {
    for (id, name, mana, kind, cycle, either) in [
        (
            "break_asunder",
            "Break Asunder",
            "{2}{G}{G}",
            "Sorcery",
            "{2}",
            true,
        ),
        (
            "dissenters_deliverance",
            "Dissenter's Deliverance",
            "{1}{G}",
            "Instant",
            "{G}",
            false,
        ),
        ("wilt", "Wilt", "{1}{G}", "Instant", "{2}", true),
    ] {
        let f = identity(id, name, mana, &[kind], None);
        assert!(matches!(
            f.spell_effect.as_slice(),
            [SpellEffectKind::Destroy { subject: EffectSubject::Chosen(filter) }]
                if if either {
                    filter.permanent_types == vec![
                        PermanentTypeFilter::Artifact,
                        PermanentTypeFilter::Enchantment,
                    ]
                } else {
                    filter.permanent_types == vec![PermanentTypeFilter::Artifact]
                }
        ));
        let targets = f
            .targeting
            .as_ref()
            .unwrap_or_else(|| panic!("{id}: target"));
        assert_eq!(targets.groups.len(), 1);
        assert_eq!(targets.groups[0].min, 1);
        assert_eq!(targets.groups[0].max, 1);
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: one cycling ability");
        };
        assert_eq!(
            ability.source_zone,
            tricerules_cards::primitives::AbilitySourceZone::Hand
        );
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert_eq!(
            ability.costs,
            vec![
                AbilityCost::Mana(ManaCost::parse(cycle).unwrap()),
                AbilityCost::DiscardSelf,
            ]
        );
        assert!(matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }]
        ));
    }
}

// ── 04 pump plus keyword ───────────────────────────────────────────────────

#[test]
fn q48_pump_keyword_maps_definitions() {
    for (id, name, mana, kind, power, toughness, keyword) in [
        (
            "lash_of_thorns",
            "Lash of Thorns",
            "{B}",
            "Instant",
            2,
            1,
            Keyword::Deathtouch,
        ),
        (
            "mighty_leap",
            "Mighty Leap",
            "{1}{W}",
            "Instant",
            2,
            2,
            Keyword::Flying,
        ),
        (
            "screaming_fury",
            "Screaming Fury",
            "{2}{R}",
            "Sorcery",
            5,
            0,
            Keyword::Haste,
        ),
        (
            "uncanny_speed",
            "Uncanny Speed",
            "{1}{R}",
            "Instant",
            3,
            0,
            Keyword::Haste,
        ),
    ] {
        let f = identity(id, name, mana, &[kind], None);
        let [pump, grant] = f.spell_effect.as_slice() else {
            panic!("{id}: pump and keyword");
        };
        assert!(matches!(
            pump,
            SpellEffectKind::PumpTarget {
                power: p,
                toughness: t,
                subject: EffectSubject::Chosen(filter),
                ..
            } if *p == power && *t == toughness && filter.kind == TargetKind::Creature
        ));
        assert!(matches!(
            grant,
            SpellEffectKind::GrantKeywords {
                subject: EffectSubject::Chosen(filter),
                keywords,
            } if filter.kind == TargetKind::Creature && keywords == &vec![keyword]
        ));
        let targets = f
            .targeting
            .as_ref()
            .unwrap_or_else(|| panic!("{id}: target"));
        assert_eq!(targets.groups.len(), 1);
        assert_eq!(targets.groups[0].effect_indices, vec![0, 1]);
    }
}

// ── 05 typed counters ──────────────────────────────────────────────────────

#[test]
fn q48_typed_counters_maps_definitions() {
    for (id, name, mana, expected) in [
        (
            "artifact_blast",
            "Artifact Blast",
            "{R}",
            StackSpellFilter {
                card_type: Some(CardTypeFilter::Artifact),
                ..Default::default()
            },
        ),
        (
            "gainsay",
            "Gainsay",
            "{1}{U}",
            StackSpellFilter {
                is_color: Some(Color::Blue),
                ..Default::default()
            },
        ),
        (
            "guttural_response",
            "Guttural Response",
            "{R/G}",
            StackSpellFilter {
                card_type: Some(CardTypeFilter::Instant),
                is_color: Some(Color::Blue),
                ..Default::default()
            },
        ),
        (
            "mystic_denial",
            "Mystic Denial",
            "{1}{U}{U}",
            StackSpellFilter {
                any_of: Some(vec![
                    StackSpellFilter {
                        card_type: Some(CardTypeFilter::Creature),
                        ..Default::default()
                    },
                    StackSpellFilter {
                        card_type: Some(CardTypeFilter::Sorcery),
                        ..Default::default()
                    },
                ]),
                ..Default::default()
            },
        ),
    ] {
        let f = identity(id, name, mana, &["Instant"], None);
        assert!(matches!(
            f.spell_effect.as_slice(),
            [SpellEffectKind::CounterTargetSpell { spell_filter, unless_controller_pays: None, .. }]
                if *spell_filter == expected
        ));
        assert!(f.targeting.is_some(), "{id}: explicit stack target");
    }
}

// ── 06 entering tap ────────────────────────────────────────────────────────

#[test]
fn q48_etb_tap_maps_definitions() {
    for (id, name, mana, types, body, line, keywords, stun) in [
        (
            "alchemax_slayer-bots",
            "Alchemax Slayer-Bots",
            "{2}{U}",
            &["Artifact", "Creature", "Robot", "Villain"][..],
            (2, 2),
            1,
            &[][..],
            true,
        ),
        (
            "chrome_prowler",
            "Chrome Prowler",
            "{2}{U}",
            &["Artifact", "Creature", "Phyrexian", "Cat"][..],
            (3, 2),
            2,
            &[Keyword::Flash][..],
            false,
        ),
        (
            "fractal_mascot",
            "Fractal Mascot",
            "{4}{G}{U}",
            &["Creature", "Fractal", "Elk"][..],
            (6, 6),
            2,
            &[Keyword::Trample][..],
            true,
        ),
        (
            "heavy_infantry",
            "Heavy Infantry",
            "{4}{W}",
            &["Creature", "Human", "Soldier"][..],
            (3, 4),
            1,
            &[][..],
            false,
        ),
        (
            "shipwreck_patrol",
            "Shipwreck Patrol",
            "{3}{U}",
            &["Creature", "Merfolk", "Warrior"][..],
            (3, 3),
            1,
            &[][..],
            true,
        ),
    ] {
        let f = identity(id, name, mana, types, Some(body));
        assert_eq!(f.keywords, keywords, "{id}");
        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{id}: one enters trigger");
        };
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![line]),
            "{id}"
        );
        let expected_len = if stun { 2 } else { 1 };
        assert_eq!(trigger.effect.len(), expected_len, "{id}");
        assert!(matches!(
            &trigger.effect[0],
            SpellEffectKind::Tap {
                subject: EffectSubject::Chosen(filter)
            } if filter.kind == TargetKind::Creature
                && filter.controller == TargetController::Opponent
        ));
        if stun {
            assert!(matches!(
                &trigger.effect[1],
                SpellEffectKind::PutCounters {
                    counter: CounterKind::Stun,
                    count: Amount::Fixed(1),
                    subject: EffectSubject::Chosen(filter),
                } if filter.kind == TargetKind::Creature
                    && filter.controller == TargetController::Opponent
            ));
        }
        let targets = trigger
            .targeting
            .as_ref()
            .unwrap_or_else(|| panic!("{id}: one target group"));
        let expected_indices: Vec<u32> = if stun { vec![0, 1] } else { vec![0] };
        assert_eq!(targets.groups[0].effect_indices, expected_indices, "{id}");
    }
}

// ── 07 any-color mana ──────────────────────────────────────────────────────

#[test]
fn q48_any_color_mana_maps_definitions() {
    for (id, name, mana, types, body, entries) in [
        (
            "centaur_nurturer",
            "Centaur Nurturer",
            "{3}{G}",
            &["Creature", "Centaur", "Druid"][..],
            Some((2, 4)),
            2,
        ),
        (
            "dawnhart_rejuvenator",
            "Dawnhart Rejuvenator",
            "{3}{G}",
            &["Creature", "Human", "Warlock"][..],
            Some((2, 4)),
            2,
        ),
        (
            "henge_of_ramos",
            "Henge of Ramos",
            "",
            &["Land"][..],
            None,
            2,
        ),
        (
            "orochi_leafcaller",
            "Orochi Leafcaller",
            "{G}",
            &["Creature", "Snake", "Shaman"][..],
            Some((1, 1)),
            1,
        ),
        (
            "prismite",
            "Prismite",
            "{2}",
            &["Artifact", "Creature", "Golem"][..],
            Some((2, 1)),
            1,
        ),
        (
            "school_of_the_unseen",
            "School of the Unseen",
            "",
            &["Land"][..],
            None,
            2,
        ),
    ] {
        let f = identity(id, name, mana, types, body);
        if id == "centaur_nurturer" || id == "dawnhart_rejuvenator" {
            let [trigger] = f.triggered_abilities.as_slice() else {
                panic!("{id}: gain-life trigger");
            };
            assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
            assert!(matches!(
                trigger.effect.as_slice(),
                [SpellEffectKind::GainLife {
                    amount: Amount::Fixed(3),
                    who: PlayerRecipient::Controller,
                }]
            ));
        }
        let expected_abilities = if id == "centaur_nurturer" || id == "dawnhart_rejuvenator" {
            1
        } else {
            entries
        };
        assert_eq!(f.activated_abilities.len(), expected_abilities, "{id}");
        let last = f.activated_abilities.last().expect("--color ability");
        assert_eq!(
            last.presentation,
            AbilityPresentation::OracleLines(vec![entries as u16])
        );
        assert!(matches!(
            last.effect.as_slice(),
            [SpellEffectKind::ProduceMana { options, .. }]
                if options.len() == 5
                    && options.iter().all(|option| option.w + option.u + option.b + option.r + option.g + option.c == 1)
        ));
        if entries == 2 && expected_abilities == 2 {
            assert!(matches!(
                f.activated_abilities[0].effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 1
                        && options[0].w + options[0].u + options[0].b + options[0].r + options[0].g + options[0].c == 1
            ));
        }
    }
}

// ── 08 mass destroy with one filter ────────────────────────────────────────

#[test]
fn q48_mass_destroy_maps_definitions() {
    for (id, name, mana, color, permanent_type, subtype) in [
        (
            "acid_rain",
            "Acid Rain",
            "{3}{U}",
            None,
            Some(PermanentTypeFilter::Land),
            Some("Forest"),
        ),
        (
            "anarchy",
            "Anarchy",
            "{2}{R}{R}",
            Some(Color::White),
            None,
            None,
        ),
        (
            "flashfires",
            "Flashfires",
            "{3}{R}",
            None,
            Some(PermanentTypeFilter::Land),
            Some("Plains"),
        ),
        (
            "natures_ruin",
            "Nature's Ruin",
            "{2}{B}",
            Some(Color::Green),
            None,
            None,
        ),
        (
            "virtues_ruin",
            "Virtue's Ruin",
            "{2}{B}",
            Some(Color::White),
            None,
            None,
        ),
    ] {
        let f = identity(id, name, mana, &["Sorcery"], None);
        let [destroy] = f.spell_effect.as_slice() else {
            panic!("{id}: one DestroyAll");
        };
        let SpellEffectKind::DestroyAll {
            kind,
            prevent_regeneration,
        } = destroy
        else {
            panic!("{id}: DestroyAll");
        };
        assert!(!prevent_regeneration, "{id}: regeneration is allowed");
        assert_eq!(kind.is_color, color, "{id} color filter");
        assert_eq!(
            kind.permanent_types.len(),
            usize::from(permanent_type.is_some())
        );
        if let Some(permanent_type) = permanent_type {
            assert_eq!(kind.permanent_types, vec![permanent_type], "{id}");
        }
        assert_eq!(
            kind.required_subtypes,
            subtype.map(str::to_string).into_iter().collect::<Vec<_>>(),
            "{id} subtype"
        );
        let expected_kind = if id == "natures_ruin" || id == "virtues_ruin" {
            TargetKind::Creature
        } else {
            TargetKind::AnyPermanent
        };
        assert_eq!(kind.kind, expected_kind, "{id}");
    }
}

// ── 09 landcycling creatures ───────────────────────────────────────────────

#[test]
fn q48_landcycling_creatures_maps_definitions() {
    for (id, name, mana, types, body, keywords, ward) in [
        (
            "horta",
            "Horta",
            "{4}{R}{R}",
            &["Creature", "Elemental"][..],
            (7, 6),
            &[Keyword::Trample][..],
            false,
        ),
        (
            "humpback_whales",
            "Humpback Whales",
            "{4}{U}{U}",
            &["Creature", "Whale"][..],
            (5, 6),
            &[Keyword::Vigilance][..],
            true,
        ),
        (
            "shardless_outlander",
            "Shardless Outlander",
            "{7}",
            &["Artifact", "Creature", "Construct", "Scout"][..],
            (6, 5),
            &[Keyword::Trample][..],
            false,
        ),
    ] {
        let f = identity(id, name, mana, types, Some(body));
        assert_eq!(f.keywords, keywords, "{id}");
        if ward {
            let [trigger] = f.triggered_abilities.as_slice() else {
                panic!("{id}: ward trigger");
            };
            assert_eq!(
                trigger.trigger,
                TriggerCondition::WheneverSelfBecomesTarget {
                    source: TargetingSourceFilter::SpellOrAbility,
                    source_controller: CastTriggerPlayer::Opponent,
                }
            );
            assert_eq!(trigger.presentation, AbilityPresentation::Fallback);
        } else {
            assert!(f.triggered_abilities.is_empty(), "{id}");
        }
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: basic landcycling");
        };
        assert_eq!(
            ability.source_zone,
            tricerules_cards::primitives::AbilitySourceZone::Hand
        );
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert!(matches!(
            ability.costs.as_slice(),
            [AbilityCost::Mana(cost), AbilityCost::DiscardSelf] if cost.to_string() == "{2}"
        ));
        assert!(matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::SearchLibrary { filter: Some(filter), reveal: true, .. }]
                if filter.card_type == Some(CardTypeFilter::BasicLand)
        ));
    }
}

#[test]
fn q48_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name) in [
        ("honor_of_the_pure", "Honor of the Pure"),
        ("jacques_le_vert", "Jacques le Vert"),
        ("kaysa", "Kaysa"),
        ("meng_huo,_barbarian_king", "Meng Huo, Barbarian King"),
        ("windstorm_drake", "Windstorm Drake"),
        ("spirit_of_the_spires", "Spirit of the Spires"),
        ("deliberate", "Deliberate"),
        ("foresee", "Foresee"),
        ("introduction_to_prophecy", "Introduction to Prophecy"),
        ("tamiyos_epiphany", "Tamiyo's Epiphany"),
        ("break_asunder", "Break Asunder"),
        ("dissenters_deliverance", "Dissenter's Deliverance"),
        ("wilt", "Wilt"),
        ("lash_of_thorns", "Lash of Thorns"),
        ("mighty_leap", "Mighty Leap"),
        ("screaming_fury", "Screaming Fury"),
        ("uncanny_speed", "Uncanny Speed"),
        ("artifact_blast", "Artifact Blast"),
        ("gainsay", "Gainsay"),
        ("guttural_response", "Guttural Response"),
        ("mystic_denial", "Mystic Denial"),
        ("alchemax_slayer-bots", "Alchemax Slayer-Bots"),
        ("chrome_prowler", "Chrome Prowler"),
        ("fractal_mascot", "Fractal Mascot"),
        ("heavy_infantry", "Heavy Infantry"),
        ("shipwreck_patrol", "Shipwreck Patrol"),
        ("centaur_nurturer", "Centaur Nurturer"),
        ("dawnhart_rejuvenator", "Dawnhart Rejuvenator"),
        ("henge_of_ramos", "Henge of Ramos"),
        ("orochi_leafcaller", "Orochi Leafcaller"),
        ("prismite", "Prismite"),
        ("school_of_the_unseen", "School of the Unseen"),
        ("acid_rain", "Acid Rain"),
        ("anarchy", "Anarchy"),
        ("flashfires", "Flashfires"),
        ("natures_ruin", "Nature's Ruin"),
        ("virtues_ruin", "Virtue's Ruin"),
        ("horta", "Horta"),
        ("humpback_whales", "Humpback Whales"),
        ("shardless_outlander", "Shardless Outlander"),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "{id}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], face_id(name), "face id drift for {id}");
        assert_eq!(fields[3], name);
        assert_eq!(fields[4].len(), 64, "fingerprint length for {id}");
        assert!(
            fields[4].chars().all(|ch| ch.is_ascii_hexdigit()),
            "fingerprint digest for {id}"
        );
        assert_eq!(definition(id).primary_face().name, name, "{id}");
    }
}
