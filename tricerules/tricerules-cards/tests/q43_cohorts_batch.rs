//! Registry conformance for the queue-v43 reuse-only cohorts.
#![allow(clippy::type_complexity)]
use tricerules_cards::primitives::{
    AbilityCost, Amount, CardTypeFilter, Color, CombatRestrictionScope, CounterKind, EffectSubject,
    HandCardAction, Keyword, PowerComparison, ProtectionQuality, SpellEffectKind, TargetController,
    TargetKind, TriggerCondition,
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

#[test]
fn q43_aura_pump_maps_definitions() {
    let cases = [
        Card {
            id: "blessing",
            name: "Blessing",
            mana: "{W}{W}",
            types: &["Enchantment", "Aura"],
            body: None,
        },
        Card {
            id: "flowstone_blade",
            name: "Flowstone Blade",
            mana: "{R}",
            types: &["Enchantment", "Aura"],
            body: None,
        },
        Card {
            id: "firebreathing",
            name: "Firebreathing",
            mana: "{R}",
            types: &["Enchantment", "Aura"],
            body: None,
        },
    ];
    let pumps = [
        ("blessing", "{W}", (1, 1)),
        ("flowstone_blade", "{R}", (1, -1)),
        ("firebreathing", "{R}", (1, 0)),
    ];
    for card in &cases {
        let f = identity(card);
        let [SpellEffectKind::AuraAttach { .. }] = f.spell_effect.as_slice() else {
            panic!("{}: AuraAttach", card.id);
        };
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: one activated ability", card.id);
        };
        let (_, cost, (dp, dt)) = pumps.iter().find(|(id, ..)| *id == card.id).expect("pump");
        assert_eq!(mana_cost(&ability.costs[0]), *cost, "{}", card.id);
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{}",
            card.id
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::PumpTarget { power, toughness, subject: EffectSubject::AttachedObject, .. }]
                    if (*power, *toughness) == (*dp, *dt)
            ),
            "{}: attached pump",
            card.id
        );
    }
}

#[test]
fn q43_goblin_tokens_maps_definitions() {
    let cases = [
        ("goblin_rally", "Goblin Rally", "{3}{R}{R}", 4u32),
        ("krenkos_command", "Krenko's Command", "{1}{R}", 2),
        ("hordeling_outburst", "Hordeling Outburst", "{1}{R}{R}", 3),
    ];
    for (id, name, mana, count) in cases {
        let f = identity(&Card {
            id,
            name,
            mana,
            types: &["Sorcery"],
            body: None,
        });
        assert!(
            matches!(
                f.spell_effect.as_slice(),
                [SpellEffectKind::CreateTokens { token, count: Amount::Fixed(actual), .. }]
                    if token == "goblin_r_1_1" && *actual == count
            ),
            "{id}: {} Goblin tokens",
            count
        );
    }
}

#[test]
fn q43_activated_target_pump_maps_definitions() {
    let cases = [
        (
            "flowstone_overseer",
            "Flowstone Overseer",
            "{2}{R}{R}{R}",
            &["Creature", "Beast"][..],
            (4, 4),
            "{R}{R}",
            (1, -1),
        ),
        (
            "devotee_of_strength",
            "Devotee of Strength",
            "{2}{G}",
            &["Creature", "Snake", "Wizard"][..],
            (3, 2),
            "{4}{G}",
            (2, 2),
        ),
        (
            "smokespew_invoker",
            "Smokespew Invoker",
            "{2}{B}",
            &["Creature", "Zombie", "Mutant"][..],
            (3, 1),
            "{7}{B}",
            (-3, -3),
        ),
        (
            "hagra_sharpshooter",
            "Hagra Sharpshooter",
            "{2}{B}",
            &["Creature", "Human", "Assassin", "Ally"][..],
            (2, 2),
            "{4}{B}",
            (-1, -1),
        ),
    ];
    for (id, name, mana, types, body, cost, (dp, dt)) in cases {
        let f = identity(&Card {
            id,
            name,
            mana,
            types,
            body: Some(body),
        });
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: one activated ability");
        };
        assert_eq!(mana_cost(&ability.costs[0]), cost, "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::PumpTarget { power, toughness, subject: EffectSubject::Chosen(filter), .. }]
                    if (*power, *toughness) == (dp, dt) && filter.kind == TargetKind::Creature
            ),
            "{id}: chosen-creature pump"
        );
        let Some(targeting) = &ability.targeting else {
            panic!("{id}: targeting")
        };
        assert_eq!(targeting.groups.len(), 1, "{id}");
        assert_eq!(
            (targeting.groups[0].min, targeting.groups[0].max),
            (1, 1),
            "{id}"
        );
        assert_eq!(targeting.groups[0].effect_indices, [0], "{id}");
    }
}

