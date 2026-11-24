//! Registry conformance for the queue-v75 01-treasure-riders cohort.
//!
//! One spell body plus the printed Treasure rider. Fingerprints are checked after the presentation
//! metadata refresh.

use tricerules_cards::primitives::{
    Amount, EffectSubject, PermanentTypeFilter, SpellEffectKind, TargetKind,
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

fn assert_treasure_rider(f: &CardFace, id: &str) {
    let last = f.spell_effect.last().expect("a Treasure rider");
    assert!(
        matches!(
            last,
            SpellEffectKind::CreateTokens {
                token,
                count: Amount::Fixed(1),
                ..
            } if token == "treasure"
        ),
        "{id}: the last instruction creates exactly one Treasure"
    );
}

#[test]
fn q75_treasure_riders_map_definitions() {
    let deadly = face(
        "deadly_derision",
        "Deadly Derision",
        "{2}{B}{B}",
        &["Instant"],
    );
    assert!(
        matches!(
            deadly.spell_effect.first(),
            Some(SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(filter),
            }) if filter.kind == TargetKind::AnyPermanent
                && filter.permanent_types == vec![PermanentTypeFilter::Creature, PermanentTypeFilter::Planeswalker]
        ),
        "Deadly Derision destroys a creature or planeswalker"
    );
    assert_treasure_rider(deadly, "deadly_derision");

    let grim = face("grim_bounty", "Grim Bounty", "{2}{B}{B}", &["Sorcery"]);
    assert_eq!(
        format!("{:?}", grim.spell_effect),
        format!("{:?}", deadly.spell_effect),
        "Grim Bounty has the same typed body as Deadly Derision"
    );
    assert_treasure_rider(grim, "grim_bounty");

    let weaponry = face(
        "improvised_weaponry",
        "Improvised Weaponry",
        "{2}{R}",
        &["Sorcery"],
    );
    assert!(
        matches!(
            weaponry.spell_effect.first(),
            Some(SpellEffectKind::DamageTarget {
                amount: Amount::Fixed(2),
                target,
            }) if target.kind == TargetKind::AnyTarget
        ),
        "Improvised Weaponry deals two damage to any target"
    );
    assert_treasure_rider(weaponry, "improvised_weaponry");

    let prize = face("pirates_prize", "Pirate's Prize", "{3}{U}", &["Sorcery"]);
    assert!(
        matches!(
            prize.spell_effect.first(),
            Some(SpellEffectKind::Draw {
                count: Amount::Fixed(2),
                ..
            })
        ),
        "Pirate's Prize draws two"
    );
    assert_treasure_rider(prize, "pirates_prize");

    let hornswoggle = face("hornswoggle", "Hornswoggle", "{2}{U}", &["Instant"]);
    assert!(
        matches!(
            hornswoggle.spell_effect.first(),
            Some(SpellEffectKind::CounterTargetSpell { spell_filter, .. })
                if spell_filter.card_type
                    == Some(tricerules_cards::primitives::CardTypeFilter::Creature)
        ),
        "Hornswoggle counters a creature spell"
    );
    assert_treasure_rider(hornswoggle, "hornswoggle");

    for f in [deadly, grim, weaponry, prize, hornswoggle] {
        assert!(f.spell_keywords.is_empty(), "no spell keyword");
        assert!(f.flashback_cost.is_none(), "no flashback");
        assert!(f.static_abilities.is_empty(), "no static ability");
        assert!(f.activated_abilities.is_empty(), "no activated ability");
    }
}

#[test]
fn q75_treasure_riders_target_groups_match_their_bodies() {
    for (id, expected_targets) in [
        ("deadly_derision", true),
        ("grim_bounty", true),
        ("improvised_weaponry", true),
        ("hornswoggle", false),
    ] {
        let f = face(
            id,
            match id {
                "deadly_derision" => "Deadly Derision",
                "grim_bounty" => "Grim Bounty",
                "improvised_weaponry" => "Improvised Weaponry",
                _ => "Hornswoggle",
            },
            match id {
                "deadly_derision" | "grim_bounty" => "{2}{B}{B}",
                "improvised_weaponry" => "{2}{R}",
                _ => "{2}{U}",
            },
            match id {
                "deadly_derision" | "hornswoggle" => &["Instant"][..],
                "grim_bounty" => &["Sorcery"][..],
                _ => &["Sorcery"][..],
            },
        );
        let targeting = f.targeting.as_ref();
        if expected_targets {
            let group = targeting
                .and_then(|t| t.groups.first())
                .unwrap_or_else(|| panic!("{id}: one target group"));
            assert_eq!((group.min, group.max), (1, 1), "{id}");
            assert_eq!(group.effect_indices, [0], "{id}");
        } else {
            assert!(
                targeting.is_none(),
                "{id}: the counter owns its stack target; no extra group"
            );
        }
    }
    let prize = face("pirates_prize", "Pirate's Prize", "{3}{U}", &["Sorcery"]);
    assert!(prize.targeting.is_none(), "Pirate's Prize is untargeted");
}

#[test]
fn q75_treasure_riders_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, face_id, fingerprint) in [
        (
            "deadly_derision",
            "Deadly Derision",
            "deadly_derision",
            "23ee79dffb1cdf526957299f669f36efa8b0aa8b87b62402290a648f3f7cbe57",
        ),
        (
            "grim_bounty",
            "Grim Bounty",
            "grim_bounty",
            "23ee79dffb1cdf526957299f669f36efa8b0aa8b87b62402290a648f3f7cbe57",
        ),
        (
            "improvised_weaponry",
            "Improvised Weaponry",
            "improvised_weaponry",
            "8e9365e369339a65f7359280e68ac2794ca1677c7db5537268482b027e4d6f39",
        ),
        (
            "pirates_prize",
            "Pirate's Prize",
            "pirates_prize",
            "66859a43405e9f4ddc1a92a2e33c9318cbe263fdd30d254695adbf492870d756",
        ),
        (
            "hornswoggle",
            "Hornswoggle",
            "hornswoggle",
            "7bdab68cd29df88e413d61628299892351a27612877728a8c709aa85addd4377",
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
