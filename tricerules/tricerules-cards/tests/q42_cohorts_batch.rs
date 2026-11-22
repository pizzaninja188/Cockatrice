//! Registry conformance for the queue-v42 reuse-only cohorts.
#![allow(clippy::type_complexity)]
use tricerules_cards::primitives::{
    AbilityCost, Amount, CardTypeFilter, CastTriggerPlayer, Color, CounterKind, EffectSubject,
    Keyword, PlayerRecipient, ProtectionQuality, SpellEffectKind, StaticAbilityDef, TargetKind,
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
    power: u32,
    toughness: u32,
}

#[test]
fn q42_cycling_keyword_maps_definitions() {
    let cards = [
        Card {
            id: "wasteland_scorpion",
            name: "Wasteland Scorpion",
            mana: "{2}{B}",
            types: &["Creature", "Scorpion"],
            power: 2,
            toughness: 2,
        },
        Card {
            id: "lava_serpent",
            name: "Lava Serpent",
            mana: "{5}{R}",
            types: &["Creature", "Elemental", "Serpent"],
            power: 5,
            toughness: 5,
        },
        Card {
            id: "bloated_toad",
            name: "Bloated Toad",
            mana: "{2}{G}",
            types: &["Creature", "Frog"],
            power: 2,
            toughness: 2,
        },
        Card {
            id: "disciple_of_malice",
            name: "Disciple of Malice",
            mana: "{1}{B}",
            types: &["Creature", "Human", "Cleric"],
            power: 1,
            toughness: 2,
        },
        Card {
            id: "moaning_wall",
            name: "Moaning Wall",
            mana: "{2}{B}",
            types: &["Creature", "Zombie", "Wall"],
            power: 0,
            toughness: 5,
        },
        Card {
            id: "imposing_vantasaur",
            name: "Imposing Vantasaur",
            mana: "{5}{W}",
            types: &["Creature", "Dinosaur"],
            power: 3,
            toughness: 6,
        },
        Card {
            id: "granitic_titan",
            name: "Granitic Titan",
            mana: "{4}{R}{R}",
            types: &["Creature", "Elemental"],
            power: 5,
            toughness: 4,
        },
        Card {
            id: "rampaging_hippo",
            name: "Rampaging Hippo",
            mana: "{4}{G}{G}",
            types: &["Creature", "Hippo"],
            power: 5,
            toughness: 6,
        },
    ];
    let costs = [
        ("wasteland_scorpion", "{2}", Some(Keyword::Deathtouch), None),
        ("lava_serpent", "{2}", Some(Keyword::Haste), None),
        ("bloated_toad", "{2}", None, Some(Color::Blue)),
        ("disciple_of_malice", "{2}", None, Some(Color::White)),
        ("moaning_wall", "{2}", Some(Keyword::Defender), None),
        ("imposing_vantasaur", "{1}", Some(Keyword::Vigilance), None),
        ("granitic_titan", "{2}", Some(Keyword::Menace), None),
        ("rampaging_hippo", "{2}", Some(Keyword::Trample), None),
    ];
    let registry = tricerules_cards::registry::global();
    for card in cards {
        let f = definition(card.id).primary_face();
        assert_eq!(definition(card.id).layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        assert_eq!(f.face_id.as_str(), card.id);
        assert_eq!(f.mana_cost.to_string(), card.mana);
        assert_eq!(f.types, card.types);
        assert_eq!(
            (f.power, f.toughness),
            (Some(card.power), Some(card.toughness))
        );
        let (id, cost, keyword, protection) = costs
            .iter()
            .find(|(id, ..)| *id == card.id)
            .expect("cycling case");
        assert!(*id == card.id);
        assert!(f.triggered_abilities.is_empty());
        assert!(f.static_abilities.is_empty());
        assert_eq!(f.activated_abilities.len(), 1);
        let ability = &f.activated_abilities[0];
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert_eq!(ability.costs.len(), 2);
        assert_eq!(mana_cost(&ability.costs[0]), *cost);
        assert_eq!(ability.costs[1], AbilityCost::DiscardSelf);
        assert!(matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::Draw {
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
            }]
        ));
        match keyword {
            Some(keyword) => {
                assert_eq!(f.keywords, vec![*keyword]);
                assert!(f.protections.is_empty());
            }
            None => {
                assert!(f.keywords.is_empty());
                assert_eq!(
                    f.protections,
                    vec![ProtectionQuality::Color(protection.expect("color"))]
                );
            }
        }
    }
}

