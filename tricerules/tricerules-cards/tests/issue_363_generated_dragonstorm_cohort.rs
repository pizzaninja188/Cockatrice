//! Registry and presentation conformance for issue #363.
//!
//! The four generated Standard Dragonstorm enchantments must register with their complete typed
//! payloads: CR 603.6a/119.3/701.25 Corroding's each-opponent drain plus surveil; CR
//! 603.6a/701.23/614.1d Encroaching's up-to-two basic-land search onto the battlefield tapped; CR
//! 603.6a/121.1/701.9 Roiling's draw-two-then-discard; CR 603.6a/111.1 Teeming's two 2/2 white
//! Soldier tokens; and CR 603.6a/400.3 the shared return-self trigger on a controlled Dragon.

use tricerules_cards::primitives::{
    CardTypeFilter, DrawDiscardOrder, EffectSubject, LifeAmount, PermanentEventFilter,
    PlayerRecipient, SearchDestination, SearchZoneSelection, ZoneCardFilter,
};
use tricerules_cards::{
    AbilityPresentation, Amount, CardRegistry, CastTriggerPlayer, Color, LibraryPartitionKind,
    SpellEffectKind, TriggerCondition,
};

fn dragon_return_trigger() -> TriggerCondition {
    TriggerCondition::WheneverPermanentEntersBattlefield {
        controller: CastTriggerPlayer::Controller,
        filter: PermanentEventFilter {
            required_subtypes: vec!["Dragon".into()],
            ..PermanentEventFilter::default()
        },
        creature_filter: None,
    }
}

fn assert_shared_dragon_return(face: tricerules_cards::FaceRef<'_>) {
    let abilities = face.triggered_abilities.as_slice();
    assert_eq!(
        abilities.len(),
        2,
        "each Dragonstorm card prints two triggers"
    );
    let return_ability = &abilities[1];
    assert_eq!(
        return_ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(return_ability.trigger, dragon_return_trigger());
    assert_eq!(
        return_ability.effect,
        [SpellEffectKind::ReturnToOwnersHand {
            subject: EffectSubject::Source,
        }]
    );
    assert!(return_ability.targeting.is_none());
    assert!(!return_ability.may);
}

#[test]
fn issue_363_registers_the_four_reviewed_identities() {
    let registry = CardRegistry::global();
    for (id, name, face_id) in [
        (
            "corroding_dragonstorm",
            "Corroding Dragonstorm",
            "corroding_dragonstorm",
        ),
        (
            "encroaching_dragonstorm",
            "Encroaching Dragonstorm",
            "encroaching_dragonstorm",
        ),
        (
            "roiling_dragonstorm",
            "Roiling Dragonstorm",
            "roiling_dragonstorm",
        ),
        (
            "teeming_dragonstorm",
            "Teeming Dragonstorm",
            "teeming_dragonstorm",
        ),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing reviewed card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        assert_eq!(definition.layout, tricerules_cards::Layout::Normal);
        assert_eq!(definition.face_count(), 1);
        assert_eq!(definition.primary_face().face_id.as_str(), face_id);
    }
}

#[test]
fn issue_363_corroding_dragonstorm_drains_each_opponent_then_surveils() {
    let definition = CardRegistry::global()
        .get("corroding_dragonstorm")
        .expect("Corroding Dragonstorm");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{B}");
    assert_eq!(face.types, ["Enchantment"]);
    assert_eq!(face.colors(), vec![Color::Black]);
    let abilities = face.triggered_abilities.as_slice();
    let etb = &abilities[0];
    assert_eq!(etb.presentation, AbilityPresentation::OracleLines(vec![1]));
    assert_eq!(etb.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        etb.effect,
        [
            SpellEffectKind::LoseLife {
                amount: LifeAmount::Fixed(2),
                who: PlayerRecipient::EachOpponent,
            },
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(2),
            },
            SpellEffectKind::LibraryPartition {
                count: 2,
                top_min: 0,
                top_max: None,
                kind: LibraryPartitionKind::Surveil,
            },
        ]
    );
    assert!(etb.targeting.is_none());
    assert_shared_dragon_return(face);
}

