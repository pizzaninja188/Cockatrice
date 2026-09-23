use tricerules_cards::primitives::{
    CastTriggerPlayer, EffectSubject, GameCondition, PermanentTypeFilter, SpellEffectKind,
    TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, CardRegistry};

#[test]
fn issue_479_ticket_tortoise_is_a_complete_conditioned_treasure_card() {
    let registry = CardRegistry::global();
    let definition = registry
        .get("ticket_tortoise")
        .expect("Ticket Tortoise is registered");
    let face = definition.primary_face();
    assert_eq!(definition.name, "Ticket Tortoise");
    assert_eq!(face.mana_cost.to_string(), "{2}");
    assert_eq!(face.types, ["Artifact", "Creature", "Turtle"]);
    assert_eq!((face.power, face.toughness), (Some(3), Some(1)));
    assert!(face.keywords.contains(&tricerules_cards::Keyword::Defender));

    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Ticket Tortoise has one triggered ability");
    };
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![2]),
        "the entry ability is Oracle line 2 after Defender"
    );
    assert_eq!(
        trigger.intervening_if,
        Some(GameCondition::OpponentControlsMoreLandsThanYou)
    );
    assert!(matches!(
        trigger.effect.as_slice(),
        [SpellEffectKind::CreateTokens { token, count, .. }]
            if token == "treasure" && *count == tricerules_cards::Amount::Fixed(1)
    ));
    assert!(registry.is_token("treasure"));
}

#[test]
fn issue_479_sunstar_expansionist_has_both_complete_entry_and_landfall_abilities() {
    let registry = CardRegistry::global();
    let definition = registry
        .get("sunstar_expansionist")
        .expect("Sunstar Expansionist is registered");
    let face = definition.primary_face();
    assert_eq!(definition.name, "Sunstar Expansionist");
    assert_eq!(face.mana_cost.to_string(), "{1}{W}");
    assert_eq!(face.types, ["Creature", "Human", "Knight"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(3)));

    let [entry, landfall] = face.triggered_abilities.as_slice() else {
        panic!("Sunstar Expansionist has the ETB and landfall abilities");
    };
    assert_eq!(entry.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        entry.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        entry.intervening_if,
        Some(GameCondition::OpponentControlsMoreLandsThanYou)
    );
    assert!(matches!(
        entry.effect.as_slice(),
        [SpellEffectKind::CreateTokens { token, count, .. }]
            if token == "lander" && *count == tricerules_cards::Amount::Fixed(1)
    ));
    assert!(matches!(
        &landfall.trigger,
        TriggerCondition::WheneverPermanentEntersBattlefield {
            controller: CastTriggerPlayer::Controller,
            filter,
            creature_filter: None,
        } if filter.permanent_type == Some(PermanentTypeFilter::Land)
    ));
    assert!(matches!(
        landfall.effect.as_slice(),
        [SpellEffectKind::PumpTarget {
            power: 1,
            toughness: 0,
            subject: EffectSubject::Source,
            ..
        }]
    ));
    assert_eq!(
        landfall.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(registry.is_token("lander"));
}

#[test]
fn issue_479_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, hash) in [
        (
            "ticket_tortoise",
            "Ticket Tortoise",
            "973b4a0b23f052c20383159f96f218abdbd54ce791675a50d30aa9205ae1ca1d",
        ),
        (
            "sunstar_expansionist",
            "Sunstar Expansionist",
            "46dcf9578ce1c3b485344a2f7ae9d0ccdb567b219f8c8093f37e22546560fb2e",
        ),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing Oracle fingerprint for {id}"));
        let fields: Vec<_> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[4], hash, "pinned Oracle fingerprint for {id}");
    }
}
