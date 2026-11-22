//! Registry conformance for the queue-v19 flying regeneration cohort.
//!
//! Every included card is a normal-layout creature with flying and one
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
        "fog_of_gnats",
        "Fog of Gnats",
        "{B}{B}",
        &["Creature", "Insect"],
        1,
        1,
        "{B}",
        "55d80e52c7bd1fe36e3057090fd3436776e1ae940308690c27c9d0d58cfc9974",
    ),
    (
        "tattered_drake",
        "Tattered Drake",
        "{4}{U}",
        &["Creature", "Zombie", "Drake"],
        2,
        2,
        "{B}",
        "55d80e52c7bd1fe36e3057090fd3436776e1ae940308690c27c9d0d58cfc9974",
    ),
    (
        "will-o-the-wisp",
        "Will-o'-the-Wisp",
        "{B}",
        &["Creature", "Spirit"],
        0,
        1,
        "{B}",
        "8bb941a639799c1f0d1b0bead0207a56cbece1aea03bf0c6e90f5c6eb4df2128",
    ),
    (
        "yavimaya_gnats",
        "Yavimaya Gnats",
        "{2}{G}",
        &["Creature", "Insect"],
        0,
        1,
        "{G}",
        "69fa84e8e58cce0f2bf0246a28e61867e2adc66ef0f3680ec15bc48e063c08a9",
    ),
];

#[test]
fn flier_regenerate_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, power, toughness, regenerate, _fingerprint) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(definition.name, name, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id.replace('-', "_"), "{id} face id");
        assert_eq!(face.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(face.types, types, "{id} types");
        assert_eq!(
            (face.power, face.toughness),
            (Some(power), Some(toughness)),
            "{id}"
        );
        assert_eq!(face.keywords, [Keyword::Flying], "{id} keywords");
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
fn flier_regenerate_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], id.replace('-', "_"));
        assert_eq!(fields[4], fingerprint, "fingerprint drift for {id}");
    }
}