#[test]
fn issue_363_encroaching_dragonstorm_searches_up_to_two_basic_lands_tapped() {
    let definition = CardRegistry::global()
        .get("encroaching_dragonstorm")
        .expect("Encroaching Dragonstorm");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}{G}");
    assert_eq!(face.types, ["Enchantment"]);
    assert_eq!(face.colors(), vec![Color::Green]);
    let abilities = face.triggered_abilities.as_slice();
    let etb = &abilities[0];
    assert_eq!(etb.presentation, AbilityPresentation::OracleLines(vec![1]));
    assert_eq!(etb.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        etb.effect,
        [SpellEffectKind::SearchLibrary {
            who: PlayerRecipient::Controller,
            optional: false,
            count: 2,
            count_by_cast_cost: None,
            filter: Some(ZoneCardFilter {
                card_type: Some(CardTypeFilter::BasicLand),
                ..ZoneCardFilter::default()
            }),
            slots: Vec::new(),
            zones: SearchZoneSelection::default(),
            destination: SearchDestination::Battlefield { tapped: true },
            conditional_destination: None,
            shuffle: true,
            reveal: false,
            result_id: None,
        }]
    );
    assert!(etb.targeting.is_none());
    assert_shared_dragon_return(face);
}

#[test]
fn issue_363_roiling_dragonstorm_draws_two_then_discards_one() {
    let definition = CardRegistry::global()
        .get("roiling_dragonstorm")
        .expect("Roiling Dragonstorm");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{U}");
    assert_eq!(face.types, ["Enchantment"]);
    assert_eq!(face.colors(), vec![Color::Blue]);
    let abilities = face.triggered_abilities.as_slice();
    let etb = &abilities[0];
    assert_eq!(etb.presentation, AbilityPresentation::OracleLines(vec![1]));
    assert_eq!(etb.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        etb.effect,
        [SpellEffectKind::DrawDiscard {
            who: PlayerRecipient::Controller,
            draw_count: 2,
            discard_count: 1,
            order: DrawDiscardOrder::DrawThenDiscard,
            optional: false,
        }]
    );
    assert!(etb.targeting.is_none());
    assert_shared_dragon_return(face);
}

#[test]
fn issue_363_teeming_dragonstorm_creates_two_white_soldiers() {
    let definition = CardRegistry::global()
        .get("teeming_dragonstorm")
        .expect("Teeming Dragonstorm");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}{W}");
    assert_eq!(face.types, ["Enchantment"]);
    assert_eq!(face.colors(), vec![Color::White]);
    let abilities = face.triggered_abilities.as_slice();
    let etb = &abilities[0];
    assert_eq!(etb.presentation, AbilityPresentation::OracleLines(vec![1]));
    assert_eq!(etb.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        etb.effect,
        [SpellEffectKind::CreateTokens {
            token: "soldier_w_2_2".into(),
            count: Amount::Fixed(2),
            who: PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        }]
    );
    assert_shared_dragon_return(face);
}

#[test]
fn issue_363_fingerprint_rows_match_the_presentation_registry() {
    let registry = CardRegistry::global();
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, face_id) in [
        ("corroding_dragonstorm", "corroding_dragonstorm"),
        ("encroaching_dragonstorm", "encroaching_dragonstorm"),
        ("roiling_dragonstorm", "roiling_dragonstorm"),
        ("teeming_dragonstorm", "teeming_dragonstorm"),
    ] {
        let presentation = registry
            .presentation_face(id, face_id)
            .unwrap_or_else(|| panic!("missing presentation metadata for {id}/{face_id}"));
        assert_eq!(presentation.oracle_text_sha256.len(), 64);
        let row = fingerprints
            .lines()
            .find(|line| {
                line.starts_with(&format!("{id}\t")) && line.contains(&format!("\t{face_id}\t"))
            })
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}/{face_id}"));
        assert!(
            row.ends_with(&presentation.oracle_text_sha256),
            "fingerprint drift for {id}/{face_id}: {row}"
        );
    }
}
