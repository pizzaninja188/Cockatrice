//! Registry conformance for the queue-v71 batch A cohorts.
//!
//! Three stable typed shapes: a printed spell body plus the Storm keyword, a damage spell with a
//! printed flashback cost, and a removal spell with a hand-zone cycling ability. Fingerprints are
//! added after the presentation metadata refresh.

use tricerules_cards::primitives::{
    Amount, EffectSubject, GraveyardDestination, SpellEffectKind, SpellKeyword,
};
use tricerules_cards::{
    registry, AbilityCost, AbilityPresentation, AbilitySourceZone, CardFace, Layout,
};

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

fn assert_target_group(f: &CardFace, min: u32, max: u32, effect_indices: &[u32], id: &str) {
    let targeting = f
        .targeting
        .as_ref()
        .unwrap_or_else(|| panic!("{id}: targeting"));
    let group = targeting
        .groups
        .first()
        .unwrap_or_else(|| panic!("{id}: target group"));
    assert_eq!((group.min, group.max), (min, max), "{id}: target bound");
    assert_eq!(group.effect_indices, effect_indices, "{id}: group effects");
}

fn assert_storm(f: &CardFace, id: &str) {
    assert_eq!(f.spell_keywords, vec![SpellKeyword::Storm], "{id}: Storm");
}

fn assert_cycling(f: &CardFace, cost: &str, id: &str) {
    let ability = f
        .activated_abilities
        .first()
        .unwrap_or_else(|| panic!("{id}: missing cycling ability"));
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2]),
        "{id}: cycling presentation"
    );
    assert_eq!(ability.source_zone, AbilitySourceZone::Hand, "{id}");
    assert!(
        matches!(
            ability.costs.as_slice(),
            [AbilityCost::Mana(mana), AbilityCost::DiscardSelf]
                if mana.to_string() == cost
        ),
        "{id}: cycling cost {cost}"
    );
    assert!(
        matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::Draw {
                count: Amount::Fixed(1),
                ..
            }]
        ),
        "{id}: cycling draws one"
    );
}

