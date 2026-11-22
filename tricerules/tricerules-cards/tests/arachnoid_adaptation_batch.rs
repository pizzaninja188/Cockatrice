//! Registry conformance for the reviewed Arachnoid Adaptation pump-reach-untap family.
//!
//! Every included card is a normal-layout Instant with one shared mandatory creature
//! target, one P/T pump, one Reach grant and one Untap effect. Card names, costs and
//! numbers are printed data; the shared scenario executes every card. Each fingerprint
//! is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{EffectSubject, SpellEffectKind, TargetFilter};
use tricerules_cards::{Keyword, Layout};

type Card = (&'static str, &'static str, &'static str, i32, i32);

const CARDS: &[Card] = &[
    ("arachnoid_adaptation", "Arachnoid Adaptation", "{G}", 2, 2),
    (
        "tethermages_advantage",
        "Tethermage's Advantage",
        "{G}",
        2,
        2,
    ),
    ("vines_of_the_recluse", "Vines of the Recluse", "{G}", 1, 2),
    ("witchs_web", "Witch's Web", "{1}{G}", 3, 3),
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
fn arachnoid_adaptation_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, power, toughness) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, ["Instant"], "{id} types");
        assert_eq!(f.power, None, "{id} has no power");
        assert_eq!(f.toughness, None, "{id} has no toughness");
        assert!(f.keywords.is_empty(), "{id} has no printed keyword");
        assert!(
            f.activated_abilities.is_empty(),
            "{id} has no activated ability"
        );
        assert!(
            f.triggered_abilities.is_empty(),
            "{id} has no triggered ability"
        );
        assert!(f.static_abilities.is_empty(), "{id} has no static ability");

        let chosen = EffectSubject::Chosen(Box::new(TargetFilter::default_creature()));
        assert_eq!(
            f.spell_effect,
            [
                SpellEffectKind::PumpTarget {
                    power,
                    toughness,
                    scale: None,
                    subject: chosen.clone(),
                },
                SpellEffectKind::GrantKeywords {
                    subject: chosen.clone(),
                    keywords: vec![Keyword::Reach],
                },
                SpellEffectKind::Untap { subject: chosen },
            ],
            "{id}: exact pump, reach and untap payload"
        );
        assert!(
            f.targeting.is_none(),
            "{id}: one shared mandatory creature target uses the implicit contract"
        );
    }
}

#[test]
fn arachnoid_adaptation_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _) in CARDS {
        let expected = match id {
            "arachnoid_adaptation" => {
                "c372a661bf4599db4c5fcd7da5ee48dd591439aa048425be27299c1b8bcd78a6"
            }
            "tethermages_advantage" => {
                "c372a661bf4599db4c5fcd7da5ee48dd591439aa048425be27299c1b8bcd78a6"
            }
            "vines_of_the_recluse" => {
                "9ee6b69441fc64f8d380f7d5193de2516a574519c2e01aa23fe729556cd68a09"
            }
            "witchs_web" => "bf23716bc8baddfa6a918bcba0ead7cd7dda43e0f0d9a2ac665c51a1e973339c",
            _ => unreachable!(),
        };
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id);
        assert_eq!(fields[3], name);
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
