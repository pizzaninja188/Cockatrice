//! Registry conformance for the queue-v19 defender regeneration cohort.
//!
//! Every included card is a normal-layout creature with defender and one
//! `{cost}: Regenerate this creature.` activated ability. Card names, costs, types and
//! stats are printed data; the shared scenario executes every card. Each fingerprint
//! is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, EffectSubject, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, ActivationTiming, Keyword, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    u32,
    u32,
    &'static str,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "living_wall",
        "Living Wall",
        "{4}",
        &["Artifact", "Creature", "Wall"],
        0,
        6,
        "{1}",
        "3abe42bcf16b7d895ad094998051a55fee1c93b6b5a499aa81c45168223c7f18",
    ),
    (
        "wall_of_bone",
        "Wall of Bone",
        "{2}{B}",
        &["Creature", "Skeleton", "Wall"],
        1,
        4,
        "{B}",
        "e656040762232ab35eea7f1dbff937a74c75881c5560476589f008525c6d426f",
    ),
    (
        "wall_of_brambles",
        "Wall of Brambles",
        "{2}{G}",
        &["Creature", "Plant", "Wall"],
        2,
        3,
        "{G}",
        "38739dcd713cd8d75da976ff06678b04bcec56d2b3e416a1b515cbcb06528648",
    ),
    (
        "wall_of_pine_needles",
        "Wall of Pine Needles",
        "{3}{G}",
        &["Creature", "Plant", "Wall"],
        3,
        3,
        "{G}",
        "38739dcd713cd8d75da976ff06678b04bcec56d2b3e416a1b515cbcb06528648",
    ),
];

#[test]
fn wall_regenerate_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, power, toughness, regenerate, _fingerprint) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(definition.name, name, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id, "{id} face id");
        assert_eq!(face.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(face.types, types, "{id} types");
        assert_eq!(
            (face.power, face.toughness),
            (Some(power), Some(toughness)),
            "{id}"
        );
        assert_eq!(face.keywords, [Keyword::Defender], "{id} keywords");
        assert!(face.spell_effect.is_empty(), "{id}");
        assert!(face.targeting.is_none(), "{id}");
        assert!(face.triggered_abilities.is_empty(), "{id}");
        assert!(face.static_abilities.is_empty(), "{id}");

        let [ability] = face.activated_abilities.as_slice() else {
            panic!("{id}: one activated ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id} presentation"
        );
        assert_eq!(ability.timing, ActivationTiming::Normal, "{id} timing");
        assert!(
            matches!(
                ability.costs.as_slice(),
                [AbilityCost::Mana(cost)] if cost.to_string() == regenerate
            ),
            "{id}: {:?}",
            ability.costs
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::Regenerate {
                    subject: EffectSubject::Source,
                }]
            ),
            "{id}: {:?}",
            ability.effect
        );
        assert!(ability.targeting.is_none(), "{id}: no targets");
    }
}

#[test]
fn wall_regenerate_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _mana, _types, _power, _toughness, _regenerate, fingerprint) in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id);
        assert_eq!(fields[4], fingerprint, "fingerprint drift for {id}");
    }
}