#[test]
fn q42_regenerate_self_maps_definitions() {
    let cases: [(&str, &str, &str, &[&str], u32, u32, &str, Keyword); 6] = [
        (
            "living_airship",
            "Living Airship",
            "{3}{U}",
            &["Creature", "Metathran"],
            2,
            3,
            "{2}{G}",
            Keyword::Flying,
        ),
        (
            "screeching_harpy",
            "Screeching Harpy",
            "{2}{B}{B}",
            &["Creature", "Harpy", "Beast"],
            2,
            2,
            "{1}{B}",
            Keyword::Flying,
        ),
        (
            "carnassid",
            "Carnassid",
            "{4}{G}{G}",
            &["Creature", "Beast"],
            5,
            4,
            "{1}{G}",
            Keyword::Trample,
        ),
        (
            "manor_skeleton",
            "Manor Skeleton",
            "{1}{B}",
            &["Creature", "Skeleton"],
            1,
            1,
            "{1}{B}",
            Keyword::Haste,
        ),
        (
            "lim-duls_high_guard",
            "Lim-Dûl's High Guard",
            "{1}{B}{B}",
            &["Creature", "Skeleton"],
            2,
            1,
            "{1}{B}",
            Keyword::FirstStrike,
        ),
        (
            "sanguine_guard",
            "Sanguine Guard",
            "{1}{B}{B}",
            &["Creature", "Phyrexian", "Zombie", "Knight"],
            2,
            2,
            "{1}{B}",
            Keyword::FirstStrike,
        ),
    ];
    for (id, name, mana, types, power, toughness, regenerate, keyword) in cases {
        let f = definition(id).primary_face();
        assert_eq!(f.name, name);
        assert_eq!(f.mana_cost.to_string(), mana);
        assert_eq!(f.types, types);
        assert_eq!((f.power, f.toughness), (Some(power), Some(toughness)));
        assert_eq!(f.keywords, vec![keyword]);
        assert_eq!(f.activated_abilities.len(), 1, "{id}");
        let ability = &f.activated_abilities[0];
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert_eq!(ability.costs.len(), 1, "{id}");
        assert_eq!(mana_cost(&ability.costs[0]), regenerate, "{id}");
        assert!(matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::Regenerate {
                subject: EffectSubject::Source,
            }]
        ));
    }
}

