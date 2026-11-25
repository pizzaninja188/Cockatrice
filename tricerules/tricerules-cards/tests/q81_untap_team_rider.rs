//! Registry conformance for the queue-v81 03-untap-team-rider cohort.
//!
//! Each card is a normal instant that untaps every creature its controller controls and applies
//! one printed mass rider. Fingerprints are checked after the presentation metadata refresh.

use tricerules_cards::primitives::{
    CreatureScopeController, CreatureScopeFilter, MassPlayerSet, SpellEffectKind, TargetKind,
};
use tricerules_cards::{Keyword, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    rider: Rider,
    fingerprint: &'static str,
}

enum Rider {
    SamuraiPump,
    FlyingPump,
    HexproofIndestructible,
}

fn cases() -> Vec<Card> {
    vec![
        Card {
            id: "call_to_glory",
            name: "Call to Glory",
            mana_cost: "{1}{W}",
            rider: Rider::SamuraiPump,
            fingerprint: "4440e75e30c35a423cb9ef0ceaa168bba8de0a27892bb07bba05881cd353b570",
        },
        Card {
            id: "rally_of_wings",
            name: "Rally of Wings",
            mana_cost: "{1}{W}",
            rider: Rider::FlyingPump,
            fingerprint: "b730585bd66d6fccc8ccff2acd8619f577fdf445b3630f1ef464d4e37d714e84",
        },
        Card {
            id: "join_shields",
            name: "Join Shields",
            mana_cost: "{3}{G}{W}",
            rider: Rider::HexproofIndestructible,
            fingerprint: "7b39eb06f4643a5eb71112bffc96a71df7b5185a9085bece7294ac7e3e1bf265",
        },
    ]
}

fn you_control_only(filter: &CreatureScopeFilter) -> bool {
    matches!(filter.controller, Some(CreatureScopeController::YouControl))
        && !filter.exclude_self
        && !filter.attacking
        && filter.name.is_none()
        && filter.color.is_none()
        && !filter.requires_any_counter
        && filter.required_counter.is_none()
}

#[test]
fn q81_untap_rider_map_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in cases() {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(
            registry.id_for_name(card.name),
            Some(card.id),
            "{}",
            card.id
        );
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), card.id, "{} face id", card.id);
        assert_eq!(face.name, card.name, "{} name", card.id);
        assert_eq!(
            face.mana_cost.to_string(),
            card.mana_cost,
            "{} mana",
            card.id
        );
        assert_eq!(face.types, ["Instant"], "{} types", card.id);
        assert!(face.power.is_none(), "{}", card.id);
        assert!(face.toughness.is_none(), "{}", card.id);
        assert!(face.keywords.is_empty(), "{}", card.id);
        assert!(face.protections.is_empty(), "{}", card.id);
        assert!(face.static_abilities.is_empty(), "{}", card.id);
        assert!(face.activated_abilities.is_empty(), "{}", card.id);
        assert!(face.triggered_abilities.is_empty(), "{}", card.id);
        assert!(face.targeting.is_none(), "{}", card.id);

        let [SpellEffectKind::UntapAll { players, filter }, rider] = face.spell_effect.as_slice()
        else {
            panic!("{}: untap plus one rider", card.id);
        };
        assert_eq!(*players, MassPlayerSet::Controller, "{}", card.id);
        assert_eq!(filter.kind, TargetKind::Creature, "{}", card.id);
        assert_eq!(
            filter.required_keywords,
            Vec::<Keyword>::new(),
            "{}",
            card.id
        );

        match card.rider {
            Rider::SamuraiPump => {
                let SpellEffectKind::PumpAll {
                    filter,
                    power,
                    toughness,
                } = rider
                else {
                    panic!("{}: Samurai pump", card.id);
                };
                assert_eq!((*power, *toughness), (1, 1), "{}", card.id);
                assert!(you_control_only(filter), "{}", card.id);
                assert_eq!(filter.subtype.as_deref(), Some("Samurai"), "{}", card.id);
                assert!(filter.required_keyword.is_none(), "{}", card.id);
            }
            Rider::FlyingPump => {
                let SpellEffectKind::PumpAll {
                    filter,
                    power,
                    toughness,
                } = rider
                else {
                    panic!("{}: flying pump", card.id);
                };
                assert_eq!((*power, *toughness), (2, 2), "{}", card.id);
                assert!(you_control_only(filter), "{}", card.id);
                assert!(filter.subtype.is_none(), "{}", card.id);
                assert_eq!(
                    filter.required_keyword,
                    Some(Keyword::Flying),
                    "{}",
                    card.id
                );
            }
            Rider::HexproofIndestructible => {
                let SpellEffectKind::GrantKeywordsAll { filter, keywords } = rider else {
                    panic!("{}: keyword grant", card.id);
                };
                assert!(you_control_only(filter), "{}", card.id);
                assert_eq!(
                    keywords,
                    &[Keyword::Hexproof, Keyword::Indestructible],
                    "{}",
                    card.id
                );
                assert!(filter.subtype.is_none(), "{}", card.id);
                assert!(filter.required_keyword.is_none(), "{}", card.id);
            }
        }
    }
}

#[test]
fn q81_untap_rider_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in cases() {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(fields[2], card.id, "face id drift for {}", card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
