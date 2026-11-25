//! Registry conformance for the queue-v77 01-target-quality-removal cohort.
//!
//! Every card is a one-clause black removal spell whose only difference is the printed creature
//! target filter. Fingerprints are checked after the presentation metadata refresh.

use tricerules_cards::primitives::{
    Color, EffectSubject, SpellEffectKind, TargetChooser, TargetController, TargetFilter,
    TargetKind, TargetingDef,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    filter: TargetFilter,
    prompt: &'static str,
    fingerprint: &'static str,
}

fn cases() -> Vec<Card> {
    vec![
        Card {
            id: "rend_flesh",
            name: "Rend Flesh",
            mana_cost: "{2}{B}",
            types: &["Instant", "Arcane"],
            filter: TargetFilter {
                kind: TargetKind::Creature,
                excluded_subtypes: vec!["Spirit".into()],
                ..Default::default()
            },
            prompt: "Choose target non-Spirit creature",
            fingerprint: "bce58023351887b1b3c2a1cba3f2b0b84be3e72106a2ddf3ffecbc180fc95399",
        },
        Card {
            id: "asphyxiate",
            name: "Asphyxiate",
            mana_cost: "{1}{B}{B}",
            types: &["Sorcery"],
            filter: TargetFilter {
                kind: TargetKind::Creature,
                tapped: Some(false),
                ..Default::default()
            },
            prompt: "Choose target untapped creature",
            fingerprint: "8fda2100f871b857613cc05032bc2b03f7efddf2bbf0d20a8893acbaf7a44891",
        },
        Card {
            id: "heros_demise",
            name: "Hero's Demise",
            mana_cost: "{1}{B}",
            types: &["Instant"],
            filter: TargetFilter {
                kind: TargetKind::Creature,
                required_supertypes: vec!["Legendary".into()],
                ..Default::default()
            },
            prompt: "Choose target legendary creature",
            fingerprint: "f73e314ddd94e4616b49a19c6cf6496dd220ec5037735ce2fbe8cf5926f89085",
        },
        Card {
            id: "walk_the_plank",
            name: "Walk the Plank",
            mana_cost: "{B}{B}",
            types: &["Sorcery"],
            filter: TargetFilter {
                kind: TargetKind::Creature,
                excluded_subtypes: vec!["Merfolk".into()],
                ..Default::default()
            },
            prompt: "Choose target non-Merfolk creature",
            fingerprint: "828129b357389d0eb3b7e89db1500cbced026e6acd1f4c6247a4d7e666e0ad19",
        },
        Card {
            id: "chill_to_the_bone",
            name: "Chill to the Bone",
            mana_cost: "{3}{B}",
            types: &["Instant"],
            filter: TargetFilter {
                kind: TargetKind::Creature,
                excluded_supertypes: vec!["Snow".into()],
                ..Default::default()
            },
            prompt: "Choose target nonsnow creature",
            fingerprint: "75aebc69d1cecad0986a07c06017671b280243d38a3733db35141009975fb637",
        },
        Card {
            id: "human_frailty",
            name: "Human Frailty",
            mana_cost: "{B}",
            types: &["Instant"],
            filter: TargetFilter {
                kind: TargetKind::Creature,
                required_subtypes: vec!["Human".into()],
                ..Default::default()
            },
            prompt: "Choose target Human creature",
            fingerprint: "cbe4992865e325c10292255100a90026b705f753aff894286d9d83331be20614",
        },
        Card {
            id: "cast_down",
            name: "Cast Down",
            mana_cost: "{1}{B}",
            types: &["Instant"],
            filter: TargetFilter {
                kind: TargetKind::Creature,
                excluded_supertypes: vec!["Legendary".into()],
                ..Default::default()
            },
            prompt: "Choose target nonlegendary creature",
            fingerprint: "269542ae84a4c4c7553986593cb9732319cb23a8353609c7b0a646029075b728",
        },
        Card {
            id: "dark_betrayal",
            name: "Dark Betrayal",
            mana_cost: "{B}",
            types: &["Instant"],
            filter: TargetFilter {
                kind: TargetKind::Creature,
                is_color: Some(Color::Black),
                ..Default::default()
            },
            prompt: "Choose target black creature",
            fingerprint: "8ea34682a85d610cb9cd67dca2c4ff086e4f1c80c5074c8dabf25ff370960713",
        },
        Card {
            id: "victim_of_night",
            name: "Victim of Night",
            mana_cost: "{B}{B}",
            types: &["Instant"],
            filter: TargetFilter {
                kind: TargetKind::Creature,
                excluded_subtypes: vec!["Vampire".into(), "Werewolf".into(), "Zombie".into()],
                ..Default::default()
            },
            prompt: "Choose target non-Vampire, non-Werewolf, non-Zombie creature",
            fingerprint: "35dc089d70478eb48d1ed45eb5206d2c0486bca7ce852a8cba468196ee17cc29",
        },
        Card {
            id: "power_word_kill",
            name: "Power Word Kill",
            mana_cost: "{1}{B}",
            types: &["Instant"],
            filter: TargetFilter {
                kind: TargetKind::Creature,
                excluded_subtypes: vec![
                    "Angel".into(),
                    "Demon".into(),
                    "Devil".into(),
                    "Dragon".into(),
                ],
                ..Default::default()
            },
            prompt: "Choose target non-Angel, non-Demon, non-Devil, non-Dragon creature",
            fingerprint: "8c3caabd35b33221a7256b5af5587ee53f8fec2a672130f440b479afdf7a9f85",
        },
    ]
}

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
fn q77_quality_removal_map_definitions() {
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
        let f = face(registry, card.id);
        assert_eq!(f.face_id.as_str(), card.id, "{} face id", card.id);
        assert_eq!(f.name, card.name, "{} name", card.id);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost, "{} mana", card.id);
        assert_eq!(f.types, card.types, "{} types", card.id);
        assert_eq!((f.power, f.toughness), (None, None), "{} body", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.activated_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);

        let [effect] = f.spell_effect.as_slice() else {
            panic!("{}: exactly one spell effect", card.id);
        };
        match effect {
            SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(filter),
            } => assert_eq!(&**filter, &card.filter, "{} filter", card.id),
            other => panic!("{}: expected Destroy(Chosen), got {other:?}", card.id),
        }

        let targeting = f
            .targeting
            .as_ref()
            .unwrap_or_else(|| panic!("{} targeting", card.id));
        assert_eq!(
            *targeting,
            TargetingDef {
                groups: vec![tricerules_cards::primitives::TargetGroupDef {
                    min: 1,
                    max: 1,
                    prompt: card.prompt.to_string(),
                    effect_indices: vec![0],
                    for_each_opponent: false,
                    chooser: TargetChooser::Controller,
                    controlled_by_chooser: false,
                    distinct_from: Vec::new(),
                    relation: None,
                    cast_cost_expansion: None
                }],
            },
            "{} targeting",
            card.id
        );
        let group = &targeting.groups[0];
        assert!(!group.for_each_opponent, "{}", card.id);
        assert_eq!(group.chooser, TargetChooser::Controller, "{}", card.id);
        assert!(!group.controlled_by_chooser, "{}", card.id);
        assert!(group.distinct_from.is_empty(), "{}", card.id);
        assert!(group.relation.is_none(), "{}", card.id);
        assert!(group.cast_cost_expansion.is_none(), "{}", card.id);
        assert!(
            matches!(card.filter.kind, TargetKind::Creature),
            "{}",
            card.id
        );
        assert!(
            card.filter.controller == TargetController::Any,
            "{}: no controller restriction",
            card.id
        );
    }
}

#[test]
fn q77_quality_removal_fingerprints_match_the_pinned_oracle_text() {
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
