//! Registry conformance for the reviewed white vigilance-grant cohort.
//!
//! Every included card is a normal-layout creature with one `{W}` activated ability that gives itself vigilance until end of turn.
//! Each included card is executed by the shared actual-card scenarios.
use tricerules_cards::primitives::{AbilityCost, EffectSubject, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Keyword, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
    Option<(u32, u32)>,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "bladed_sentinel",
        "Bladed Sentinel",
        "{4}",
        &["Artifact", "Creature", "Construct"],
        &[],
        Some((2, 4)),
        "e9ce5dba47e28aadeba84b449a12ca03bb88a05df01c5aa9eb6471322ba81390",
    ),
    (
        "llanowar_cavalry",
        "Llanowar Cavalry",
        "{2}{G}",
        &["Creature", "Human", "Soldier"],
        &[],
        Some((1, 4)),
        "e9ce5dba47e28aadeba84b449a12ca03bb88a05df01c5aa9eb6471322ba81390",
    ),
    (
        "bastion_mastodon",
        "Bastion Mastodon",
        "{5}",
        &["Artifact", "Creature", "Elephant"],
        &[],
        Some((4, 5)),
        "e9ce5dba47e28aadeba84b449a12ca03bb88a05df01c5aa9eb6471322ba81390",
    ),
    (
        "towering_thunderfist",
        "Towering Thunderfist",
        "{4}{R}",
        &["Creature", "Giant", "Soldier"],
        &[],
        Some((4, 4)),
        "e9ce5dba47e28aadeba84b449a12ca03bb88a05df01c5aa9eb6471322ba81390",
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
fn white_gain_vigilance_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, supertypes, body, _) in CARDS {
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
        assert!(f.protections.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: exactly one activated ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} presentation"
        );
        assert!(
            matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost)] if cost.to_string() == "{W}"),
            "{id}: exact vigilance cost"
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::GrantKeywords {
                    subject: EffectSubject::Source,
                    keywords,
                }] if keywords.as_slice() == [Keyword::Vigilance]
            ),
            "{id}: source-only vigilance grant"
        );
        assert!(ability.targeting.is_none(), "{id}");
    }
}

#[test]
fn white_gain_vigilance_batch_fingerprints_match_the_pinned_oracle_text() {
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
