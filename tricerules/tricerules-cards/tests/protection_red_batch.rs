//! Registry conformance for the reviewed protection-from-red composition family.
//!
//! Every included card is a normal-layout creature with protection from red and
//! one printed clause: Flash, first strike, or Cycling {2}. Each included card
//! is executed by the shared actual-card scenarios.
use tricerules_cards::primitives::{AbilityCost, ProtectionQuality, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Color, Keyword, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
    Option<(u32, u32)>,
    &'static [Keyword],
    bool,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "defender_of_law",
        "Defender of Law",
        "{2}{W}",
        &["Creature", "Human", "Knight"],
        &[],
        Some((2, 1)),
        &[Keyword::Flash],
        false,
        "211490d7b12a06e80963799c35d7ee7d878a9b0bb5ed77203a4b210462eacf5e",
    ),
    (
        "disciple_of_law",
        "Disciple of Law",
        "{1}{W}",
        &["Creature", "Human", "Cleric"],
        &[],
        Some((1, 2)),
        &[],
        true,
        "6584a14038e9baf6be23ed81dbdd3479aba3e1c823f57ce7cbd8fd7e892898d2",
    ),
    (
        "narwhal",
        "Narwhal",
        "{2}{U}{U}",
        &["Creature", "Whale"],
        &[],
        Some((2, 2)),
        &[Keyword::FirstStrike],
        false,
        "17e5192420eea2a7e7a16e7d55bbdab56017f4e511e41b479f783291c9e4b2e8",
    ),
    (
        "silver_knight",
        "Silver Knight",
        "{W}{W}",
        &["Creature", "Human", "Knight"],
        &[],
        Some((2, 2)),
        &[Keyword::FirstStrike],
        false,
        "17e5192420eea2a7e7a16e7d55bbdab56017f4e511e41b479f783291c9e4b2e8",
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
fn protection_red_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, supertypes, body, keywords, cycling, _) in CARDS {
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
        assert_eq!(f.keywords, keywords, "{id} keywords");
        assert_eq!(
            f.protections,
            [ProtectionQuality::Color(Color::Red)],
            "{id} protection"
        );
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        if cycling {
            let [ability] = f.activated_abilities.as_slice() else {
                panic!("{id}: exactly one cycling ability");
            };
            assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
            assert_eq!(
                ability.presentation,
                AbilityPresentation::OracleLines(vec![2]),
                "{id} presentation"
            );
            assert_eq!(
                ability.source_zone,
                tricerules_cards::AbilitySourceZone::Hand,
                "{id}"
            );
            assert!(
                matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost), AbilityCost::DiscardSelf] if cost.to_string() == "{2}"),
                "{id}: exact cycling cost"
            );
            assert!(
                matches!(ability.effect.as_slice(), [SpellEffectKind::Draw { .. }]),
                "{id}: cycling draws one"
            );
            assert!(ability.targeting.is_none(), "{id}");
        } else {
            assert!(f.activated_abilities.is_empty(), "{id}");
        }
    }
}

#[test]
fn protection_red_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, _, _, _, expected) in CARDS {
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