#[test]
fn q71_batch_a_storm_spells_map_definitions() {
    for (id, name, mana, types) in [
        ("astral_steel", "Astral Steel", "{2}{W}", &["Instant"][..]),
        (
            "empty_the_warrens",
            "Empty the Warrens",
            "{3}{R}",
            &["Sorcery"][..],
        ),
        ("grapeshot", "Grapeshot", "{1}{R}", &["Sorcery"][..]),
        (
            "reaping_the_graves",
            "Reaping the Graves",
            "{2}{B}",
            &["Instant"][..],
        ),
        ("scattershot", "Scattershot", "{2}{R}", &["Instant"][..]),
        (
            "temporal_fissure",
            "Temporal Fissure",
            "{4}{U}",
            &["Sorcery"][..],
        ),
        (
            "tendrils_of_agony",
            "Tendrils of Agony",
            "{2}{B}{B}",
            &["Sorcery"][..],
        ),
        (
            "volcanic_awakening",
            "Volcanic Awakening",
            "{4}{R}{R}",
            &["Sorcery"][..],
        ),
        (
            "weather_the_storm",
            "Weather the Storm",
            "{1}{G}",
            &["Instant"][..],
        ),
    ] {
        let f = face(id, name, mana, types);
        assert_storm(f, id);
        assert!(f.flashback_cost.is_none(), "{id}: no flashback");
        assert!(
            f.activated_abilities.is_empty(),
            "{id}: no activated ability"
        );
    }

    assert!(
        matches!(
            face("astral_steel", "Astral Steel", "{2}{W}", &["Instant"])
                .spell_effect
                .as_slice(),
            [SpellEffectKind::PumpTarget {
                power: 1,
                toughness: 2,
                ..
            }]
        ),
        "Astral Steel pump"
    );
    assert!(
        matches!(
            face("empty_the_warrens", "Empty the Warrens", "{3}{R}", &["Sorcery"])
                .spell_effect
                .as_slice(),
            [SpellEffectKind::CreateTokens { token, count, .. }]
                if token == "goblin_r_1_1" && *count == Amount::Fixed(2)
        ),
        "Empty the Warrens tokens"
    );
    assert!(
        matches!(
            face("grapeshot", "Grapeshot", "{1}{R}", &["Sorcery"])
                .spell_effect
                .as_slice(),
            [SpellEffectKind::DamageTarget {
                amount: Amount::Fixed(1),
                ..
            }]
        ),
        "Grapeshot damage"
    );
    assert!(
        matches!(
            face(
                "reaping_the_graves",
                "Reaping the Graves",
                "{2}{B}",
                &["Instant"]
            )
            .spell_effect
            .as_slice(),
            [SpellEffectKind::MoveGraveyardCards {
                destination: GraveyardDestination::Hand,
                ..
            }]
        ),
        "Reaping the Graves return"
    );
    assert!(
        matches!(
            face("scattershot", "Scattershot", "{2}{R}", &["Instant"])
                .spell_effect
                .as_slice(),
            [SpellEffectKind::DamageTarget {
                amount: Amount::Fixed(1),
                ..
            }]
        ),
        "Scattershot damage"
    );
    assert!(
        matches!(
            face(
                "temporal_fissure",
                "Temporal Fissure",
                "{4}{U}",
                &["Sorcery"]
            )
            .spell_effect
            .as_slice(),
            [SpellEffectKind::ReturnToOwnersHand {
                subject: EffectSubject::Chosen(_)
            }]
        ),
        "Temporal Fissure return"
    );
    let tendrils = face(
        "tendrils_of_agony",
        "Tendrils of Agony",
        "{2}{B}{B}",
        &["Sorcery"],
    );
    assert_target_group(tendrils, 1, 1, &[0], "tendrils_of_agony");
    assert!(
        matches!(
            tendrils.spell_effect.as_slice(),
            [
                SpellEffectKind::TargetPlayerLosesLife { amount: 2, .. },
                SpellEffectKind::GainLife {
                    amount: Amount::Fixed(2),
                    ..
                }
            ]
        ),
        "Tendrils of Agony drain"
    );
    assert!(
        matches!(
            face(
                "volcanic_awakening",
                "Volcanic Awakening",
                "{4}{R}{R}",
                &["Sorcery"]
            )
            .spell_effect
            .as_slice(),
            [SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(_)
            }]
        ),
        "Volcanic Awakening destroy"
    );
    assert!(
        matches!(
            face(
                "weather_the_storm",
                "Weather the Storm",
                "{1}{G}",
                &["Instant"]
            )
            .spell_effect
            .as_slice(),
            [SpellEffectKind::GainLife {
                amount: Amount::Fixed(3),
                ..
            }]
        ),
        "Weather the Storm life"
    );
}