#[test]
fn q42_landfall_pump_maps_definitions() {
    let cases: [(&str, &str, &str, &[&str], u32, u32, Option<Keyword>, bool); 5] = [
        (
            "wave-wing_elemental",
            "Wave-Wing Elemental",
            "{5}{U}",
            &["Creature", "Elemental"],
            3,
            4,
            Some(Keyword::Flying),
            false,
        ),
        (
            "hagra_crocodile",
            "Hagra Crocodile",
            "{3}{B}",
            &["Creature", "Crocodile"],
            3,
            1,
            None,
            true,
        ),
        (
            "windrider_eel",
            "Windrider Eel",
            "{3}{U}",
            &["Creature", "Fish"],
            2,
            2,
            Some(Keyword::Flying),
            false,
        ),
        (
            "ondu_greathorn",
            "Ondu Greathorn",
            "{3}{W}",
            &["Creature", "Beast"],
            2,
            3,
            Some(Keyword::FirstStrike),
            false,
        ),
        (
            "plated_geopede",
            "Plated Geopede",
            "{1}{R}",
            &["Creature", "Insect"],
            1,
            1,
            Some(Keyword::FirstStrike),
            false,
        ),
    ];
    for (id, name, mana, types, power, toughness, keyword, cant_block) in cases {
        let f = definition(id).primary_face();
        assert_eq!(f.name, name);
        assert_eq!(f.mana_cost.to_string(), mana);
        assert_eq!(f.types, types);
        assert_eq!((f.power, f.toughness), (Some(power), Some(toughness)));
        assert_eq!(f.keywords, keyword.into_iter().collect::<Vec<_>>());
        let has_restriction = f.static_abilities.iter().any(|ability| {
            matches!(
                &ability.definition,
                StaticAbilityDef::SelfCombatRestriction { restriction, .. }
                    if restriction.cant_block
            )
        });
        assert_eq!(has_restriction, cant_block, "{id}");
        assert_eq!(f.static_abilities.len(), usize::from(cant_block), "{id}");
        assert_eq!(f.triggered_abilities.len(), 1, "{id}");
        let trigger = &f.triggered_abilities[0];
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert!(matches!(
            &trigger.trigger,
            TriggerCondition::WheneverPermanentEntersBattlefield {
                controller: CastTriggerPlayer::Controller,
                ..
            }
        ));
        assert!(matches!(
            trigger.effect.as_slice(),
            [SpellEffectKind::PumpTarget {
                power: 2,
                toughness: 2,
                subject: EffectSubject::Source,
                ..
            }]
        ));
    }
}

#[test]
fn q42_cast_trigger_pump_maps_definitions() {
    let cases: [(&str, &str, &str, u32, u32, Option<Keyword>, u16); 5] = [
        (
            "mercurial_geists",
            "Mercurial Geists",
            "{2}{U}{R}",
            1,
            3,
            Some(Keyword::Flying),
            2,
        ),
        (
            "fire_urchin",
            "Fire Urchin",
            "{1}{R}",
            1,
            3,
            Some(Keyword::Trample),
            2,
        ),
        (
            "festival_crasher",
            "Festival Crasher",
            "{1}{R}",
            1,
            3,
            None,
            1,
        ),
        (
            "wee_dragonauts",
            "Wee Dragonauts",
            "{1}{U}{R}",
            1,
            3,
            Some(Keyword::Flying),
            2,
        ),
        ("kiln_fiend", "Kiln Fiend", "{1}{R}", 1, 2, None, 1),
    ];
    let pumps = [
        ("mercurial_geists", 3, 0),
        ("fire_urchin", 1, 0),
        ("festival_crasher", 2, 0),
        ("wee_dragonauts", 2, 0),
        ("kiln_fiend", 3, 0),
    ];
    for (id, name, mana, power, toughness, keyword, line) in cases {
        let f = definition(id).primary_face();
        assert_eq!(f.name, name);
        assert_eq!(f.mana_cost.to_string(), mana);
        assert_eq!((f.power, f.toughness), (Some(power), Some(toughness)));
        assert_eq!(f.keywords, keyword.into_iter().collect::<Vec<_>>());
        assert_eq!(f.triggered_abilities.len(), 1, "{id}");
        let trigger = &f.triggered_abilities[0];
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![line])
        );
        assert!(matches!(
            &trigger.trigger,
            TriggerCondition::WheneverPlayerCastsSpell {
                caster: CastTriggerPlayer::Controller,
                filter,
                ..
            } if filter.card_type == Some(CardTypeFilter::InstantOrSorcery)
        ));
        let (_, dp, dt) = pumps
            .iter()
            .find(|(pump_id, ..)| *pump_id == id)
            .expect("pump case");
        assert!(matches!(
            trigger.effect.as_slice(),
            [SpellEffectKind::PumpTarget {
                power,
                toughness,
                subject: EffectSubject::Source,
                ..
            }] if *power == *dp && *toughness == *dt
        ));
    }
}

