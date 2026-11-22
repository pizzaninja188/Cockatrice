//! Registry conformance for the reviewed enters-destroy-artifact cohort.
//!
//! Every included card is a normal-layout creature with an enters-the-battlefield
//! trigger that destroys one target artifact.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::{AbilityPresentation, Layout};

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
        "manic_vandal",
        "Manic Vandal",
        "{2}{R}",
        &["Creature", "Human", "Warrior"],
        &[],
        Some((2, 2)),
        "2bfa08356ecea7d05fd4f1ed0d719652b4e9698687d7f6750ada9d9647edad11",
    ),
    (
        "oxidda_scrapmelter",
        "Oxidda Scrapmelter",
        "{3}{R}",
        &["Creature", "Beast"],
        &[],
        Some((3, 3)),
        "2bfa08356ecea7d05fd4f1ed0d719652b4e9698687d7f6750ada9d9647edad11",
    ),
    (
        "uktabi_orangutan",
        "Uktabi Orangutan",
        "{2}{G}",
        &["Creature", "Ape"],
        &[],
        Some((2, 2)),
        "2bfa08356ecea7d05fd4f1ed0d719652b4e9698687d7f6750ada9d9647edad11",
    ),
    (
        "viridian_shaman",
        "Viridian Shaman",
        "{2}{G}",
        &["Creature", "Elf", "Shaman"],
        &[],
        Some((2, 2)),
        "2bfa08356ecea7d05fd4f1ed0d719652b4e9698687d7f6750ada9d9647edad11",
    ),
    (
        "vithian_renegades",
        "Vithian Renegades",
        "{1}{R}{G}",
        &["Creature", "Human", "Shaman"],
        &[],
        Some((3, 2)),
        "2bfa08356ecea7d05fd4f1ed0d719652b4e9698687d7f6750ada9d9647edad11",
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
fn etb_destroy_artifact_batch_maps_definitions() {
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
        let tricerules_cards::primitives::SpellEffectKind::Destroy {
            subject: tricerules_cards::primitives::EffectSubject::Chosen(filter),
        } = &ability.effect[0]
        else {
            panic!("{id}: destroy with a chosen subject");
        };
        assert_eq!(
            filter.permanent_types,
            [tricerules_cards::primitives::PermanentTypeFilter::Artifact],
            "{id}"
        );
        assert!(filter.excluded_permanent_types.is_empty(), "{id}");
        let targeting = ability.targeting.as_ref().expect("{id}: target");
        let [group] = targeting.groups.as_slice() else {
            panic!("{id}: one target group");
        };
        assert_eq!((group.min, group.max), (1, 1), "{id}");
        assert_eq!(group.effect_indices, [0], "{id}");
    }
}

#[test]
fn etb_destroy_artifact_batch_fingerprints_match_the_pinned_oracle_text() {
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
