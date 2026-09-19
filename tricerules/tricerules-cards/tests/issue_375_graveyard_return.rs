//! Issue #375 registry conformance for the four retained graveyard-return identities.
//!
//! Each card is generated from one exact typed recipe: `Descend 4`/`Threshold`/`Descend 8`
//! public graveyard gates use `GameCondition::GraveyardAggregate` (CR 404.2, 603.4), graveyard
//! recursion uses `MoveGraveyardCards`/`ReturnToOwnersHand` (CR 400.7 new-object identity), and
//! mobilize reuses `CreateAttackingTokens` scaled by `CountExpression::GraveyardCards`
//! (CR 404.2, 508.4, 702.181). The eleven identities whose printed clauses need absent engine
//! behavior stay unretained and are asserted absent.

use tricerules_cards::primitives::{
    CardTypeFilter, CountExpression, DelayedTokenSacrificeTiming, EffectSubject,
    GraveyardAggregate, GraveyardDestination, GraveyardFilter, GraveyardOwner, PermanentTypeFilter,
    RelativePlayerSet, TargetKind, TargetObjectExclusion, ZoneCardFilter,
};
use tricerules_cards::{
    Amount, CardRegistry, GameCondition, Keyword, SpellEffectKind, TriggerCondition,
};

fn permanent_card_filter() -> ZoneCardFilter {
    ZoneCardFilter {
        excluded_card_types: vec![CardTypeFilter::Instant, CardTypeFilter::Sorcery],
        ..ZoneCardFilter::default()
    }
}

fn graveyard_gate(
    aggregate: GraveyardAggregate,
    filter: Option<ZoneCardFilter>,
    min: u32,
) -> GameCondition {
    GameCondition::GraveyardAggregate {
        owners: RelativePlayerSet::Controller,
        aggregate,
        filter,
        min: Some(min),
        max: None,
    }
}

#[test]
fn issue_375_registers_the_four_completed_identities() {
    let registry = CardRegistry::global();
    for (id, name, face_id, mana_cost, types, stats, keywords) in [
        (
            "avenger_of_the_fallen",
            "Avenger of the Fallen",
            "avenger_of_the_fallen",
            "{2}{B}",
            vec!["Creature", "Human", "Warrior"],
            (2, 4),
            vec![Keyword::Deathtouch],
        ),
        (
            "coati_scavenger",
            "Coati Scavenger",
            "coati_scavenger",
            "{2}{G}",
            vec!["Creature", "Raccoon"],
            (3, 2),
            vec![],
        ),
        (
            "council_of_echoes",
            "Council of Echoes",
            "council_of_echoes",
            "{4}{U}{U}",
            vec!["Creature", "Spirit", "Advisor"],
            (4, 4),
            vec![Keyword::Flying],
        ),
        (
            "tidecaller_mentor",
            "Tidecaller Mentor",
            "tidecaller_mentor",
            "{1}{U}{B}",
            vec!["Creature", "Rat", "Wizard"],
            (3, 3),
            vec![Keyword::Menace],
        ),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing retained card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), face_id);
        assert_eq!(face.mana_cost.to_string(), mana_cost);
        assert_eq!(face.types, types);
        assert_eq!(face.power.zip(face.toughness), Some(stats));
        assert_eq!(face.keywords, keywords);
    }
}

#[test]
fn issue_375_excludes_the_eleven_blocked_identities() {
    let registry = CardRegistry::global();
    for name in [
        "Brilliance Unleashed",
        "Emet-Selch, Unsundered // Hades, Sorcerer of Eld",
        "Persistent Marshstalker",
        "Kaya, Spirits' Justice",
        "Likeness Looter",
        "Matzalantli, the Great Door // The Core",
        "Night Nurse, Healer of Heroes",
        "Ran and Shaw",
        "Squirming Emergence",
        "The Everflowing Well // The Myriad Pools",
        "Too Evil to Stay Dead",
    ] {
        assert_eq!(
            registry.id_for_name(name),
            None,
            "{name} has a blocked clause and must stay unretained"
        );
    }
}

#[test]
fn issue_375_coati_payload_is_exact() {
    let face = CardRegistry::global()
        .get("coati_scavenger")
        .expect("Coati Scavenger")
        .primary_face();
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Coati must have exactly one triggered ability");
    };
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(!ability.may);
    assert_eq!(
        ability.intervening_if,
        Some(graveyard_gate(
            GraveyardAggregate::CardCount,
            Some(permanent_card_filter()),
            4,
        ))
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::MoveGraveyardCards {
            filter: GraveyardFilter {
                excluded_objects: Vec::new(),
                owner: GraveyardOwner::Controller,
                card: Some(permanent_card_filter()),
            },
            destination: GraveyardDestination::Hand,
            linked_exile_id: None,
        }]
    );
    let targeting = ability.targeting.as_ref().expect("one target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("Coati must have exactly one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(
        group.prompt,
        "Choose target permanent card from your graveyard"
    );
    assert_eq!(group.effect_indices, [0]);
}

