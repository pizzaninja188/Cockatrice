//! Registry conformance for the reviewed Flying plus enters-scry family.
//!
//! Every included card is a normal-layout creature with Flying and an enters
//! trigger that scries its printed count. Card names, costs, types, bodies and
//! scry counts are printed data; the shared scenario executes every card. Each
//! fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{Amount, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Keyword, Layout, TriggerCondition};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    (u32, u32),
    u32,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "horizon_scholar",
        "Horizon Scholar",
        "{5}{U}",
        &["Creature", "Sphinx"],
        (4, 4),
        2,
        "3014024ec11789a2cc181ff20a9e7131a6804d42e3759a75614e7225556755af",
    ),
    (
        "augury_owl",
        "Augury Owl",
        "{1}{U}",
        &["Creature", "Bird"],
        (1, 1),
        3,
        "81b991e7db1bf7f76282f339fb25b939615fe2e90514f7f282a9056f54775951",
    ),
    (
        "silver_raven",
        "Silver Raven",
        "{U}",
        &["Artifact", "Creature", "Bird"],
        (1, 1),
        1,
        "fe75845757dc8b0f27232f668ba38212d7895256611694c4a56c51c0676cfc77",
    ),
    (
        "thaumaturges_familiar",
        "Thaumaturge's Familiar",
        "{3}",
        &["Artifact", "Creature", "Bird"],
        (1, 3),
        1,
        "da877542407877a738b79d4e469934ac003c82986447e7d76bfc8e93c0292b0c",
    ),
    (
        "senate_griffin",
        "Senate Griffin",
        "{2}{W/U}{W/U}",
        &["Creature", "Griffin"],
        (3, 2),
        1,
        "da877542407877a738b79d4e469934ac003c82986447e7d76bfc8e93c0292b0c",
    ),
    (
        "faerie_seer",
        "Faerie Seer",
        "{U}",
        &["Creature", "Faerie", "Wizard"],
        (1, 1),
        2,
        "3014024ec11789a2cc181ff20a9e7131a6804d42e3759a75614e7225556755af",
    ),
    (
        "cloudreader_sphinx",
        "Cloudreader Sphinx",
        "{4}{U}",
        &["Creature", "Sphinx"],
        (3, 4),
        2,
        "3014024ec11789a2cc181ff20a9e7131a6804d42e3759a75614e7225556755af",
    ),
    (
        "willow-wind",
        "Willow-Wind",
        "{4}{U}",
        &["Creature", "Elemental"],
        (3, 4),
        2,
        "581eea00ef4b981320a9058efe9a9c9c9107246d89e46e91c9219cb5393e0bf3",
    ),
];

fn face<'a>(
    registry: &'a tricerules_cards::CardRegistry,
    id: &str,
) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn flying_etb_scry_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, body, scry, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id.replace('-', "_"), "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            types,
            "{id} types"
        );
        assert!(f.supertypes.is_empty(), "{id} supertypes");
        assert_eq!(
            (f.power, f.toughness),
            (Some(body.0), Some(body.1)),
            "{id} body"
        );
        assert_eq!(f.keywords, [Keyword::Flying], "{id} keyword");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");

        let [ability] = f.triggered_abilities.as_slice() else {
            panic!("{id}: exactly one triggered ability");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id} presentation"
        );
        assert_eq!(
            ability.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::Scry { count: Amount::Fixed(count) }] if *count == scry
            ),
            "{id}: printed scry {scry}"
        );
        assert!(ability.targeting.is_none(), "{id}");
    }
}

#[test]
fn flying_etb_scry_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, _, expected) in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id.replace('-', "_"));
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
