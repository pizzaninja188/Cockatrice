//! Registry conformance for the queue-v51 reviewed cohorts.
#![allow(clippy::type_complexity)]
use tricerules_cards::primitives::{
    AbilityCost, CardTypeFilter, Keyword, SpellEffectKind, StaticAbilityDef, TargetKind,
    TriggerCondition,
};
use tricerules_cards::AbilityPresentation;
use tricerules_cards::{registry, CardDefinition, CardFace, Layout};

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

fn token_effect(id: &str, token: &str, count: u32) {
    let f = definition(id).primary_face();
    assert!(
        f.spell_effect.iter().any(|effect| matches!(
            effect,
            SpellEffectKind::CreateTokens { token: actual, count: actual_count, .. }
                if actual == token
                    && matches!(actual_count, tricerules_cards::primitives::Amount::Fixed(actual)
                        if *actual == count)
        )),
        "{id}: creates {count} {token}"
    );
}

// ── 01 token spells ─────────────────────────────────────────────────────

#[test]
fn q51_token_spells_map_definitions_and_tokens() {
    for (id, name, mana, types, token, count) in [
        (
            "call_of_the_conclave",
            "Call of the Conclave",
            "{G}{W}",
            &["Sorcery"][..],
            "centaur_g_3_3",
            1,
        ),
        (
            "elemental_summoning",
            "Elemental Summoning",
            "{3}{U/R}{U/R}",
            &["Sorcery", "Lesson"][..],
            "elemental_ur_4_4",
            1,
        ),
        (
            "advent_of_the_wurm",
            "Advent of the Wurm",
            "{1}{G}{G}{W}",
            &["Instant"][..],
            "wurm_g_5_5_trample",
            1,
        ),
        (
            "spore_swarm",
            "Spore Swarm",
            "{3}{G}",
            &["Instant"][..],
            "saproling_g_1_1",
            3,
        ),
        (
            "masters_call",
            "Master's Call",
            "{2}{W}",
            &["Instant"][..],
            "myr_token_c_1_1",
            2,
        ),
        (
            "servo_exhibition",
            "Servo Exhibition",
            "{1}{W}",
            &["Sorcery"][..],
            "servo_c_1_1",
            2,
        ),
        (
            "sworn_companions",
            "Sworn Companions",
            "{2}{W}",
            &["Sorcery"][..],
            "soldier_w_1_1_lifelink",
            2,
        ),
        (
            "icatian_town",
            "Icatian Town",
            "{5}{W}",
            &["Sorcery"][..],
            "citizen_w_1_1",
            4,
        ),
        (
            "knight_watch",
            "Knight Watch",
            "{4}{W}",
            &["Sorcery"][..],
            "knight_w_2_2_vigilance",
            2,
        ),
        (
            "flurry_of_horns",
            "Flurry of Horns",
            "{4}{R}",
            &["Sorcery"][..],
            "minotaur_r_2_3_haste",
            2,
        ),
        (
            "take_up_arms",
            "Take Up Arms",
            "{4}{W}",
            &["Instant"][..],
            "warrior_w_1_1",
            3,
        ),
    ] {
        identity(id, name, mana, types, None);
        token_effect(id, token, count);
    }
    for (token, name, types, color, pt, keyword) in [
        (
            "centaur_g_3_3",
            "Centaur",
            &["Creature", "Centaur"][..],
            "g",
            (3, 3),
            None,
        ),
        (
            "elemental_ur_4_4",
            "Elemental",
            &["Creature", "Elemental"][..],
            "ur",
            (4, 4),
            None,
        ),
        (
            "wurm_g_5_5_trample",
            "Wurm",
            &["Creature", "Wurm"][..],
            "g",
            (5, 5),
            Some(Keyword::Trample),
        ),
        (
            "servo_c_1_1",
            "Servo",
            &["Artifact", "Creature", "Servo"][..],
            "",
            (1, 1),
            None,
        ),
        (
            "soldier_w_1_1_lifelink",
            "Soldier",
            &["Creature", "Soldier"][..],
            "w",
            (1, 1),
            Some(Keyword::Lifelink),
        ),
        (
            "citizen_w_1_1",
            "Citizen",
            &["Creature", "Citizen"][..],
            "w",
            (1, 1),
            None,
        ),
        (
            "minotaur_r_2_3_haste",
            "Minotaur",
            &["Creature", "Minotaur"][..],
            "r",
            (2, 3),
            Some(Keyword::Haste),
        ),
        (
            "warrior_w_1_1",
            "Warrior",
            &["Creature", "Warrior"][..],
            "w",
            (1, 1),
            None,
        ),
        (
            "golem_c_3_3",
            "Golem",
            &["Artifact", "Creature", "Phyrexian", "Golem"][..],
            "",
            (3, 3),
            None,
        ),
    ] {
        let token_def = definition(token);
        assert!(
            registry::global().is_token(token),
            "{token} is a token template"
        );
        let f = token_def.primary_face();
        assert_eq!(f.name, name, "{token} name");
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            types,
            "{token} types"
        );
        let mut letters = f
            .colors()
            .iter()
            .map(|color| match format!("{color:?}").as_str() {
                "White" => 'w',
                "Blue" => 'u',
                "Black" => 'b',
                "Red" => 'r',
                "Green" => 'g',
                _ => '?',
            })
            .collect::<Vec<_>>();
        letters.sort_by_key(|letter| "wubrg".find(*letter).unwrap_or(5));
        assert_eq!(
            letters.into_iter().collect::<String>(),
            color,
            "{token} color"
        );
        assert_eq!(
            (f.power, f.toughness),
            (Some(pt.0), Some(pt.1)),
            "{token} body"
        );
        assert_eq!(
            f.keywords,
            keyword.into_iter().collect::<Vec<_>>(),
            "{token} keywords"
        );
    }
}