#[test]
fn q71_batch_a_flashback_damage_map_definitions() {
    for (id, name, mana, types, flashback) in [
        (
            "morbid_hunger",
            "Morbid Hunger",
            "{4}{B}{B}",
            &["Sorcery"][..],
            "{7}{B}{B}",
        ),
        (
            "sacred_fire",
            "Sacred Fire",
            "{R}{W}",
            &["Instant"][..],
            "{4}{R}{W}",
        ),
        (
            "scorching_missile",
            "Scorching Missile",
            "{3}{R}",
            &["Sorcery"][..],
            "{9}{R}",
        ),
        (
            "smiting_helix",
            "Smiting Helix",
            "{3}{B}",
            &["Sorcery"][..],
            "{R}{W}",
        ),
    ] {
        let f = face(id, name, mana, types);
        assert_eq!(
            f.flashback_cost
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            Some(flashback),
            "{id}: flashback cost"
        );
        assert!(f.spell_keywords.is_empty(), "{id}: no spell keyword");
        assert!(
            f.activated_abilities.is_empty(),
            "{id}: no activated ability"
        );
        assert_target_group(f, 1, 1, &[0], id);
    }

    for id in ["morbid_hunger", "sacred_fire", "smiting_helix"] {
        let f = face(
            id,
            match id {
                "morbid_hunger" => "Morbid Hunger",
                "sacred_fire" => "Sacred Fire",
                _ => "Smiting Helix",
            },
            match id {
                "morbid_hunger" => "{4}{B}{B}",
                "sacred_fire" => "{R}{W}",
                _ => "{3}{B}",
            },
            if id == "sacred_fire" {
                &["Instant"][..]
            } else {
                &["Sorcery"][..]
            },
        );
        assert!(
            matches!(
                f.spell_effect.as_slice(),
                [
                    SpellEffectKind::DamageTarget { .. },
                    SpellEffectKind::GainLife { amount, .. }
                ] if match id {
                    "morbid_hunger" => *amount == Amount::Fixed(3),
                    "sacred_fire" => *amount == Amount::Fixed(2),
                    _ => *amount == Amount::Fixed(3),
                }
            ),
            "{id}: damage plus equal life"
        );
    }

    let missile = face(
        "scorching_missile",
        "Scorching Missile",
        "{3}{R}",
        &["Sorcery"],
    );
    assert!(
        matches!(
            missile.spell_effect.as_slice(),
            [SpellEffectKind::DamageTarget {
                amount: Amount::Fixed(4),
                ..
            }]
        ),
        "Scorching Missile damage"
    );
    let debug = format!("{:?}", missile.spell_effect);
    assert!(debug.contains("AnyPlayer"), "{debug}");
    assert!(debug.contains("Planeswalker"), "{debug}");
}

#[test]
fn q71_batch_a_cycling_removal_map_definitions() {
    for (id, name, mana, types) in [
        (
            "forsake_the_worldly",
            "Forsake the Worldly",
            "{2}{W}",
            &["Instant"][..],
        ),
        ("hush", "Hush", "{3}{G}", &["Sorcery"][..]),
        ("scrap", "Scrap", "{2}{R}", &["Instant"][..]),
        (
            "volcanic_submersion",
            "Volcanic Submersion",
            "{4}{R}",
            &["Sorcery"][..],
        ),
        ("wipe_clean", "Wipe Clean", "{1}{W}", &["Instant"][..]),
    ] {
        let f = face(id, name, mana, types);
        assert!(f.spell_keywords.is_empty(), "{id}: no spell keyword");
        assert_cycling(f, if id == "wipe_clean" { "{3}" } else { "{2}" }, id);
    }

    let forsake = face(
        "forsake_the_worldly",
        "Forsake the Worldly",
        "{2}{W}",
        &["Instant"],
    );
    assert_target_group(forsake, 1, 1, &[0], "forsake_the_worldly");
    assert!(
        matches!(
            forsake.spell_effect.as_slice(),
            [SpellEffectKind::Exile {
                subject: EffectSubject::Chosen(_),
                ..
            }]
        ),
        "Forsake the Worldly exile"
    );

    let hush = face("hush", "Hush", "{3}{G}", &["Sorcery"]);
    assert!(hush.targeting.is_none(), "Hush: untargeted");
    assert!(
        matches!(
            hush.spell_effect.as_slice(),
            [SpellEffectKind::DestroyAll { .. }]
        ),
        "Hush mass destroy"
    );

    let scrap = face("scrap", "Scrap", "{2}{R}", &["Instant"]);
    assert_target_group(scrap, 1, 1, &[0], "scrap");
    assert!(
        matches!(
            scrap.spell_effect.as_slice(),
            [SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(_)
            }]
        ),
        "Scrap destroy"
    );

    let submersion = face(
        "volcanic_submersion",
        "Volcanic Submersion",
        "{4}{R}",
        &["Sorcery"],
    );
    assert_target_group(submersion, 1, 1, &[0], "volcanic_submersion");
    assert!(
        matches!(
            submersion.spell_effect.as_slice(),
            [SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(_)
            }]
        ),
        "Volcanic Submersion destroy"
    );
    let debug = format!("{:?}", submersion.spell_effect);
    assert!(debug.contains("Artifact"), "{debug}");
    assert!(debug.contains("Land"), "{debug}");

    let wipe = face("wipe_clean", "Wipe Clean", "{1}{W}", &["Instant"]);
    assert_target_group(wipe, 1, 1, &[0], "wipe_clean");
    assert!(
        matches!(
            wipe.spell_effect.as_slice(),
            [SpellEffectKind::Exile {
                subject: EffectSubject::Chosen(_),
                ..
            }]
        ),
        "Wipe Clean exile"
    );
}

