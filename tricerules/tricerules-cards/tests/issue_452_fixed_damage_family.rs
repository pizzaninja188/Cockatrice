//! Registry and presentation conformance for the issue #452 reviewed fixed-damage family cohort.
//!
//! The seven identities each print exactly `<name> deals N damage to target creature.` in the
//! pinned Scryfall snapshot `27bf3214-1271-490b-bdfe-c0be6c23d02e`; Scryfall exact-name records
//! and rulings were fetched 2026-09-19. CR 120.2b keeps the resolving spell as the damage source
//! and CR 115.1/608.2b govern the creature-only target and its revalidation.

use tricerules_cards::primitives::{Amount, SpellEffectKind, TargetFilter};
use tricerules_cards::{CardRegistry, Color, Layout};

#[derive(Clone, Copy)]
struct CohortFixture {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    card_type: &'static str,
    amount: u32,
    fingerprint: &'static str,
}

const COHORT: [CohortFixture; 7] = [
    CohortFixture {
        id: "ragefire",
        name: "Ragefire",
        mana: "{1}{R}",
        card_type: "Sorcery",
        amount: 3,
        fingerprint: "b3f58f8080c7de284add00f9b8e107396436510f2d78c1102cc2b6f37769035a",
    },
    CohortFixture {
        id: "repulsor_rays",
        name: "Repulsor Rays",
        mana: "{R}",
        card_type: "Sorcery",
        amount: 3,
        fingerprint: "fadfba9de2aeac4200793fb40af6c9e97accf237664d2456e2ee375f2b2aa075",
    },
    CohortFixture {
        id: "scorching_shot",
        name: "Scorching Shot",
        mana: "{R}{R}",
        card_type: "Sorcery",
        amount: 5,
        fingerprint: "7378a4dc9fa9b906b52beeed1eabb75e2a4947c3ede07c00730c53667efc6327",
    },
    CohortFixture {
        id: "command_the_storm",
        name: "Command the Storm",
        mana: "{4}{R}",
        card_type: "Instant",
        amount: 5,
        fingerprint: "6c8d2a2e44346e2d7fb8edc7f077f5429d77a8600be17b62b06e3fe2b2a22386",
    },
    CohortFixture {
        id: "concentrated_fire",
        name: "Concentrated Fire",
        mana: "{3}{R}",
        card_type: "Instant",
        amount: 5,
        fingerprint: "e0b6c4a78e0b14a690b3811a2ef0373554178f2d895d9aa0e734da24057ef432",
    },
    CohortFixture {
        id: "direct_hit",
        name: "Direct Hit",
        mana: "{2}{R}",
        card_type: "Sorcery",
        amount: 5,
        fingerprint: "eafe634efbdddc5f364d2251bb5ac2a64a7f9ac3ac8968ca8e69c0e762088fe2",
    },
    CohortFixture {
        id: "engulfing_eruption",
        name: "Engulfing Eruption",
        mana: "{2}{R}{R}",
        card_type: "Sorcery",
        amount: 5,
        fingerprint: "d60508519919872e82b6993ac5d5a5da7aa6c8b5477cf8cd6c52ed6d8441106f",
    },
];

#[test]
fn issue_452_family_registers_exactly_the_reviewed_cohort() {
    let registry = CardRegistry::global();
    for card in COHORT {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing reviewed card {}", card.id));
        assert_eq!(definition.name, card.name);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        assert_eq!(definition.layout, Layout::Normal);
        assert_eq!(definition.face_count(), 1);
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), card.id);
        assert_eq!(face.mana_cost.to_string(), card.mana);
        assert_eq!(face.types, [card.card_type]);
        assert_eq!(face.colors(), vec![Color::Red]);
        assert!(face.keywords.is_empty());
        assert!(face.triggered_abilities.is_empty());
        assert!(face.activated_abilities.is_empty());
        assert!(face.static_abilities.is_empty());
        assert!(face.custom_effect.is_none());
        assert!(face.modal_spell.is_none());
        assert!(
            face.targeting.is_none(),
            "{} targets inline through DamageTarget, not a grouped targeting definition",
            card.id
        );
        assert_eq!(
            face.spell_effect,
            [SpellEffectKind::DamageTarget {
                amount: Amount::Fixed(card.amount),
                target: TargetFilter::default_creature(),
            }],
            "{} exact typed damage",
            card.id
        );
    }
}

#[test]
fn issue_452_family_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in COHORT {
        let prefix = format!("{}\t", card.id);
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&prefix))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        assert!(
            row.contains(card.name),
            "fingerprint row names the reviewed card: {row}"
        );
        assert!(
            row.ends_with(card.fingerprint),
            "fingerprint drift for {}: {row}",
            card.id
        );
    }
}