// ── 02 splicer cycle ────────────────────────────────────────────────────

#[test]
fn q51_splicers_map_entry_and_lord_profiles() {
    for (id, name, mana, lord) in [
        (
            "wing_splicer",
            "Wing Splicer",
            "{3}{U}",
            Some(Keyword::Flying),
        ),
        (
            "sensor_splicer",
            "Sensor Splicer",
            "{4}{W}",
            Some(Keyword::Vigilance),
        ),
        ("master_splicer", "Master Splicer", "{3}{W}", None),
        (
            "blade_splicer",
            "Blade Splicer",
            "{2}{W}",
            Some(Keyword::FirstStrike),
        ),
        ("vital_splicer", "Vital Splicer", "{3}{G}", None),
    ] {
        let types = if id == "sensor_splicer" {
            &["Creature", "Phyrexian", "Artificer"][..]
        } else {
            &["Creature", "Phyrexian", "Human", "Artificer"][..]
        };
        let f = identity(id, name, mana, types, Some((1, 1)));
        let trigger = &f.triggered_abilities[0];
        assert_eq!(
            trigger.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        assert!(
            trigger.effect.iter().any(|effect| matches!(
                effect,
                SpellEffectKind::CreateTokens { token, .. } if token == "golem_c_3_3"
            )),
            "{id}: creates a Golem"
        );
        if let Some(keyword) = lord {
            assert!(
                matches!(
                    f.static_abilities[0].definition,
                    StaticAbilityDef::AnthemKeyword { keyword: actual, .. } if actual == keyword
                ),
                "{id}: keyword lord"
            );
        } else if id == "master_splicer" {
            assert!(
                matches!(
                    f.static_abilities[0].definition,
                    StaticAbilityDef::AnthemPt {
                        delta_power: 1,
                        delta_toughness: 1,
                        ..
                    }
                ),
                "{id}: +1/+1 lord"
            );
        } else {
            let activated = &f.activated_abilities[0];
            assert_eq!(activated.costs.len(), 1, "{id}");
            assert!(
                activated
                    .effect
                    .iter()
                    .any(|effect| matches!(effect, SpellEffectKind::Regenerate { .. })),
                "{id}: regenerate"
            );
        }
    }
}

// ── 03 artifact value tutors ────────────────────────────────────────────

#[test]
fn q51_artifact_value_tutors_map_bounded_searches() {
    for (id, name, low, high) in [
        ("trinket_mage", "Trinket Mage", 0, 1),
        ("tribute_mage", "Tribute Mage", 2, 2),
        ("trophy_mage", "Trophy Mage", 3, 3),
        ("transit_mage", "Transit Mage", 4, 5),
    ] {
        let f = identity(
            id,
            name,
            "{2}{U}",
            &["Creature", "Human", "Wizard"],
            Some((2, 2)),
        );
        let trigger = &f.triggered_abilities[0];
        assert_eq!(
            trigger.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        let Some(SpellEffectKind::SearchLibrary {
            filter: Some(filter),
            optional,
            reveal,
            shuffle,
            destination,
            ..
        }) = trigger.effect.first()
        else {
            panic!("{id}: search effect");
        };
        assert!(*optional, "{id}: optional");
        assert!(*reveal, "{id}: reveals");
        assert!(*shuffle, "{id}: shuffles");
        assert_eq!(
            *destination,
            tricerules_cards::primitives::SearchDestination::Hand,
            "{id}: hand"
        );
        assert_eq!(filter.card_type, Some(CardTypeFilter::Artifact), "{id}");
        assert_eq!(filter.min_mana_value, Some(low), "{id}");
        assert_eq!(filter.max_mana_value, Some(high), "{id}");
    }
}

// ── 04 subtype harbingers ───────────────────────────────────────────────

#[test]
fn q51_subtype_harbingers_map_top_of_library_searches() {
    for (id, name, mana, types, body, subtypes) in [
        (
            "boggart_harbinger",
            "Boggart Harbinger",
            "{2}{B}",
            &["Creature", "Goblin", "Shaman"][..],
            (2, 1),
            &["Goblin"][..],
        ),
        (
            "kithkin_harbinger",
            "Kithkin Harbinger",
            "{2}{W}",
            &["Creature", "Kithkin", "Wizard"][..],
            (1, 3),
            &["Kithkin"][..],
        ),
        (
            "giant_harbinger",
            "Giant Harbinger",
            "{4}{R}",
            &["Creature", "Giant", "Shaman"][..],
            (3, 4),
            &["Giant"][..],
        ),
        (
            "flamekin_harbinger",
            "Flamekin Harbinger",
            "{R}",
            &["Creature", "Elemental", "Shaman"][..],
            (1, 1),
            &["Elemental"][..],
        ),
        (
            "elvish_harbinger",
            "Elvish Harbinger",
            "{2}{G}",
            &["Creature", "Elf", "Druid"][..],
            (1, 2),
            &["Elf"][..],
        ),
        (
            "treefolk_harbinger",
            "Treefolk Harbinger",
            "{G}",
            &["Creature", "Treefolk", "Druid"][..],
            (0, 3),
            &["Treefolk", "Forest"][..],
        ),
    ] {
        let f = identity(id, name, mana, types, Some(body));
        if id == "elvish_harbinger" {
            let [mana_ability] = f.activated_abilities.as_slice() else {
                panic!("{id}: one mana ability");
            };
            assert_eq!(mana_ability.costs, [AbilityCost::Tap], "{id}: tap cost");
            assert_eq!(
                mana_ability.presentation,
                AbilityPresentation::OracleLines(vec![2]),
                "{id}: presentation"
            );
            assert!(mana_ability.is_mana_ability(), "{id}: mana ability");
            assert!(
                matches!(
                    mana_ability.effect.as_slice(),
                    [SpellEffectKind::ProduceMana { options, .. }]
                        if options.len() == 5
                            && options.iter().all(|amount| {
                                [amount.w, amount.u, amount.b, amount.r, amount.g]
                                    .iter()
                                    .filter(|color| **color == 1)
                                    .count()
                                    == 1
                            })
                ),
                "{id}: five single-color options"
            );
        }
        let trigger = &f.triggered_abilities[0];
        let Some(SpellEffectKind::SearchLibrary {
            filter: Some(filter),
            optional,
            reveal,
            destination,
            ..
        }) = trigger.effect.first()
        else {
            panic!("{id}: search effect");
        };
        assert!(*optional && *reveal, "{id}");
        assert_eq!(
            *destination,
            tricerules_cards::primitives::SearchDestination::TopOfLibrary,
            "{id}"
        );
        let branches = filter
            .any_of
            .as_ref()
            .map(|branches| {
                branches
                    .iter()
                    .map(|branch| branch.required_subtypes.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec![filter.required_subtypes.clone()]);
        assert_eq!(
            branches,
            subtypes
                .iter()
                .map(|subtype| vec![subtype.to_string()])
                .collect::<Vec<_>>(),
            "{id}"
        );
    }
}

// ── 05 Aura restrictions ────────────────────────────────────────────────

#[test]
fn q51_aura_restrictions_map_static_profiles() {
    let cases: &[(&str, &str, &str, fn(&StaticAbilityDef) -> bool)] = &[
        ("cloak_of_mists", "Cloak of Mists", "{1}{U}", |definition| {
            matches!(
                definition,
                StaticAbilityDef::AttachedModifier { restriction, .. } if restriction.cant_be_blocked
            )
        }),
        ("aether_tunnel", "Aether Tunnel", "{1}{U}", |definition| {
            matches!(
                definition,
                StaticAbilityDef::AttachedModifier { delta_power: 1, restriction, .. } if restriction.cant_be_blocked
            )
        }),
        (
            "protective_bubble",
            "Protective Bubble",
            "{3}{U}",
            |definition| {
                matches!(
                    definition,
                    StaticAbilityDef::AttachedModifier { keywords, restriction, .. }
                        if keywords.contains(&Keyword::Shroud) && restriction.cant_be_blocked
                )
            },
        ),
        ("demotion", "Demotion", "{W}", |definition| {
            matches!(
                definition,
                StaticAbilityDef::AttachedModifier { restriction, .. } if restriction.cant_block
            )
        }),
        ("arrest", "Arrest", "{2}{W}", |definition| {
            matches!(
                definition,
                StaticAbilityDef::AttachedModifier { restriction, .. }
                    if restriction.cant_attack && restriction.cant_block
            )
        }),
        (
            "paralyzing_grasp",
            "Paralyzing Grasp",
            "{2}{U}",
            |definition| {
                matches!(
                    definition,
                    StaticAbilityDef::AttachedModifier {
                        doesnt_untap_during_untap_step: true,
                        ..
                    }
                )
            },
        ),
        ("invisibility", "Invisibility", "{U}{U}", |definition| {
            matches!(
                definition,
                StaticAbilityDef::AttachedModifier { restriction, .. }
                    if restriction.cant_be_blocked_by.iter().any(|filter| {
                        filter.kind == TargetKind::Creature && filter.excluded_subtypes == ["Wall"]
                    })
            )
        }),
    ];
    for (id, name, mana, check) in cases {
        let f = identity(id, name, mana, &["Enchantment", "Aura"], None);
        assert!(
            f.static_abilities
                .iter()
                .any(|ability| check(&ability.definition)),
            "{id}"
        );
        if matches!(*id, "demotion" | "arrest") {
            assert!(
                f.static_abilities.iter().any(|ability| matches!(
                    ability.definition,
                    StaticAbilityDef::ProhibitActivatedAbilitiesOfAttachedPermanent
                )),
                "{id}: activation prohibition"
            );
        }
    }
}

// ── 06 enters each player discards ──────────────────────────────────────

#[test]
fn q51_each_player_discard_maps_the_shared_discard() {
    for (id, name, mana, types, body) in [
        (
            "miasmic_mummy",
            "Miasmic Mummy",
            "{1}{B}",
            &["Creature", "Zombie", "Jackal"][..],
            (2, 2),
        ),
        (
            "sibsig_icebreakers",
            "Sibsig Icebreakers",
            "{2}{B}",
            &["Creature", "Zombie"][..],
            (2, 3),
        ),
        (
            "earsplitting_rats",
            "Earsplitting Rats",
            "{3}{B}",
            &["Creature", "Rat"][..],
            (2, 1),
        ),
    ] {
        let f = identity(id, name, mana, types, Some(body));
        let trigger = &f.triggered_abilities[0];
        assert_eq!(
            trigger.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        assert!(
            matches!(
                trigger.effect[0],
                SpellEffectKind::Discard {
                    who: tricerules_cards::primitives::PlayerRecipient::EachPlayer,
                    quantity: tricerules_cards::primitives::DiscardQuantity::Exact(1),
                }
            ),
            "{id}: each player discards one"
        );
    }
    let rats = identity(
        "earsplitting_rats",
        "Earsplitting Rats",
        "{3}{B}",
        &["Creature", "Rat"],
        Some((2, 1)),
    );
    assert_eq!(rats.activated_abilities[0].costs.len(), 1);
}

// ── 08 attack team pumps ────────────────────────────────────────────────

#[test]
fn q51_attack_team_pumps_map_attack_triggers() {
    for (id, name, mana, types, body) in [
        (
            "goblin_general",
            "Goblin General",
            "{1}{R}{R}",
            &["Creature", "Goblin", "Warrior"][..],
            (1, 1),
        ),
        (
            "riot_ringleader",
            "Riot Ringleader",
            "{2}{R}",
            &["Creature", "Human", "Warrior"][..],
            (2, 2),
        ),
        (
            "blood-chin_rager",
            "Blood-Chin Rager",
            "{1}{B}",
            &["Creature", "Human", "Warrior"][..],
            (2, 2),
        ),
        (
            "inspiring_unicorn",
            "Inspiring Unicorn",
            "{2}{W}{W}",
            &["Creature", "Unicorn"][..],
            (2, 2),
        ),
        (
            "chieftain_en-dal",
            "Chieftain en-Dal",
            "{1}{W}{W}",
            &["Creature", "Human", "Knight"][..],
            (2, 2),
        ),
        (
            "fangren_pathcutter",
            "Fangren Pathcutter",
            "{4}{G}{G}",
            &["Creature", "Beast"][..],
            (4, 6),
        ),
    ] {
        let f = identity(id, name, mana, types, Some(body));
        let trigger = &f.triggered_abilities[0];
        assert!(
            matches!(
                trigger.trigger,
                TriggerCondition::WheneverSelfAttacks {
                    minimum_other_attackers: 0
                }
            ),
            "{id}"
        );
        assert!(
            matches!(
                trigger.effect[0],
                SpellEffectKind::PumpAll { .. } | SpellEffectKind::GrantKeywordsAll { .. }
            ),
            "{id}"
        );
    }
}

// ── 09 Spirit or Arcane payoffs ─────────────────────────────────────────

#[test]
fn q51_spirit_arcane_payoffs_map_the_cast_trigger() {
    for (id, name, mana, body) in [
        ("briarknit_kami", "Briarknit Kami", "{3}{G}{G}", (3, 3)),
        ("horizon_seed", "Horizon Seed", "{4}{W}", (2, 1)),
        (
            "guardian_of_solitude",
            "Guardian of Solitude",
            "{1}{U}",
            (1, 2),
        ),
        (
            "kami_of_fires_roar",
            "Kami of Fire's Roar",
            "{3}{R}",
            (2, 3),
        ),
        ("soul_of_magma", "Soul of Magma", "{3}{R}{R}", (2, 2)),
    ] {
        let f = identity(id, name, mana, &["Creature", "Spirit"], Some(body));
        let trigger = &f.triggered_abilities[0];
        let TriggerCondition::WheneverPlayerCastsSpell { filter, .. } = &trigger.trigger else {
            panic!("{id}: cast trigger");
        };
        let branches = filter
            .any_of
            .as_ref()
            .expect("disjunctive Spirit/Arcane filter");
        assert_eq!(
            branches
                .iter()
                .map(|branch| branch.required_subtypes.clone())
                .collect::<Vec<_>>(),
            vec![vec!["Spirit".to_string()], vec!["Arcane".to_string()]],
            "{id}"
        );
        assert_eq!(
            trigger.targeting.as_ref().expect("target").groups[0].min,
            1,
            "{id}"
        );
    }
}

// ── fingerprints ──────────────────────────────────────────────────────────

#[test]
fn q51_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, digest) in [
        (
            "advent_of_the_wurm",
            "Advent of the Wurm",
            "1ab888010ceda3557b4b13854dc05f4410d50df548f7a400aa452facd85badf6",
        ),
        (
            "aether_tunnel",
            "Aether Tunnel",
            "4ed8cae504c80ab4be8a10cba125d45e5608928fd7386c45c23d158c8d7a2a8d",
        ),
        (
            "arrest",
            "Arrest",
            "9193c80a02805cbd5d0f19a146baf55c00ca329c11fdc6150367844187074dce",
        ),
        (
            "blade_splicer",
            "Blade Splicer",
            "d98f95fe085934f53b58d6c1181656efb233bc7b63896ff33515f8763c471b1b",
        ),
        (
            "blood-chin_rager",
            "Blood-Chin Rager",
            "c8204ed43cdfc4c2b6221128da00eca531ec336c107c213f7332932a9cfeae24",
        ),
        (
            "boggart_harbinger",
            "Boggart Harbinger",
            "f132767a695a9d02bfd7e198e8bec0ab16a1466bdf602245437df8cae725f527",
        ),
        (
            "briarknit_kami",
            "Briarknit Kami",
            "554915992507788914f4d7edc55c40ff35b584d30db2ed98f242d9d6e58e0b29",
        ),
        (
            "call_of_the_conclave",
            "Call of the Conclave",
            "9058fc5f43bc6f14879aabb8b51554f36022612fbd723f21916e95dee3dcf980",
        ),
        (
            "chieftain_en-dal",
            "Chieftain en-Dal",
            "c4460af66aa813dfb2f0efee64c96f46642583fb228c4b4d0c01947050a7333f",
        ),
        (
            "cloak_of_mists",
            "Cloak of Mists",
            "6ab91b4c70e79be964cece45ea77f6e0a1c85dc50b7fee23cd46808ecccde988",
        ),
        (
            "demotion",
            "Demotion",
            "3ae4f192b8737442c32af70350b650e82f39abef6dff1c943003684bd3cfd0bf",
        ),
        (
            "earsplitting_rats",
            "Earsplitting Rats",
            "ffa41e3c65c917726988ed9690408df3b712758862add37e01f4c4382b6a984a",
        ),
        (
            "elemental_summoning",
            "Elemental Summoning",
            "968f9f92efd4f8596f2943cd5cfbd20feb72719f14c044f2b32ecb58542be988",
        ),
        (
            "elvish_harbinger",
            "Elvish Harbinger",
            "11a6a3ba83693f2e157015076fff49b8bdc623e73377d3207eac65c08da70355",
        ),
        (
            "fangren_pathcutter",
            "Fangren Pathcutter",
            "562e5adfb525d220cf994c07fcac9ba48f3548dfcc29a2acafece74e86df7a41",
        ),
        (
            "flamekin_harbinger",
            "Flamekin Harbinger",
            "c37949c580ed609095c8a0c459abc1579388716673b1313c91e7ddca553a6ee0",
        ),
        (
            "flurry_of_horns",
            "Flurry of Horns",
            "315ee50014da01ba673efb3af6413fddc7cf6c9082d13988aa2d167c6dca8cce",
        ),
        (
            "giant_harbinger",
            "Giant Harbinger",
            "513a75b991abc92032fc9d30399e0f2cecb42d90b87d263e06dcc0271d7cd2a9",
        ),
        (
            "goblin_general",
            "Goblin General",
            "a3b6a4cfa92226afb1aca212275f555328e1c8385b84e68e193273dc833aff17",
        ),
        (
            "guardian_of_solitude",
            "Guardian of Solitude",
            "90da2946d5994deed54ba7fc22bf9f414d2f598f73ce92234df90dfde1a98d44",
        ),
        (
            "horizon_seed",
            "Horizon Seed",
            "aa273e109f0554332af137f72f296bbaeb0d6c95b5be7cf4bf71288616cdeaf4",
        ),
        (
            "icatian_town",
            "Icatian Town",
            "664e4b5980af2cacfcb5d3b6b899d13d8dcf71ad439b5aaa036889740aff2700",
        ),
        (
            "inspiring_unicorn",
            "Inspiring Unicorn",
            "0bc665f95b447f913e902f18212986527e22aeca154b376fa29b7c7b1c5c180e",
        ),
        (
            "invisibility",
            "Invisibility",
            "4d545a0a7b3ef2776a89d39bdf678089ab1092804d32926ec891f25005c283ae",
        ),
        (
            "kami_of_fires_roar",
            "Kami of Fire's Roar",
            "153d2df6bd21396cffc57ca6e0aa5c9a6a5d65dc5fb88e114a92f3bb3a8fe2b4",
        ),
        (
            "kithkin_harbinger",
            "Kithkin Harbinger",
            "9bdc9861e8499f1df8e30bae43b14bbed606dbe93b420ccd33b86821bbf3706d",
        ),
        (
            "knight_watch",
            "Knight Watch",
            "15ed50ba10044aa0bae5dfac7332411bfb802b8bb1a050db0e0668e384b50eca",
        ),
        (
            "master_splicer",
            "Master Splicer",
            "a9b67c4af833cbafd88c7b2fc06a8471090d40ed04613f6f5396ea26781f24b5",
        ),
        (
            "masters_call",
            "Master's Call",
            "43c6663b97bf7be233f1a948af4a16d69d5d1c8a30c2ea2f6f7dc35da4d7b5fa",
        ),
        (
            "miasmic_mummy",
            "Miasmic Mummy",
            "2fcb3049fe1ba6773ece08951c2b497be40b02849243fa71742cbe0e41203d0c",
        ),
        (
            "paralyzing_grasp",
            "Paralyzing Grasp",
            "95ea171d6c817d1bf47a757a8ae1d2b76bf254fe49a287d38f339317c2282ed2",
        ),
        (
            "protective_bubble",
            "Protective Bubble",
            "d5ef2270412e035dc8b1bd739ea4cb6cff9517e3f28dcd5a0e783a3be995344c",
        ),
        (
            "riot_ringleader",
            "Riot Ringleader",
            "6cc259f61d2894bf0b7903245351b657c6ac09f9cd31876fb189e032b33438de",
        ),
        (
            "sensor_splicer",
            "Sensor Splicer",
            "35d068406edd9b0e9abe9ecaa454a7210092d3435656daa661558730b52142fb",
        ),
        (
            "servo_exhibition",
            "Servo Exhibition",
            "fbcb8879eea0b861317559d2d82cf30d5b8ee5c30c99597c2c04128ded1b5187",
        ),
        (
            "sibsig_icebreakers",
            "Sibsig Icebreakers",
            "2fcb3049fe1ba6773ece08951c2b497be40b02849243fa71742cbe0e41203d0c",
        ),
        (
            "soul_of_magma",
            "Soul of Magma",
            "d23ac6c04d9df77467dc8d7a7302f15280b1553e43265908b27c92e577840053",
        ),
        (
            "spore_swarm",
            "Spore Swarm",
            "3cbd743e48a58ff370cf4447d17d21ec6ab266c6ed4cc16811106f87c46c34a9",
        ),
        (
            "sworn_companions",
            "Sworn Companions",
            "7104685f40d919595f55d2b315de23bb541bba8ab1a32bc8dcbd9633a3b91616",
        ),
        (
            "take_up_arms",
            "Take Up Arms",
            "0c726dcb2a9fc738fd047f4603fb63b12385e3634029ea3a46b4cf0df3719d74",
        ),
        (
            "transit_mage",
            "Transit Mage",
            "82492ca803eba000b9a8c6dca70970bc2855e01daeb7191641a6eb6b61445e96",
        ),
        (
            "treefolk_harbinger",
            "Treefolk Harbinger",
            "98b9c07727e103095394539b5c6794bd6b24916bf0d0d5659cdd7698422c0b90",
        ),
        (
            "tribute_mage",
            "Tribute Mage",
            "e2676f5bb70c75dd474ea16e0ccc4293439303f491d2ef24910509e9704d5ed6",
        ),
        (
            "trinket_mage",
            "Trinket Mage",
            "9c0d785a92081bc3b1f6963c9e3d16705333d37388bfdc291c6cb85142477ab0",
        ),
        (
            "trophy_mage",
            "Trophy Mage",
            "c81c6709d03cd8c3976e3a36737f0e9734bb744793bda692aff64fe8b70edfbf",
        ),
        (
            "vital_splicer",
            "Vital Splicer",
            "04e076c5479141e579521929b4c7e44c4ea46d6d688416cfe6b9e70957f29953",
        ),
        (
            "wing_splicer",
            "Wing Splicer",
            "abc898521103577b3444b6cf43ec4e16c94b958a93a1cda1086b0a4b1ea2d0ea",
        ),
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
        assert_eq!(fields[4], digest, "pinned fingerprint digest for {id}");
        assert_eq!(definition(id).primary_face().name, name, "{id}");
    }
}