#[test]
fn issue_375_bounce_payloads_are_exact() {
    let registry = CardRegistry::global();

    let council = registry
        .get("council_of_echoes")
        .expect("Council of Echoes")
        .primary_face();
    let [council_trigger] = council.triggered_abilities.as_slice() else {
        panic!("Council must have exactly one triggered ability");
    };
    assert_eq!(
        council_trigger.intervening_if,
        Some(graveyard_gate(
            GraveyardAggregate::CardCount,
            Some(permanent_card_filter()),
            4,
        ))
    );
    let [SpellEffectKind::ReturnToOwnersHand {
        subject: EffectSubject::Chosen(target),
    }] = council_trigger.effect.as_slice()
    else {
        panic!("unexpected Council payload: {:?}", council_trigger.effect);
    };
    assert_eq!(target.kind, TargetKind::AnyPermanent);
    assert_eq!(target.excluded_permanent_types, [PermanentTypeFilter::Land]);
    assert_eq!(
        target.excluded_objects,
        [TargetObjectExclusion::Source],
        "the printed clause excludes this creature"
    );
    let council_targeting = council_trigger
        .targeting
        .as_ref()
        .expect("one target group");
    assert_eq!(
        (
            council_targeting.groups[0].min,
            council_targeting.groups[0].max
        ),
        (0, 1)
    );

    let tidecaller = registry
        .get("tidecaller_mentor")
        .expect("Tidecaller Mentor")
        .primary_face();
    let [threshold] = tidecaller.triggered_abilities.as_slice() else {
        panic!("Tidecaller must have exactly one triggered ability");
    };
    assert_eq!(
        threshold.intervening_if,
        Some(graveyard_gate(GraveyardAggregate::CardCount, None, 7)),
        "the printed threshold counts every card, not only permanents"
    );
    let [SpellEffectKind::ReturnToOwnersHand {
        subject: EffectSubject::Chosen(target),
    }] = threshold.effect.as_slice()
    else {
        panic!("unexpected Tidecaller payload: {:?}", threshold.effect);
    };
    assert_eq!(target.kind, TargetKind::AnyPermanent);
    assert_eq!(target.excluded_permanent_types, [PermanentTypeFilter::Land]);
    assert!(
        target.excluded_objects.is_empty(),
        "the printed clause has no source exclusion"
    );
    let tidecaller_targeting = threshold.targeting.as_ref().expect("one target group");
    assert_eq!(
        (
            tidecaller_targeting.groups[0].min,
            tidecaller_targeting.groups[0].max
        ),
        (0, 1)
    );
}

#[test]
fn issue_375_avenger_mobilize_payload_and_token_are_exact() {
    let registry = CardRegistry::global();
    let face = registry
        .get("avenger_of_the_fallen")
        .expect("Avenger of the Fallen")
        .primary_face();
    let [mobilize] = face.triggered_abilities.as_slice() else {
        panic!("Avenger must have exactly one triggered ability");
    };
    assert_eq!(
        mobilize.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0,
        }
    );
    assert_eq!(
        mobilize.effect,
        [SpellEffectKind::CreateAttackingTokens {
            token: "warrior_r_1_1".into(),
            count: Amount::Count(CountExpression::GraveyardCards {
                owners: RelativePlayerSet::Controller,
                filter: Some(ZoneCardFilter {
                    card_type: Some(CardTypeFilter::Creature),
                    ..ZoneCardFilter::default()
                }),
            }),
            sacrifice_timing: Some(DelayedTokenSacrificeTiming::NextEndStep),
        }]
    );

    assert!(registry.is_token("warrior_r_1_1"));
    let token = registry
        .get("warrior_r_1_1")
        .expect("mobilize Warrior token");
    assert_eq!(token.primary_face().name, "Warrior");
    assert_eq!(token.primary_face().types, ["Creature", "Warrior"]);
    assert_eq!(
        token.primary_face().colors_override,
        Some(vec![tricerules_cards::Color::Red])
    );
    assert_eq!(
        token
            .primary_face()
            .power
            .zip(token.primary_face().toughness),
        Some((1, 1))
    );
}

#[test]
fn issue_375_fingerprint_rows_match_the_presentation_registry() {
    let registry = CardRegistry::global();
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, face_id) in [
        ("avenger_of_the_fallen", "avenger_of_the_fallen"),
        ("coati_scavenger", "coati_scavenger"),
        ("council_of_echoes", "council_of_echoes"),
        ("tidecaller_mentor", "tidecaller_mentor"),
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