#[test]
fn q42_explore_etb_maps_definitions() {
    let cases: [(&str, &str, &str, &[&str], u32, u32, Keyword); 6] = [
        (
            "queens_agent",
            "Queen's Agent",
            "{5}{B}",
            &["Creature", "Vampire", "Scout"],
            3,
            3,
            Keyword::Lifelink,
        ),
        (
            "dire_fleet_interloper",
            "Dire Fleet Interloper",
            "{3}{B}",
            &["Creature", "Human", "Pirate"],
            2,
            2,
            Keyword::Menace,
        ),
        (
            "siren_lookout",
            "Siren Lookout",
            "{2}{U}",
            &["Creature", "Siren", "Pirate"],
            1,
            2,
            Keyword::Flying,
        ),
        (
            "brazen_buccaneers",
            "Brazen Buccaneers",
            "{3}{R}",
            &["Creature", "Human", "Pirate"],
            2,
            2,
            Keyword::Haste,
        ),
        (
            "sunrise_seeker",
            "Sunrise Seeker",
            "{4}{W}",
            &["Creature", "Human", "Scout"],
            3,
            3,
            Keyword::Vigilance,
        ),
        (
            "emissary_of_sunrise",
            "Emissary of Sunrise",
            "{2}{W}",
            &["Creature", "Human", "Cleric"],
            2,
            1,
            Keyword::FirstStrike,
        ),
    ];
    for (id, name, mana, types, power, toughness, keyword) in cases {
        let f = definition(id).primary_face();
        assert_eq!(f.name, name);
        assert_eq!(f.mana_cost.to_string(), mana);
        assert_eq!(f.types, types);
        assert_eq!((f.power, f.toughness), (Some(power), Some(toughness)));
        assert_eq!(f.keywords, vec![keyword]);
        assert_eq!(f.triggered_abilities.len(), 1, "{id}");
        let trigger = &f.triggered_abilities[0];
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert!(matches!(
            trigger.effect.as_slice(),
            [SpellEffectKind::Explore {
                subject: EffectSubject::Source,
            }]
        ));
    }
}

#[test]
fn q42_combat_damage_counter_maps_definitions() {
    let cases: [(&str, &str, &str, u32, u32, Option<Keyword>); 5] = [
        ("stromkirk_patrol", "Stromkirk Patrol", "{4}{B}", 4, 3, None),
        (
            "erdwal_ripper",
            "Erdwal Ripper",
            "{1}{R}{R}",
            2,
            1,
            Some(Keyword::Haste),
        ),
        (
            "slith_ascendant",
            "Slith Ascendant",
            "{1}{W}{W}",
            1,
            1,
            Some(Keyword::Flying),
        ),
        (
            "slith_firewalker",
            "Slith Firewalker",
            "{R}{R}",
            1,
            1,
            Some(Keyword::Haste),
        ),
        (
            "slith_predator",
            "Slith Predator",
            "{G}{G}",
            1,
            1,
            Some(Keyword::Trample),
        ),
    ];
    for (id, name, mana, power, toughness, keyword) in cases {
        let f = definition(id).primary_face();
        assert_eq!(f.name, name);
        assert_eq!(f.mana_cost.to_string(), mana);
        assert_eq!((f.power, f.toughness), (Some(power), Some(toughness)));
        assert_eq!(f.keywords, keyword.into_iter().collect::<Vec<_>>());
        assert_eq!(f.triggered_abilities.len(), 1, "{id}");
        let trigger = &f.triggered_abilities[0];
        assert_eq!(
            trigger.trigger,
            TriggerCondition::WheneverSelfDealsCombatDamageToPlayer
        );
        assert!(matches!(
            trigger.effect.as_slice(),
            [SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                subject: EffectSubject::Source,
            }]
        ));
    }
}

