//! Registry conformance for the reviewed enters-scry value cohort.
//!
//! Every included card is a normal-layout creature with an enters-the-battlefield
//! trigger that scries exactly its printed count and no other rules text.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.
use tricerules_cards::primitives::SpellEffectKind;
use tricerules_cards::{AbilityPresentation, Amount, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
    Option<(u32, u32)>,
    u32,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "chrome_cat",
        "Chrome Cat",
        "{3}",
        &["Artifact", "Creature", "Cat"],
        &[],
        Some((3, 2)),
        1,
        "0dd754af518eed71759e7a7abcdb97aa12f9ea2db69fce9701e82aaee8325c6a",
    ),
    (
        "galadhrim_guide",
        "Galadhrim Guide",
        "{3}{G}",
        &["Creature", "Elf", "Scout"],
        &[],
        Some((3, 4)),
        2,
        "334c92b9a1542869c7da9a5507dd91ec830bcf0866c5fb36f1ca5b484ea8d978",
    ),
    (
        "rumbling_sentry",
        "Rumbling Sentry",
        "{3}{W}{W}",
        &["Creature", "Giant"],
        &[],
        Some((3, 6)),
        1,
        "0dd754af518eed71759e7a7abcdb97aa12f9ea2db69fce9701e82aaee8325c6a",
    ),
    (
        "sages_row_savant",
        "Sage's Row Savant",
        "{1}{U}",
        &["Creature", "Vedalken", "Wizard"],
        &[],
        Some((2, 1)),
        2,
        "334c92b9a1542869c7da9a5507dd91ec830bcf0866c5fb36f1ca5b484ea8d978",
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
fn etb_scry_value_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, supertypes, body, count, _) in CARDS {
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
        assert_eq!(
            f.supertypes.iter().map(String::as_str).collect::<Vec<_>>(),
            supertypes,
            "{id} supertypes"
        );
        assert_eq!(
            (f.power, f.toughness),
            body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
            "{id} body"
        );
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        let [ability] = f.triggered_abilities.as_slice() else {
            panic!("{id}: exactly one triggered ability");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} presentation"
        );
        assert_eq!(
            ability.trigger,
            tricerules_cards::primitives::TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::Scry {
                    count: Amount::Fixed(value)
                }] if *value == count
            ),
            "{id}: printed scry count"
        );
        assert!(ability.targeting.is_none(), "{id}");
    }
}

#[test]
fn etb_scry_value_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, _, _, expected) in CARDS {
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
