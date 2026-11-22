//! Registry conformance for the reviewed tap-and-lock Aura family.
//!
//! Every included card is a normal-layout card with the same Auras that tap the enchanted creature and stop it untapping
//! shape. Card names, costs, types and bodies are printed data; the shared
//! scenario executes every card. Each fingerprint is SHA-256 of the exact
//! pinned normalized Oracle text.

use tricerules_cards::primitives::{
    EffectSubject, SpellEffectKind, StaticAbilityDef, TargetController, TargetKind,
};
use tricerules_cards::{AbilityPresentation, Layout, TriggerCondition};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    Option<(u32, u32)>,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "castaways_despair",
        "Castaway's Despair",
        "{3}{U}",
        &["Enchantment", "Aura"],
        None,
        "c0bee70da07be9afe62efc3937b38ca0bf4f83361c157fc6dbc79026f4b5987e",
    ),
    (
        "containment_protocol",
        "Containment Protocol",
        "{2}{U}",
        &["Enchantment", "Aura"],
        None,
        "c0bee70da07be9afe62efc3937b38ca0bf4f83361c157fc6dbc79026f4b5987e",
    ),
    (
        "sleep_paralysis",
        "Sleep Paralysis",
        "{3}{U}",
        &["Enchantment", "Aura"],
        None,
        "c0bee70da07be9afe62efc3937b38ca0bf4f83361c157fc6dbc79026f4b5987e",
    ),
    (
        "waterknot",
        "Waterknot",
        "{1}{U}{U}",
        &["Enchantment", "Aura"],
        None,
        "c0bee70da07be9afe62efc3937b38ca0bf4f83361c157fc6dbc79026f4b5987e",
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
fn aura_tap_lock_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, body, _) in CARDS {
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
        assert!(f.supertypes.is_empty(), "{id} supertypes");
        assert_eq!(
            (f.power, f.toughness),
            body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
            "{id} body"
        );

        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        let [SpellEffectKind::AuraAttach { target }] = f.spell_effect.as_slice() else {
            panic!("{id}: exactly one AuraAttach");
        };
        assert_eq!(target.kind, TargetKind::Creature, "{id}");
        assert_eq!(target.controller, TargetController::Any, "{id}");
        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{id}: exactly one entry trigger");
        };
        assert_eq!(trigger.ability_id.as_str(), "triggered_01", "{id}");
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id} trigger presentation"
        );
        assert_eq!(
            trigger.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        assert_eq!(
            trigger.effect,
            [SpellEffectKind::Tap {
                subject: EffectSubject::AttachedObject,
            }],
            "{id}"
        );
        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{id}: exactly one attached modifier");
        };
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![3]),
            "{id} static presentation"
        );
        assert!(
            matches!(
                &static_ability.definition,
                StaticAbilityDef::AttachedModifier {
                    doesnt_untap_during_untap_step: true,
                    ..
                }
            ),
            "{id}: the enchanted creature does not untap"
        );
    }
}

#[test]
fn aura_tap_lock_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, expected) in CARDS {
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