#[test]
fn q43_convoke_pump_maps_definitions() {
    let cases = [
        ("packs_favor", "Pack's Favor", "{2}{G}", (3, 3)),
        ("gather_courage", "Gather Courage", "{G}", (2, 2)),
        (
            "collective_nightmare",
            "Collective Nightmare",
            "{2}{B}",
            (-3, -3),
        ),
    ];
    for (id, name, mana, (dp, dt)) in cases {
        let f = identity(&Card {
            id,
            name,
            mana,
            types: &["Instant"],
            body: None,
        });
        assert_eq!(f.keywords, vec![Keyword::Convoke], "{id}: Convoke");
        assert!(
            matches!(
                f.spell_effect.as_slice(),
                [SpellEffectKind::PumpTarget { power, toughness, subject: EffectSubject::Chosen(filter), .. }]
                    if (*power, *toughness) == (dp, dt) && filter.kind == TargetKind::Creature
            ),
            "{id}: chosen-creature pump"
        );
    }
}

#[test]
fn q43_power_unblockable_maps_definitions() {
    let cases = [
        (
            "tawnoss_wand",
            "Tawnos's Wand",
            "{4}",
            &["Artifact"][..],
            None,
            0usize,
            "{2}",
            2u32,
        ),
        (
            "sneaking_guide",
            "Sneaking Guide",
            "{R}",
            &["Creature", "Goblin", "Rogue"][..],
            Some((1, 1)),
            0,
            "{2}",
            2,
        ),
        (
            "access_tunnel",
            "Access Tunnel",
            "",
            &["Land"][..],
            None,
            1,
            "{3}",
            3,
        ),
    ];
    for (id, name, mana, types, body, index, cost, limit) in cases {
        let f = identity(&Card {
            id,
            name,
            mana,
            types,
            body,
        });
        if id == "access_tunnel" {
            let [first, second] = f.activated_abilities.as_slice() else {
                panic!("{id}: two activated abilities");
            };
            assert!(
                matches!(
                    first.effect.as_slice(),
                    [SpellEffectKind::ProduceMana { options, .. }] if options[0].c == 1
                ),
                "{id}: colorless mana"
            );
            assert_eq!(
                first.presentation,
                AbilityPresentation::OracleLines(vec![1]),
                "{id}"
            );
            assert!(
                matches!(first.costs.as_slice(), [AbilityCost::Tap]),
                "{id}: mana ability tap"
            );
            assert!(
                matches!(
                    second.costs.as_slice(),
                    [AbilityCost::Mana(_), AbilityCost::Tap]
                ),
                "{id}: restriction costs mana and tap"
            );
            assert_eq!(mana_cost(&second.costs[0]), cost, "{id}");
            assert_eq!(
                second.presentation,
                AbilityPresentation::OracleLines(vec![2]),
                "{id}"
            );
        }
        let ability = &f.activated_abilities[index];
        if id != "access_tunnel" {
            assert_eq!(mana_cost(&ability.costs[0]), cost, "{id}");
            assert_eq!(
                ability.presentation,
                AbilityPresentation::OracleLines(vec![1]),
                "{id}"
            );
        }
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::ApplyCombatRestriction { scope: CombatRestrictionScope::Chosen(filter), restriction }]
                    if filter.kind == TargetKind::Creature
                        && filter.power == Some(PowerComparison::AtMost(limit))
                        && restriction.cant_be_blocked
            ),
            "{id}: power-limited unblockable"
        );
    }
}

