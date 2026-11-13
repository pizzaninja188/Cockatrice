//! Registry conformance for the reviewed tapped R/G dual-land family.
//!
//! Every included card is a normal-layout land that enters tapped and has one
//! tap mana ability with the printed R-then-G option order. Card names and the
//! Snow supertype are printed data; the shared scenario executes every card.
//! Each fingerprint is SHA-256 of the exact pinned Oracle lines.

use tricerules_cards::primitives::{
    AbilityCost, EntersTappedAffected, SpellEffectKind, StaticAbilityDef,
};
use tricerules_cards::{AbilityPresentation, Layout};

type Card = (&'static str, &'static str, bool, &'static str);

const CARDS: &[Card] = &[
    (
        "highland_weald",
        "Highland Weald",
        true,
        "0774e3d3dbe63e4f7151643ae4ec569adc4d3381a765cb62ca3bc9085615aae0",
    ),
    (
        "shivan_oasis",
        "Shivan Oasis",
        false,
        "0774e3d3dbe63e4f7151643ae4ec569adc4d3381a765cb62ca3bc9085615aae0",
    ),
    (
        "timber_gorge",
        "Timber Gorge",
        false,
        "0774e3d3dbe63e4f7151643ae4ec569adc4d3381a765cb62ca3bc9085615aae0",
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
fn timber_gorge_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, snow, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), "", "{id} mana cost");
        assert_eq!(f.types, ["Land"], "{id} types");
        assert_eq!(
            f.supertypes,
            if snow {
                vec!["Snow".to_string()]
            } else {
                Vec::<String>::new()
            },
            "{id} snow supertype"
        );
        assert_eq!(f.power, None, "{id} has no power");
        assert_eq!(f.toughness, None, "{id} has no toughness");
        assert!(f.keywords.is_empty(), "{id} has no printed keyword");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");

        let [entry] = f.static_abilities.as_slice() else {
            panic!("{id}: exactly one static ability");
        };
        assert_eq!(entry.ability_id.as_str(), "static_01", "{id}");
        assert_eq!(
            entry.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} entry presentation"
        );
        assert_eq!(
            entry.definition,
            StaticAbilityDef::EntersTapped {
                affected: EntersTappedAffected::Self_,
                condition: None,
                unless_cost: None,
            },
            "{id}"
        );

        let [mana] = f.activated_abilities.as_slice() else {
            panic!("{id}: exactly one activated ability");
        };
        assert_eq!(mana.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            mana.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id} mana presentation"
        );
        assert_eq!(mana.costs, [AbilityCost::Tap], "{id} mana cost");
        assert!(
            matches!(
                mana.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 2
                        && options[0].r == 1
                        && options[0].g == 0
                        && options[1].r == 0
                        && options[1].g == 1
            ),
            "{id}: exact red-then-green mana options"
        );
        assert!(mana.targeting.is_none(), "{id}: no target");
    }
}

#[test]
fn timber_gorge_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, expected) in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id);
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