#[test]
fn q42_activated_self_pump_maps_definitions() {
    let cases: [(&str, &str, &str, u32, u32, &str, i32, i32); 6] = [
        (
            "ember-eye_wolf",
            "Ember-Eye Wolf",
            "{1}{R}",
            1,
            2,
            "{1}{R}",
            2,
            0,
        ),
        (
            "lionheart_maverick",
            "Lionheart Maverick",
            "{W}",
            1,
            1,
            "{4}{W}",
            1,
            2,
        ),
        (
            "rune-cervin_rider",
            "Rune-Cervin Rider",
            "{3}{W}",
            2,
            2,
            "{G/W}{G/W}",
            1,
            1,
        ),
        (
            "inkrise_infiltrator",
            "Inkrise Infiltrator",
            "{1}{B}",
            1,
            2,
            "{3}{B}",
            2,
            2,
        ),
        (
            "wildfire_emissary",
            "Wildfire Emissary",
            "{3}{R}",
            2,
            4,
            "{1}{R}",
            1,
            0,
        ),
        (
            "thirsting_shade",
            "Thirsting Shade",
            "{B}",
            1,
            1,
            "{2}{B}",
            1,
            1,
        ),
    ];
    for (id, name, mana, power, toughness, activation, dp, dt) in cases {
        let f = definition(id).primary_face();
        assert_eq!(f.name, name);
        assert_eq!(f.mana_cost.to_string(), mana);
        assert_eq!((f.power, f.toughness), (Some(power), Some(toughness)));
        assert_eq!(f.activated_abilities.len(), 1, "{id}");
        let ability = &f.activated_abilities[0];
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert_eq!(ability.costs.len(), 1, "{id}");
        assert_eq!(mana_cost(&ability.costs[0]), activation, "{id}");
        assert!(matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::PumpTarget {
                power,
                toughness,
                subject: EffectSubject::Source,
                ..
            }] if *power == dp && *toughness == dt
        ));
    }
    assert_eq!(
        definition("wildfire_emissary").primary_face().protections,
        [ProtectionQuality::Color(Color::White)]
    );
}

#[test]
fn q42_attacking_pump_maps_definitions() {
    let cases: [(&str, &str, &str, i32, i32, Option<Keyword>); 5] = [
        ("army_of_allah", "Army of Allah", "{1}{W}{W}", 2, 0, None),
        ("morale", "Morale", "{1}{W}{W}", 1, 1, None),
        (
            "rally_the_forces",
            "Rally the Forces",
            "{2}{R}",
            1,
            0,
            Some(Keyword::FirstStrike),
        ),
        (
            "stampede",
            "Stampede",
            "{1}{G}{G}",
            1,
            0,
            Some(Keyword::Trample),
        ),
        (
            "volcanic_rush",
            "Volcanic Rush",
            "{4}{R}",
            2,
            0,
            Some(Keyword::Trample),
        ),
    ];
    for (id, name, mana, dp, dt, keyword) in cases {
        let f = definition(id).primary_face();
        assert_eq!(f.name, name);
        assert_eq!(f.mana_cost.to_string(), mana);
        assert_eq!(f.types, vec!["Instant"]);
        assert_eq!(f.spell_effect.len(), if keyword.is_some() { 2 } else { 1 });
        assert!(matches!(
            &f.spell_effect[0],
            SpellEffectKind::PumpAll {
                filter,
                power,
                toughness,
            } if filter.attacking
                && filter.controller.is_none()
                && *power == dp
                && *toughness == dt
        ));
        match keyword {
            Some(keyword) => assert!(matches!(
                &f.spell_effect[1],
                SpellEffectKind::GrantKeywordsAll {
                    filter,
                    keywords,
                } if filter.attacking
                    && filter.controller.is_none()
                    && keywords == &vec![keyword]
            )),
            None => assert_eq!(f.spell_effect.len(), 1),
        }
        assert!(f.targeting.is_none(), "{id}: no target");
    }
}