#[test]
fn q43_activated_self_pump_maps_definitions() {
    let cases = [
        (
            "tyrranax",
            "Tyrranax",
            "{4}{G}{G}",
            &["Creature", "Dinosaur", "Beast"][..],
            (5, 4),
            "{1}{G}",
            (-1, 1),
        ),
        (
            "greater_forgeling",
            "Greater Forgeling",
            "{3}{R}{R}",
            &["Creature", "Elemental"][..],
            (3, 4),
            "{1}{R}",
            (3, -3),
        ),
        (
            "shambling_strider",
            "Shambling Strider",
            "{4}{G}{G}",
            &["Creature", "Yeti"][..],
            (5, 5),
            "{R}{G}",
            (1, -1),
        ),
    ];
    for (id, name, mana, types, body, cost, (dp, dt)) in cases {
        let f = identity(&Card {
            id,
            name,
            mana,
            types,
            body: Some(body),
        });
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: one activated ability");
        };
        assert_eq!(ability.costs.len(), 1, "{id}: mana-only activation");
        assert_eq!(mana_cost(&ability.costs[0]), cost, "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::PumpTarget { power, toughness, subject: EffectSubject::Source, .. }]
                    if (*power, *toughness) == (dp, dt)
            ),
            "{id}: source pump"
        );
    }
}

#[test]
fn q43_discard_spells_maps_definitions() {
    let cases = [
        ("fugue", "Fugue", "{3}{B}{B}", &["Sorcery"][..], 3u32),
        (
            "three_tragedies",
            "Three Tragedies",
            "{3}{B}{B}",
            &["Sorcery", "Arcane"][..],
            3,
        ),
        (
            "waking_nightmare",
            "Waking Nightmare",
            "{2}{B}",
            &["Sorcery", "Arcane"][..],
            2,
        ),
    ];
    for (id, name, mana, types, count) in cases {
        let f = identity(&Card {
            id,
            name,
            mana,
            types,
            body: None,
        });
        assert!(
            matches!(
                f.spell_effect.as_slice(),
                [SpellEffectKind::ChooseHandCards { action: HandCardAction::Discard, count: actual, target, .. }]
                    if *actual == count && target.kind == TargetKind::AnyPlayer
            ),
            "{id}: discard {count} from any player"
        );
    }
}

#[test]
fn q43_etb_counter_target_maps_definitions() {
    let cases = [
        (
            "kujar_seedsculptor",
            "Kujar Seedsculptor",
            "{1}{G}",
            &["Creature", "Elf", "Druid"][..],
            (1, 2),
            CounterKind::PlusOnePlusOne,
        ),
        (
            "teyos_lightshield",
            "Teyo's Lightshield",
            "{2}{W}",
            &["Creature", "Illusion"][..],
            (0, 3),
            CounterKind::PlusOnePlusOne,
        ),
        (
            "ornery_kudu",
            "Ornery Kudu",
            "{2}{G}",
            &["Creature", "Antelope"][..],
            (3, 4),
            CounterKind::MinusOneMinusOne,
        ),
    ];
    for (id, name, mana, types, body, counter) in cases {
        let f = identity(&Card {
            id,
            name,
            mana,
            types,
            body: Some(body),
        });
        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{id}: one trigger");
        };
        assert_eq!(
            trigger.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert!(
            matches!(
                trigger.effect.as_slice(),
                [SpellEffectKind::PutCounters { counter: actual, count: Amount::Fixed(1), subject: EffectSubject::Chosen(filter) }]
                    if *actual == counter && filter.kind == TargetKind::Creature
                        && filter.controller == TargetController::You
            ),
            "{id}: controlled-creature counter"
        );
        let Some(targeting) = &trigger.targeting else {
            panic!("{id}: targeting")
        };
        assert_eq!(
            (targeting.groups[0].min, targeting.groups[0].max),
            (1, 1),
            "{id}"
        );
    }
}