#[test]
fn q71_batch_a_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "astral_steel",
            "Astral Steel",
            "4ecd6d09beeb9711754cf7f13419cbf3f5c3f462a58afe85527d09295a1f2e79",
        ),
        (
            "empty_the_warrens",
            "Empty the Warrens",
            "c83b9bd4f2aaa2ea92311caceca276466ddd7dcc07cf7cc254b1e2c46e2df848",
        ),
        (
            "grapeshot",
            "Grapeshot",
            "9b69ef67e2725503270ec8735b240939f150a9a378ccb33928ba251906bbf18e",
        ),
        (
            "reaping_the_graves",
            "Reaping the Graves",
            "db6d19fb3c4dc71839fdb57f2b54fc0f8f0649de8778cc0aa9b2046c885370e0",
        ),
        (
            "scattershot",
            "Scattershot",
            "0ce368827341d34e8cf752812814846671a97e7598399f0e3335d1c6605773cf",
        ),
        (
            "temporal_fissure",
            "Temporal Fissure",
            "3c4eba12ca3f5b1fc8ea71f429a14db35054e204c21f2a3f33eb04892e597f01",
        ),
        (
            "tendrils_of_agony",
            "Tendrils of Agony",
            "e5f20b4e33b3ed0c587911b86a5fb8e696ad97f3d667344a25a49cfdc5c550f0",
        ),
        (
            "volcanic_awakening",
            "Volcanic Awakening",
            "bb1033d6a038e245b0034ab081678a69228acc6566b8d1d34157d42f90889c3b",
        ),
        (
            "weather_the_storm",
            "Weather the Storm",
            "e71e33d70dcdb6897806fbf6c5dcdec97bd36855bd7e10d5cfa42d7612729d64",
        ),
        (
            "morbid_hunger",
            "Morbid Hunger",
            "f3f21cfeaed1bcaf9b2e373643db29ab8b9e02d2961ea1738c620f91133a73d0",
        ),
        (
            "sacred_fire",
            "Sacred Fire",
            "bb9228bb673e173bd57976146c2c50587478c1f78b8274889c3d86f38b55df8f",
        ),
        (
            "scorching_missile",
            "Scorching Missile",
            "c297eaba92069050251182c8d816dc34b1cf5e288c40e037eff11907f7ad58dd",
        ),
        (
            "smiting_helix",
            "Smiting Helix",
            "5332da4473a01054051565938bdfed213d0ea9624654b9b128930276b50e16ad",
        ),
        (
            "forsake_the_worldly",
            "Forsake the Worldly",
            "c2be60a6a59717aa6ef6199940294a065cded2590a3ca0a1cf19764bc20413b1",
        ),
        (
            "hush",
            "Hush",
            "cf8226e5a2047f9b5105d2b3bfc0d3eb7d670e7b90074b00711b4fd62e60ccbf",
        ),
        (
            "scrap",
            "Scrap",
            "198b43ab6d00ca0746f721b772574aff0ea9ab88fae28a6afa79207593a77618",
        ),
        (
            "volcanic_submersion",
            "Volcanic Submersion",
            "7e506dd5f4c8a17bec520727d79f45d34f653e43cbcbd8fddd2e87cf65dab9e0",
        ),
        (
            "wipe_clean",
            "Wipe Clean",
            "36f35af2979780cb25ac89821d1b00e6e9d8381486600528ec73dbc16e34f7fa",
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
        assert_eq!(fields[2], id, "face id drift for {id}");
        assert_eq!(fields[3], name, "display name drift for {id}");
        assert_eq!(fields[4], fingerprint, "pinned Oracle fingerprint for {id}");
    }
}
