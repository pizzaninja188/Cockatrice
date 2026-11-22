//! Registry conformance for the queue-v49 reviewed cohorts (batch A).
#![allow(clippy::type_complexity)]
use tricerules_cards::primitives::{
    Color, Keyword, ProtectionQuality, SpellEffectKind, StaticAbilityDef, TargetKind,
};
use tricerules_cards::{registry, AbilityPresentation, CardDefinition, CardFace, Layout};

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

// ── 01 protection from white ───────────────────────────────────────────────

#[test]
fn q49_protection_white_maps_definitions() {
    struct Case {
        id: &'static str,
        name: &'static str,
        mana: &'static str,
        types: &'static [&'static str],
        body: (u32, u32),
        keywords: &'static [Keyword],
    }
    for case in [
        Case {
            id: "blood_knight",
            name: "Blood Knight",
            mana: "{R}{R}",
            types: &["Creature", "Human", "Knight"],
            body: (2, 2),
            keywords: &[Keyword::FirstStrike],
        },
        Case {
            id: "defender_of_chaos",
            name: "Defender of Chaos",
            mana: "{2}{R}",
            types: &["Creature", "Human", "Knight"],
            body: (2, 1),
            keywords: &[Keyword::Flash],
        },
        Case {
            id: "voice_of_truth",
            name: "Voice of Truth",
            mana: "{3}{W}",
            types: &["Creature", "Angel"],
            body: (2, 2),
            keywords: &[Keyword::Flying],
        },
    ] {
        let f = identity(case.id, case.name, case.mana, case.types, Some(case.body));
        assert_eq!(f.keywords, case.keywords, "{}", case.id);
        assert_eq!(
            f.protections,
            [ProtectionQuality::Color(Color::White)],
            "{}",
            case.id
        );
        assert!(f.static_abilities.is_empty(), "{}", case.id);
        assert!(f.triggered_abilities.is_empty(), "{}", case.id);
        assert!(f.activated_abilities.is_empty(), "{}", case.id);
    }
}

// ── 02 Aura keyword grants ─────────────────────────────────────────────────

#[test]
fn q49_aura_keyword_maps_definitions() {
    struct Case {
        id: &'static str,
        name: &'static str,
        mana: &'static str,
        keyword: Keyword,
    }
    for case in [
        Case {
            id: "imposing_visage",
            name: "Imposing Visage",
            mana: "{R}",
            keyword: Keyword::Menace,
        },
        Case {
            id: "lance",
            name: "Lance",
            mana: "{W}",
            keyword: Keyword::FirstStrike,
        },
        Case {
            id: "lifelink",
            name: "Lifelink",
            mana: "{W}",
            keyword: Keyword::Lifelink,
        },
        Case {
            id: "primal_frenzy",
            name: "Primal Frenzy",
            mana: "{G}",
            keyword: Keyword::Trample,
        },
        Case {
            id: "vigilance",
            name: "Vigilance",
            mana: "{W}",
            keyword: Keyword::Vigilance,
        },
    ] {
        let f = identity(
            case.id,
            case.name,
            case.mana,
            &["Enchantment", "Aura"],
            None,
        );
        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{}: one static ability", case.id);
        };
        match f.spell_effect.as_slice() {
            [SpellEffectKind::AuraAttach { target }] => {
                assert_eq!(target.kind, TargetKind::Creature, "{}", case.id);
            }
            other => panic!("{}: expected AuraAttach, got {other:?}", case.id),
        };
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{}",
            case.id
        );
        let StaticAbilityDef::AttachedModifier {
            delta_power,
            delta_toughness,
            keywords,
            ..
        } = &static_ability.definition
        else {
            panic!("{}: AttachedModifier", case.id);
        };
        assert_eq!((*delta_power, *delta_toughness), (0, 0), "{}", case.id);
        assert_eq!(keywords, &[case.keyword], "{}", case.id);
    }
}

// ── 03 Aura pump plus keyword ──────────────────────────────────────────────

