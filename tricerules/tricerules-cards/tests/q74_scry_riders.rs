//! Registry conformance for the queue-v74 03-scry-riders cohort.
//!
//! One targeted or untargeted spell body plus `Scry`. Fingerprints are checked after the
//! presentation metadata refresh.

use tricerules_cards::primitives::{
    Amount, CombatRole, EffectSubject, Keyword, SpellEffectKind, TargetKind,
};
use tricerules_cards::{registry, CardFace, Layout};

fn face(id: &str, name: &str, mana: &str, types: &[&str]) -> &'static CardFace {
    let definition = registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"));
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    assert_eq!(registry::global().id_for_name(name), Some(id), "{id}");
    let f = definition.primary_face();
    assert_eq!(f.name, name, "{id} name");
    assert_eq!(f.mana_cost.to_string(), mana, "{id} mana");
    assert_eq!(
        f.types.iter().map(String::as_str).collect::<Vec<_>>(),
        types,
        "{id} types"
    );
    f
}

#[test]
fn q74_scry_riders_map_definitions() {
    for (id, name, mana, types) in [
        (
            "anchor_to_the_aether",
            "Anchor to the Aether",
            "{2}{U}",
            &["Sorcery"][..],
        ),
        (
            "jayas_greeting",
            "Jaya's Greeting",
            "{1}{R}",
            &["Instant"][..],
        ),
        ("piercing_light", "Piercing Light", "{W}", &["Instant"][..]),
        ("samuts_sprint", "Samut's Sprint", "{R}", &["Instant"][..]),
        ("serum_visions", "Serum Visions", "{U}", &["Sorcery"][..]),
    ] {
        let f = face(id, name, mana, types);
        assert!(f.spell_keywords.is_empty(), "{id}: no spell keyword");
        assert!(f.flashback_cost.is_none(), "{id}: no flashback");
        assert!(
            f.activated_abilities.is_empty(),
            "{id}: no activated ability"
        );
        assert!(f.static_abilities.is_empty(), "{id}: no static ability");
    }

    let anchor = face(
        "anchor_to_the_aether",
        "Anchor to the Aether",
        "{2}{U}",
        &["Sorcery"],
    );
    assert!(anchor.spell_effect.is_empty(), "Anchor is a Program");
    let program = anchor.spell_program.as_ref().expect("Anchor Program");
    assert_eq!(program.body.steps.len(), 2);
    let debug = format!("{program:?}");
    assert!(debug.contains("PutInOwnersLibrary"), "Anchor puts on top");
    assert!(debug.contains("Top"), "Anchor uses the top placement");
    assert!(debug.contains("Scry"), "Anchor scries");

    let jaya = face("jayas_greeting", "Jaya's Greeting", "{1}{R}", &["Instant"]);
    assert!(
        matches!(
            jaya.spell_effect.as_slice(),
            [
                SpellEffectKind::DamageTarget {
                    amount: Amount::Fixed(3),
                    target,
                },
                SpellEffectKind::Scry {
                    count: Amount::Fixed(1),
                }
            ] if target.kind == TargetKind::Creature
        ),
        "Jaya's Greeting damages a creature for three, then scries one"
    );

    let piercing = face("piercing_light", "Piercing Light", "{W}", &["Instant"]);
    assert!(
        matches!(
            piercing.spell_effect.as_slice(),
            [
                SpellEffectKind::DamageTarget {
                    amount: Amount::Fixed(2),
                    target,
                },
                SpellEffectKind::Scry {
                    count: Amount::Fixed(1),
                }
            ] if target.kind == TargetKind::Creature
                && target.combat_role == Some(CombatRole::AttackingOrBlocking)
        ),
        "Piercing Light damages an attacking or blocking creature for two, then scries one"
    );

    let samut = face("samuts_sprint", "Samut's Sprint", "{R}", &["Instant"]);
    assert!(
        matches!(
            samut.spell_effect.as_slice(),
            [
                SpellEffectKind::PumpTarget {
                    power: 2,
                    toughness: 1,
                    subject: EffectSubject::Chosen(_),
                    ..
                },
                SpellEffectKind::GrantKeywords {
                    subject: EffectSubject::Chosen(_),
                    keywords,
                },
                SpellEffectKind::Scry {
                    count: Amount::Fixed(1),
                }
            ] if keywords == &[Keyword::Haste]
        ),
        "Samut's Sprint pumps, grants haste and scries one"
    );
    let group = samut
        .targeting
        .as_ref()
        .expect("Samut targeting")
        .groups
        .first()
        .expect("Samut group");
    assert_eq!(group.effect_indices, [0, 1], "both grants share the target");

    let serum = face("serum_visions", "Serum Visions", "{U}", &["Sorcery"]);
    assert!(
        matches!(
            serum.spell_effect.as_slice(),
            [
                SpellEffectKind::Draw {
                    count: Amount::Fixed(1),
                    ..
                },
                SpellEffectKind::Scry {
                    count: Amount::Fixed(2),
                }
            ]
        ),
        "Serum Visions draws one, then scries two"
    );
}

#[test]
fn q74_scry_riders_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, face_id, fingerprint) in [
        (
            "anchor_to_the_aether",
            "Anchor to the Aether",
            "anchor_to_the_aether",
            "1020074a0f3b5d24f7339a050f9acdb74e820425fbbee1774f9c853fc27da3ea",
        ),
        (
            "jayas_greeting",
            "Jaya's Greeting",
            "jayas_greeting",
            "41afa965c52a9c89032b2cc11bdbdb998d2cf6d62526b8a4550e1493ac40c0d0",
        ),
        (
            "piercing_light",
            "Piercing Light",
            "piercing_light",
            "34639e19197062420f6265108ac94f659a0073fae528cbf8477946e76773a08a",
        ),
        (
            "samuts_sprint",
            "Samut's Sprint",
            "samuts_sprint",
            "25c42d048df05e85baa1815fa22208870ed7a85c5f9bb664fa635a22b611096f",
        ),
        (
            "serum_visions",
            "Serum Visions",
            "serum_visions",
            "640070591a43fd74e1ec681e68fef868e8dc9912002d4516056ca2076aae79eb",
        ),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], face_id, "face id drift for {id}");
        assert_eq!(fields[3], name, "display name drift for {id}");
        assert_eq!(fields[4], fingerprint, "pinned Oracle fingerprint for {id}");
    }
}
