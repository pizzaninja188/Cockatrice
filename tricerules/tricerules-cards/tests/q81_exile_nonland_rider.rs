//! Registry conformance for the queue-v81 02-exile-nonland-rider cohort.
//!
//! Each card is a normal instant or sorcery that exiles one target nonland permanent and applies
//! one printed rider. Fingerprints are checked after the presentation metadata refresh.

use tricerules_cards::primitives::{
    Amount, EffectSubject, LifeAmount, PermanentTypeFilter, PlayerRecipient, SpellEffectKind,
    TargetKind,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    rider: Rider,
    fingerprint: &'static str,
}

enum Rider {
    LoseThree,
    Treasure,
    Soldier,
}

fn cases() -> Vec<Card> {
    vec![
        Card {
            id: "anguished_unmaking",
            name: "Anguished Unmaking",
            mana_cost: "{1}{W}{B}",
            types: &["Instant"],
            rider: Rider::LoseThree,
            fingerprint: "a8d7c3b29d70efbb94a0595877a146307481480a995d0cc93361ca2d3cff8651",
        },
        Card {
            id: "buy_your_silence",
            name: "Buy Your Silence",
            mana_cost: "{4}{W}",
            types: &["Sorcery"],
            rider: Rider::Treasure,
            fingerprint: "f9156362bfa2e5ff0adb216bdcfe7618ddecc986126a9d9777cf3d14d574b6af",
        },
        Card {
            id: "secure_the_scene",
            name: "Secure the Scene",
            mana_cost: "{4}{W}",
            types: &["Sorcery"],
            rider: Rider::Soldier,
            fingerprint: "bc108233ccdeccff9cb7c5d9943cdba486d5cd0030f39041336a2e5fcf74762a",
        },
    ]
}

#[test]
fn q81_exile_rider_map_definitions() {
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
        assert_eq!(face.types, card.types, "{} types", card.id);
        assert!(face.power.is_none(), "{}", card.id);
        assert!(face.toughness.is_none(), "{}", card.id);
        assert!(face.keywords.is_empty(), "{}", card.id);
        assert!(face.protections.is_empty(), "{}", card.id);
        assert!(face.static_abilities.is_empty(), "{}", card.id);
        assert!(face.activated_abilities.is_empty(), "{}", card.id);
        assert!(face.triggered_abilities.is_empty(), "{}", card.id);

        let [SpellEffectKind::Exile { subject, .. }, rider] = face.spell_effect.as_slice() else {
            panic!("{}: exile plus one rider", card.id);
        };
        let EffectSubject::Chosen(filter) = subject else {
            panic!("{}: chosen target subject", card.id);
        };
        assert_eq!(filter.kind, TargetKind::AnyPermanent, "{}", card.id);
        assert_eq!(
            filter.excluded_permanent_types,
            vec![PermanentTypeFilter::Land],
            "{}: nonland",
            card.id
        );
        match card.rider {
            Rider::LoseThree => assert_eq!(
                *rider,
                SpellEffectKind::LoseLife {
                    amount: LifeAmount::Fixed(3),
                    who: PlayerRecipient::Controller,
                },
                "{}",
                card.id
            ),
            Rider::Treasure => assert!(
                matches!(
                    rider,
                    SpellEffectKind::CreateTokens {
                        token,
                        count: Amount::Fixed(1),
                        who: PlayerRecipient::ControllerOfTargetGroup { group_index: 0 },
                        ..
                    } if token == "treasure"
                ),
                "{}",
                card.id
            ),
            Rider::Soldier => assert!(
                matches!(
                    rider,
                    SpellEffectKind::CreateTokens {
                        token,
                        count: Amount::Fixed(1),
                        who: PlayerRecipient::ControllerOfTargetGroup { group_index: 0 },
                        ..
                    } if token == "soldier_w_1_1"
                ),
                "{}",
                card.id
            ),
        }

        let targeting = face.targeting.as_ref().expect("targeting");
        let [group] = targeting.groups.as_slice() else {
            panic!("{}: one target group", card.id);
        };
        assert_eq!((group.min, group.max), (1, 1), "{}", card.id);
        assert_eq!(group.effect_indices, vec![0], "{}", card.id);
    }
}

#[test]
fn q81_exile_rider_fingerprints_match_the_pinned_oracle_text() {
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
