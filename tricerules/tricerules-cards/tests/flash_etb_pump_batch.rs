//! Registry conformance for the queue-v19 flash entry-pump cohort.
//!
//! Every included card is a normal-layout creature with flash and one entry trigger
//! that gives a chosen creature a printed P/T bonus until end of turn. The shared
//! scenario executes every card. Each fingerprint is SHA-256 of the exact pinned
//! normalized Oracle text.

use tricerules_cards::primitives::{EffectSubject, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Keyword, Layout, TriggerCondition};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    u32,
    u32,
    i32,
    i32,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "affa_guard_hound",
        "Affa Guard Hound",
        "{2}{W}",
        &["Creature", "Dog"],
        2,
        2,
        0,
        3,
        "fdbaea8cbbace440463a9c904d1c74192b70c87103ea3897c8c9d8834c67a9d2",
    ),
    (
        "bogardan_rager",
        "Bogardan Rager",
        "{5}{R}",
        &["Creature", "Elemental"],
        3,
        4,
        4,
        0,
        "b910b2a024bfeeeee3445bdf95e48b73e06b0726e7932823dfdc8ccd5d29fab6",
    ),
    (
        "briarpack_alpha",
        "Briarpack Alpha",
        "{3}{G}",
        &["Creature", "Wolf"],
        3,
        3,
        2,
        2,
        "ff918e1eded515e118efdcd690bd3ea557ead23dddf665c10468c93287e77f93",
    ),
];

#[test]
fn flash_etb_pump_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, power, toughness, dp, dt, _fingerprint) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(definition.name, name, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id, "{id} face id");
        assert_eq!(face.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(face.types, types, "{id} types");
        assert_eq!(
            (face.power, face.toughness),
            (Some(power), Some(toughness)),
            "{id}"
        );
        assert_eq!(face.keywords, [Keyword::Flash], "{id} keywords");
        assert!(face.spell_effect.is_empty(), "{id}");
        assert!(face.targeting.is_none(), "{id}");
        assert!(face.activated_abilities.is_empty(), "{id}");
        assert!(face.static_abilities.is_empty(), "{id}");

        let [trigger] = face.triggered_abilities.as_slice() else {
            panic!("{id}: one entry trigger");
        };
        assert_eq!(trigger.ability_id.as_str(), "triggered_01", "{id}");
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id} presentation"
        );
        assert_eq!(
            trigger.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id} trigger"
        );
        assert!(
            matches!(
                trigger.effect.as_slice(),
                [SpellEffectKind::PumpTarget {
                    power,
                    toughness,
                    scale: None,
                    subject: EffectSubject::Chosen(_),
                }] if *power == dp && *toughness == dt
            ),
            "{id}: {:?}",
            trigger.effect
        );
    }
}

#[test]
fn flash_etb_pump_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _mana, _types, _power, _toughness, _dp, _dt, fingerprint) in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id);
        assert_eq!(fields[4], fingerprint, "fingerprint drift for {id}");
    }
}