#[test]
fn q42_pump_keyword_maps_definitions() {
    let cases: [(&str, &str, &str, i32, i32, Keyword, bool); 6] = [
        (
            "slaughter_cry",
            "Slaughter Cry",
            "{2}{R}",
            3,
            0,
            Keyword::FirstStrike,
            false,
        ),
        (
            "thunder_strike",
            "Thunder Strike",
            "{1}{R}",
            2,
            0,
            Keyword::FirstStrike,
            false,
        ),
        (
            "unlikely_aid",
            "Unlikely Aid",
            "{1}{B}",
            2,
            0,
            Keyword::Indestructible,
            false,
        ),
        (
            "mortals_resolve",
            "Mortal's Resolve",
            "{1}{G}",
            1,
            1,
            Keyword::Indestructible,
            false,
        ),
        (
            "wings_of_the_cosmos",
            "Wings of the Cosmos",
            "{W}",
            1,
            3,
            Keyword::Flying,
            true,
        ),
        (
            "escape_from_orthanc",
            "Escape from Orthanc",
            "{W}",
            1,
            3,
            Keyword::Flying,
            true,
        ),
    ];
    for (id, name, mana, dp, dt, keyword, untap) in cases {
        let f = definition(id).primary_face();
        assert_eq!(f.name, name);
        assert_eq!(f.mana_cost.to_string(), mana);
        assert_eq!(f.types, vec!["Instant"]);
        assert_eq!(f.spell_effect.len(), if untap { 3 } else { 2 }, "{id}");
        assert!(matches!(
            f.spell_effect[0],
            SpellEffectKind::PumpTarget {
                power,
                toughness,
                subject: EffectSubject::Chosen(ref filter),
                ..
            } if power == dp && toughness == dt && filter.kind == TargetKind::Creature
        ));
        assert!(matches!(
            f.spell_effect[1],
            SpellEffectKind::GrantKeywords {
                subject: EffectSubject::Chosen(ref filter),
                ref keywords,
            } if filter.kind == TargetKind::Creature && keywords == &vec![keyword]
        ));
        if untap {
            assert!(matches!(
                f.spell_effect[2],
                SpellEffectKind::Untap {
                    subject: EffectSubject::Chosen(ref filter),
                } if filter.kind == TargetKind::Creature
            ));
        }
        let groups = &f.targeting.as_ref().expect("one target group").groups;
        assert_eq!(groups.len(), 1, "{id}");
        assert_eq!((groups[0].min, groups[0].max), (1, 1), "{id}");
        assert_eq!(
            groups[0].effect_indices,
            if untap { vec![0, 1, 2] } else { vec![0, 1] },
            "{id}"
        );
    }
}