#[test]
fn q49_aura_pump_keyword_maps_definitions() {
    struct Case {
        id: &'static str,
        name: &'static str,
        mana: &'static str,
        delta: (i32, i32),
        keyword: Keyword,
    }
    for case in [
        Case {
            id: "aspect_of_gorgon",
            name: "Aspect of Gorgon",
            mana: "{2}{B}",
            delta: (1, 3),
            keyword: Keyword::Deathtouch,
        },
        Case {
            id: "lightning_talons",
            name: "Lightning Talons",
            mana: "{2}{R}",
            delta: (3, 0),
            keyword: Keyword::FirstStrike,
        },
        Case {
            id: "mantle_of_webs",
            name: "Mantle of Webs",
            mana: "{1}{G}",
            delta: (1, 3),
            keyword: Keyword::Reach,
        },
        Case {
            id: "marked_by_honor",
            name: "Marked by Honor",
            mana: "{3}{W}",
            delta: (2, 2),
            keyword: Keyword::Vigilance,
        },
        Case {
            id: "web",
            name: "Web",
            mana: "{G}",
            delta: (0, 2),
            keyword: Keyword::Reach,
        },
    ] {
        let f = identity(
            case.id,
            case.name,
            case.mana,
            &["Enchantment", "Aura"],
            None,
        );
        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{}: one static ability", case.id);
        };
        match f.spell_effect.as_slice() {
            [SpellEffectKind::AuraAttach { target }] => {
                assert_eq!(target.kind, TargetKind::Creature, "{}", case.id);
            }
            other => panic!("{}: expected AuraAttach, got {other:?}", case.id),
        };
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{}",
            case.id
        );
        let StaticAbilityDef::AttachedModifier {
            delta_power,
            delta_toughness,
            keywords,
            ..
        } = &static_ability.definition
        else {
            panic!("{}: AttachedModifier", case.id);
        };
        assert_eq!((*delta_power, *delta_toughness), case.delta, "{}", case.id);
        assert_eq!(keywords, &[case.keyword], "{}", case.id);
    }
}

#[test]
fn q49_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, digest) in [
        (
            "blood_knight",
            "Blood Knight",
            "060233c2adc5ff240a76348d381dd41c57aee0c559185a89c03c6e22eb00bd5e",
        ),
        (
            "defender_of_chaos",
            "Defender of Chaos",
            "d2fc5e2bcec8f6688deb5634e3a7c616f2bf8d09c01ec28f7efaddfe4e5cffad",
        ),
        (
            "voice_of_truth",
            "Voice of Truth",
            "fc5fa1cf0f073ae9ad5548712c7bd24a1b6692a8d77b52a23cdd64e98b252b96",
        ),
        (
            "imposing_visage",
            "Imposing Visage",
            "b91b4eb79391ec4201dcb7486072d795685e873dc37d8de6dbbd4e1cc48a83c8",
        ),
        (
            "lance",
            "Lance",
            "997f64294a08f0d8be452a202cb4106ed2e55aec727a877de0ebdee516d82508",
        ),
        (
            "lifelink",
            "Lifelink",
            "65abb58bd93e1c0f9a03995912f6f6413d0338ea796dc6622dfacb7af32ab262",
        ),
        (
            "primal_frenzy",
            "Primal Frenzy",
            "d14e0f8ecd6a8e1538828a894dd71a351e58b8fe7b6680fb8eacf821ef93b821",
        ),
        (
            "vigilance",
            "Vigilance",
            "8f0012dcd44b9ce486328c04c4ea5e5f531d308bc1ad4327802811a697021057",
        ),
        (
            "aspect_of_gorgon",
            "Aspect of Gorgon",
            "4105729a18f6a0aad0f1896677e579e559b5a46abd508eb5597299e3a37ecfa2",
        ),
        (
            "lightning_talons",
            "Lightning Talons",
            "119a23c9f58d779966775ed0bcd4d4f571db10a3341db45f7901c079954675fe",
        ),
        (
            "mantle_of_webs",
            "Mantle of Webs",
            "b788178a0c9b2e6989cfda55c0c2481da6944492ada970dde5c80b7387bc9f6d",
        ),
        (
            "marked_by_honor",
            "Marked by Honor",
            "16db3a760d462fc747d6ebb9918ae058c91ac071823106d54f7658b66a86d392",
        ),
        (
            "web",
            "Web",
            "438052811094dae3cc55829fda047c3e1ac7c869e6e55ac8845a3de073e1d42f",
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
