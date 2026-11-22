//! Registry conformance for the queue-v19 soft-counter cycling cohort.
//!
//! Every included card is a normal-layout instant with a soft counter and a hand-zone
//! cycling activated ability. The shared scenario executes every card. Each fingerprint
//! is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, Amount, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, AbilitySourceZone, ActivationTiming, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    u32,
    &'static str,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "censor",
        "Censor",
        "{1}{U}",
        1,
        "{U}",
        "1c54322989499eb8fe09bd389c6ec53b4cb7398fbb1eaf2e94672af5928ef859",
    ),
    (
        "miscalculation",
        "Miscalculation",
        "{1}{U}",
        2,
        "{2}",
        "ada29727f92e5290a7bcb0f9b2653e2c2fbf9a8dbb52371ce9210192d7d125fa",
    ),
    (
        "spell_snip",
        "Spell Snip",
        "{2}{U}",
        1,
        "{2}",
        "480f5b113ea6ff5ca2fdb669602b9d23ddbe300561fb58694385cb38f6061310",
    ),
];

#[test]
fn soft_counter_cycling_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, payment, cycle, _fingerprint) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(definition.name, name, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id, "{id} face id");
        assert_eq!(face.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(face.types, ["Instant"], "{id} types");
        assert!(face.keywords.is_empty(), "{id} keywords");
        assert!(face.targeting.is_none(), "{id}: implicit spell target");
        assert!(face.triggered_abilities.is_empty(), "{id}");
        assert!(face.static_abilities.is_empty(), "{id}");
        assert!(
            matches!(
                face.spell_effect.as_slice(),
                [SpellEffectKind::CounterTargetSpell {
                    unless_controller_pays: Some(Amount::Fixed(printed)),
                    ..
                }] if *printed == payment
            ),
            "{id}: {:?}",
            face.spell_effect
        );

        let [ability] = face.activated_abilities.as_slice() else {
            panic!("{id}: one cycling ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id} presentation"
        );
        assert_eq!(
            ability.source_zone,
            AbilitySourceZone::Hand,
            "{id} source zone"
        );
        assert_eq!(ability.timing, ActivationTiming::Normal, "{id} timing");
        assert!(
            matches!(
                ability.costs.as_slice(),
                [AbilityCost::Mana(cost), AbilityCost::DiscardSelf] if cost.to_string() == cycle
            ),
            "{id}: {:?}",
            ability.costs
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::Draw {
                    count: Amount::Fixed(1),
                    ..
                }]
            ),
            "{id}: {:?}",
            ability.effect
        );
        assert!(ability.targeting.is_none(), "{id}: cycling does not target");
    }
}

#[test]
fn soft_counter_cycling_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _mana, _payment, _cycle, fingerprint) in CARDS {
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