#[test]
fn q42_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    let cards: [(&str, &str, &str); 52] = [
        (
            "wasteland_scorpion",
            "Wasteland Scorpion",
            "wasteland_scorpion",
        ),
        ("lava_serpent", "Lava Serpent", "lava_serpent"),
        ("bloated_toad", "Bloated Toad", "bloated_toad"),
        (
            "disciple_of_malice",
            "Disciple of Malice",
            "disciple_of_malice",
        ),
        ("moaning_wall", "Moaning Wall", "moaning_wall"),
        (
            "imposing_vantasaur",
            "Imposing Vantasaur",
            "imposing_vantasaur",
        ),
        ("granitic_titan", "Granitic Titan", "granitic_titan"),
        ("rampaging_hippo", "Rampaging Hippo", "rampaging_hippo"),
        ("living_airship", "Living Airship", "living_airship"),
        ("screeching_harpy", "Screeching Harpy", "screeching_harpy"),
        ("carnassid", "Carnassid", "carnassid"),
        ("manor_skeleton", "Manor Skeleton", "manor_skeleton"),
        (
            "lim-duls_high_guard",
            "Lim-Dûl's High Guard",
            "lim_duls_high_guard",
        ),
        ("sanguine_guard", "Sanguine Guard", "sanguine_guard"),
        (
            "wave-wing_elemental",
            "Wave-Wing Elemental",
            "wave_wing_elemental",
        ),
        ("hagra_crocodile", "Hagra Crocodile", "hagra_crocodile"),
        ("windrider_eel", "Windrider Eel", "windrider_eel"),
        ("ondu_greathorn", "Ondu Greathorn", "ondu_greathorn"),
        ("plated_geopede", "Plated Geopede", "plated_geopede"),
        ("mercurial_geists", "Mercurial Geists", "mercurial_geists"),
        ("fire_urchin", "Fire Urchin", "fire_urchin"),
        ("festival_crasher", "Festival Crasher", "festival_crasher"),
        ("wee_dragonauts", "Wee Dragonauts", "wee_dragonauts"),
        ("kiln_fiend", "Kiln Fiend", "kiln_fiend"),
        ("queens_agent", "Queen's Agent", "queens_agent"),
        (
            "dire_fleet_interloper",
            "Dire Fleet Interloper",
            "dire_fleet_interloper",
        ),
        ("siren_lookout", "Siren Lookout", "siren_lookout"),
        (
            "brazen_buccaneers",
            "Brazen Buccaneers",
            "brazen_buccaneers",
        ),
        ("sunrise_seeker", "Sunrise Seeker", "sunrise_seeker"),
        (
            "emissary_of_sunrise",
            "Emissary of Sunrise",
            "emissary_of_sunrise",
        ),
        ("stromkirk_patrol", "Stromkirk Patrol", "stromkirk_patrol"),
        ("erdwal_ripper", "Erdwal Ripper", "erdwal_ripper"),
        ("slith_ascendant", "Slith Ascendant", "slith_ascendant"),
        ("slith_firewalker", "Slith Firewalker", "slith_firewalker"),
        ("slith_predator", "Slith Predator", "slith_predator"),
        ("ember-eye_wolf", "Ember-Eye Wolf", "ember_eye_wolf"),
        (
            "lionheart_maverick",
            "Lionheart Maverick",
            "lionheart_maverick",
        ),
        (
            "rune-cervin_rider",
            "Rune-Cervin Rider",
            "rune_cervin_rider",
        ),
        (
            "inkrise_infiltrator",
            "Inkrise Infiltrator",
            "inkrise_infiltrator",
        ),
        (
            "wildfire_emissary",
            "Wildfire Emissary",
            "wildfire_emissary",
        ),
        ("thirsting_shade", "Thirsting Shade", "thirsting_shade"),
        ("army_of_allah", "Army of Allah", "army_of_allah"),
        ("morale", "Morale", "morale"),
        ("rally_the_forces", "Rally the Forces", "rally_the_forces"),
        ("stampede", "Stampede", "stampede"),
        ("volcanic_rush", "Volcanic Rush", "volcanic_rush"),
        ("slaughter_cry", "Slaughter Cry", "slaughter_cry"),
        ("thunder_strike", "Thunder Strike", "thunder_strike"),
        ("unlikely_aid", "Unlikely Aid", "unlikely_aid"),
        ("mortals_resolve", "Mortal's Resolve", "mortals_resolve"),
        (
            "wings_of_the_cosmos",
            "Wings of the Cosmos",
            "wings_of_the_cosmos",
        ),
        (
            "escape_from_orthanc",
            "Escape from Orthanc",
            "escape_from_orthanc",
        ),
    ];
    for (id, name, face_id) in cards {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "{id}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], face_id, "face id drift for {id}");
        assert_eq!(fields[3], name);
        assert_eq!(fields[4].len(), 64, "fingerprint length for {id}");
        assert!(
            fields[4].chars().all(|ch| ch.is_ascii_hexdigit()),
            "fingerprint digest for {id}"
        );
        assert_eq!(
            definition(id).primary_face().name,
            name,
            "registry name for {id}"
        );
    }
}
