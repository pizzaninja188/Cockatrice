//! Registry conformance for #524: Revive and Reborn Hope return one card of a required color
//! shape from the caster's graveyard. Card names and costs are printed data; the shared
//! scenario executes both cards. Each fingerprint is SHA-256 of the pinned normalized Oracle text.

use tricerules_cards::primitives::{
    GraveyardDestination, GraveyardFilter, SpellEffectKind, ZoneCardFilter,
};
use tricerules_cards::{Color, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    filter: ZoneCardFilter,
    prompt: &'static str,
    fingerprint: &'static str,
}

fn cards() -> [Card; 2] {
    [
        Card {
            id: "revive",
            name: "Revive",
            mana: "{1}{G}",
            filter: ZoneCardFilter {
                is_color: Some(Color::Green),
                ..Default::default()
            },
            prompt: "Choose target green card from your graveyard",
            fingerprint: "15a52f4492209de97f1534565116d885e2c25930bc87bbfd6b9f73db252ed13c",
        },
        Card {
            id: "reborn_hope",
            name: "Reborn Hope",
            mana: "{G}{W}",
            filter: ZoneCardFilter {
                is_multicolored: Some(true),
                ..Default::default()
            },
            prompt: "Choose target multicolored card from your graveyard",
            fingerprint: "a2cdf60d7bf3e4607d789edc5d430da988bc56812333727d0462b133ccba911e",
        },
    ]
}

#[test]
fn issue_524_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in cards() {
        let id = card.id;
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(card.name), Some(id), "{id}");
        let f = definition.primary_face();
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, card.name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), card.mana, "{id} mana cost");
        assert_eq!(f.types, ["Sorcery"], "{id} type");
        assert!(f.supertypes.is_empty(), "{id} supertypes");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        let expected = GraveyardFilter {
            card: Some(card.filter.clone()),
            ..GraveyardFilter::default()
        };
        assert!(
            matches!(
                f.spell_effect.as_slice(),
                [SpellEffectKind::MoveGraveyardCards { filter, destination, linked_exile_id: None, entry_modifiers }]
                    if *filter == expected
                        && *destination == GraveyardDestination::Hand
                        && entry_modifiers.is_empty()
            ),
            "{id}: one matching card from the caster's graveyard to hand"
        );
        let schema = tricerules_cards::primitives::TargetSchema::compile(
            &f.spell_effect,
            f.targeting.as_ref(),
        )
        .expect("valid implicit targeting");
        let [group] = schema.groups.as_slice() else {
            panic!("{id}: exactly one target group");
        };
        assert_eq!((group.min, group.max), (1, 1), "{id}");
        assert_eq!(
            crate::common::compiled_legacy_target_prompt(&f.spell_effect, f.targeting.as_ref(), 0),
            card.prompt,
            "{id}"
        );
    }
}

#[test]
fn issue_524_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in cards() {
        let id = card.id;
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(
            fields,
            [id, card.name, id, card.name, card.fingerprint],
            "{id}"
        );
    }
}
