//! Registry conformance for the queue-v30 free sacrifice outlets.
use tricerules_cards::primitives::{
    AbilityCost, Amount, DiscardQuantity, InstructionKind, SpellEffectKind,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    effect: Effect,
    fingerprint: &'static str,
}

enum Effect {
    Mana(u32, u32),
    Draw,
    Loot,
    Fog,
}

const CARDS: &[Card] = &[
    Card {
        id: "blood_pet",
        name: "Blood Pet",
        mana: "{B}",
        types: &["Creature", "Thrull"],
        power: 1,
        toughness: 1,
        effect: Effect::Mana(0, 1),
        fingerprint: "ba3965d7647a334077f12f6ffe26c92f26c32fa14c3dcaa65496628b229a6f83",
    },
    Card {
        id: "blood_vassal",
        name: "Blood Vassal",
        mana: "{2}{B}",
        types: &["Creature", "Thrull"],
        power: 2,
        toughness: 2,
        effect: Effect::Mana(0, 2),
        fingerprint: "c85597eabb24bfc2c38dbbb24fb1ba8e4c41d311b4f541698f5359bda8c62dd2",
    },
    Card {
        id: "catalyst_elemental",
        name: "Catalyst Elemental",
        mana: "{2}{R}",
        types: &["Creature", "Elemental"],
        power: 2,
        toughness: 2,
        effect: Effect::Mana(2, 0),
        fingerprint: "14d83ece8a22f05176be354cde914c35056a3d862eefad439937d23360038d3f",
    },
    Card {
        id: "reckless_barbarian",
        name: "Reckless Barbarian",
        mana: "{1}{R}",
        types: &["Creature", "Dragon", "Barbarian"],
        power: 2,
        toughness: 2,
        effect: Effect::Mana(2, 0),
        fingerprint: "14d83ece8a22f05176be354cde914c35056a3d862eefad439937d23360038d3f",
    },
    Card {
        id: "alchemists_apprentice",
        name: "Alchemist's Apprentice",
        mana: "{1}{U}",
        types: &["Creature", "Human", "Wizard"],
        power: 1,
        toughness: 1,
        effect: Effect::Draw,
        fingerprint: "cea08b0ec81b829d4cf62b6b7151fe1c50307ceb6fbd49319a0e650122872801",
    },
    Card {
        id: "hapless_researcher",
        name: "Hapless Researcher",
        mana: "{U}",
        types: &["Creature", "Human", "Wizard"],
        power: 1,
        toughness: 1,
        effect: Effect::Loot,
        fingerprint: "0a42735e801bb53c573836b45a63a51272a270f8a321ad0359fb9c943b851da4",
    },
    Card {
        id: "kami_of_false_hope",
        name: "Kami of False Hope",
        mana: "{W}",
        types: &["Creature", "Spirit"],
        power: 1,
        toughness: 1,
        effect: Effect::Fog,
        fingerprint: "ce21ca5c014d3c8dc9b443be3e8978ec846fd47915f30ced16174df6da9b9c6b",
    },
    Card {
        id: "spore_frog",
        name: "Spore Frog",
        mana: "{G}",
        types: &["Creature", "Frog"],
        power: 1,
        toughness: 1,
        effect: Effect::Fog,
        fingerprint: "ce21ca5c014d3c8dc9b443be3e8978ec846fd47915f30ced16174df6da9b9c6b",
    },
];

#[test]
fn q30_free_sac_outlets_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        let f = definition.primary_face();
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        assert_eq!(f.face_id.as_str(), card.id, "{}", card.id);
        assert_eq!(f.name, card.name, "{}", card.id);
        assert_eq!(f.mana_cost.to_string(), card.mana, "{}", card.id);
        assert_eq!(f.types, card.types, "{}", card.id);
        assert_eq!(f.power, Some(card.power), "{}", card.id);
        assert_eq!(f.toughness, Some(card.toughness), "{}", card.id);
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.protections.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", card.id);
        };
        assert_eq!(ability.costs, [AbilityCost::SacrificeSelf], "{}", card.id);
        match card.effect {
            Effect::Mana(red, black) => {
                let [SpellEffectKind::ProduceMana { options, .. }] = ability.effect.as_slice()
                else {
                    panic!("{}: mana effect", card.id);
                };
                assert_eq!(options.len(), 1, "{}", card.id);
                assert_eq!(options[0].r, red, "{}", card.id);
                assert_eq!(options[0].b, black, "{}", card.id);
                assert_eq!(
                    options[0].w + options[0].u + options[0].g + options[0].c,
                    0,
                    "{}",
                    card.id
                );
            }
            Effect::Draw => {
                let [SpellEffectKind::Draw { count, .. }] = ability.effect.as_slice() else {
                    panic!("{}: draw effect", card.id);
                };
                assert_eq!(*count, Amount::Fixed(1), "{}", card.id);
            }
            Effect::Loot => {
                assert!(ability.effect.is_empty(), "{}: program effect", card.id);
                let program = ability.program.as_ref().expect("draw/discard program");
                let [draw, discard] = program.body.steps.as_slice() else {
                    panic!("{}: exactly two program steps", card.id);
                };
                assert_eq!(draw.id, "draw_one", "{}", card.id);
                assert!(
                    matches!(
                        &draw.kind,
                        InstructionKind::Perform(op)
                            if matches!(**op, SpellEffectKind::Draw { count: Amount::Fixed(1), .. })
                    ),
                    "{}: draw one",
                    card.id
                );
                assert_eq!(discard.id, "discard_one", "{}", card.id);
                assert!(
                    matches!(
                        &discard.kind,
                        InstructionKind::Perform(op)
                            if matches!(
                                **op,
                                SpellEffectKind::Discard {
                                    quantity: DiscardQuantity::Exact(1),
                                    ..
                                }
                            )
                    ),
                    "{}: discard one",
                    card.id
                );
            }
            Effect::Fog => {
                assert!(
                    matches!(
                        ability.effect.as_slice(),
                        [SpellEffectKind::PreventAllCombatDamageTurn]
                    ),
                    "{}: fog effect",
                    card.id
                );
            }
        }
    }
}

#[test]
fn q30_free_sac_outlets_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(fields[2], card.id.replace('-', "_"));
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