#[test]
fn q43_order_pump_maps_definitions() {
    let cases = [
        (
            "order_of_leitbur",
            "Order of Leitbur",
            "{W}{W}",
            &["Creature", "Human", "Cleric", "Knight"][..],
            Color::Black,
            "{W}",
            "{W}{W}",
        ),
        (
            "order_of_the_white_shield",
            "Order of the White Shield",
            "{W}{W}",
            &["Creature", "Human", "Knight"][..],
            Color::Black,
            "{W}",
            "{W}{W}",
        ),
        (
            "order_of_the_ebon_hand",
            "Order of the Ebon Hand",
            "{B}{B}",
            &["Creature", "Cleric", "Knight"][..],
            Color::White,
            "{B}",
            "{B}{B}",
        ),
        (
            "knight_of_stromgald",
            "Knight of Stromgald",
            "{B}{B}",
            &["Creature", "Human", "Knight"][..],
            Color::White,
            "{B}",
            "{B}{B}",
        ),
    ];
    for (id, name, mana, types, protection, first_cost, pump_cost) in cases {
        let f = identity(&Card {
            id,
            name,
            mana,
            types,
            body: Some((2, 1)),
        });
        assert_eq!(
            f.protections,
            vec![ProtectionQuality::Color(protection)],
            "{id}"
        );
        let [first, second] = f.activated_abilities.as_slice() else {
            panic!("{id}: two activated abilities");
        };
        assert!(
            first.activation_limit.is_none(),
            "{id}: no first-strike activation limit"
        );
        assert_eq!(
            mana_cost(&first.costs[0]),
            first_cost,
            "{id}: first strike cost"
        );
        assert!(
            matches!(
                first.effect.as_slice(),
                [SpellEffectKind::GrantKeywords { subject: EffectSubject::Source, keywords }]
                    if keywords == &[Keyword::FirstStrike]
            ),
            "{id}: first strike"
        );
        assert_eq!(
            first.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id}"
        );
        assert!(
            second.activation_limit.is_none(),
            "{id}: no pump activation limit"
        );
        assert_eq!(mana_cost(&second.costs[0]), pump_cost, "{id}: pump cost");
        assert!(
            matches!(
                second.effect.as_slice(),
                [SpellEffectKind::PumpTarget {
                    power: 1,
                    toughness: 0,
                    subject: EffectSubject::Source,
                    ..
                }]
            ),
            "{id}: pump"
        );
        assert_eq!(
            second.presentation,
            AbilityPresentation::OracleLines(vec![3]),
            "{id}"
        );
        assert!(f.keywords.is_empty(), "{id}: protection is not a keyword");
        let _ = CardTypeFilter::Creature;
    }
}

#[test]
fn q43_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    let cards: [(&str, &str); 29] = [
        ("blessing", "Blessing"),
        ("flowstone_blade", "Flowstone Blade"),
        ("firebreathing", "Firebreathing"),
        ("goblin_rally", "Goblin Rally"),
        ("krenkos_command", "Krenko's Command"),
        ("hordeling_outburst", "Hordeling Outburst"),
        ("flowstone_overseer", "Flowstone Overseer"),
        ("devotee_of_strength", "Devotee of Strength"),
        ("smokespew_invoker", "Smokespew Invoker"),
        ("hagra_sharpshooter", "Hagra Sharpshooter"),
        ("packs_favor", "Pack's Favor"),
        ("gather_courage", "Gather Courage"),
        ("collective_nightmare", "Collective Nightmare"),
        ("tawnoss_wand", "Tawnos's Wand"),
        ("sneaking_guide", "Sneaking Guide"),
        ("access_tunnel", "Access Tunnel"),
        ("tyrranax", "Tyrranax"),
        ("greater_forgeling", "Greater Forgeling"),
        ("shambling_strider", "Shambling Strider"),
        ("fugue", "Fugue"),
        ("three_tragedies", "Three Tragedies"),
        ("waking_nightmare", "Waking Nightmare"),
        ("kujar_seedsculptor", "Kujar Seedsculptor"),
        ("teyos_lightshield", "Teyo's Lightshield"),
        ("ornery_kudu", "Ornery Kudu"),
        ("order_of_leitbur", "Order of Leitbur"),
        ("order_of_the_white_shield", "Order of the White Shield"),
        ("order_of_the_ebon_hand", "Order of the Ebon Hand"),
        ("knight_of_stromgald", "Knight of Stromgald"),
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
        assert_eq!(
            definition(id).primary_face().name,
            name,
            "registry name for {id}"
        );
    }
}
