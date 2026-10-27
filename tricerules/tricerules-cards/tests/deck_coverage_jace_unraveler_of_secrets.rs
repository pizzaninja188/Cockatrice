//! Exact Jace identity and the first two loyalty abilities.
use tricerules_cards::primitives::{
    AbilityCost, Amount, EffectSubject, PlayerRecipient, SpellEffectKind, TargetKind,
};
use tricerules_cards::{AbilityPresentation, InstructionKind};

#[test]
fn jace_exact_identity_and_first_two_loyalty_abilities() {
    let registry = tricerules_cards::registry::global();
    let id = registry
        .id_for_name("Jace, Unraveler of Secrets")
        .expect("exact Jace, Unraveler of Secrets must be registered");
    assert_eq!(id, "jace,_unraveler_of_secrets");
    let card = registry.get(id).unwrap();
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.name, "Jace, Unraveler of Secrets");
    assert_eq!(face.face_id.as_str(), "jace_unraveler_of_secrets");
    assert_eq!(face.mana_cost.to_string(), "{3}{U}{U}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Planeswalker", "Jace"]);
    assert_eq!(face.loyalty, Some(5));
    assert!(face.activated_abilities.len() >= 2);

    let plus = &face.activated_abilities[0];
    assert_eq!(plus.ability_id.as_str(), "activated_01");
    assert_eq!(plus.presentation, AbilityPresentation::OracleLines(vec![1]));
    assert_eq!(plus.costs, [AbilityCost::Loyalty(1)]);
    let program = plus
        .program
        .as_ref()
        .expect("Scry followed by Draw uses a Program");
    let [scry, draw] = program.body.steps.as_slice() else {
        panic!("+1 scries once, then draws once");
    };
    assert!(matches!(&scry.kind, InstructionKind::Perform(effect)
        if matches!(effect.as_ref(), SpellEffectKind::Scry { count: Amount::Fixed(1) })));
    assert!(matches!(&draw.kind, InstructionKind::Perform(effect)
    if matches!(effect.as_ref(), SpellEffectKind::Draw {
        who: PlayerRecipient::Controller, count: Amount::Fixed(1),
    })));

    let minus = &face.activated_abilities[1];
    assert_eq!(minus.ability_id.as_str(), "activated_02");
    assert_eq!(
        minus.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(minus.costs, [AbilityCost::Loyalty(-2)]);
    assert!(
        matches!(minus.effect.as_slice(), [SpellEffectKind::ReturnToOwnersHand {
        subject: EffectSubject::Chosen(filter),
    }] if filter.kind == TargetKind::Creature)
    );
}

#[test]
fn jace_ultimate_has_the_exact_opponent_first_cast_emblem() {
    use tricerules_cards::primitives::{
        CastOrdinalScope, CastTriggerPlayer, SpellCastFilter, TriggerCondition,
    };
    let card = tricerules_cards::registry::global()
        .get("jace,_unraveler_of_secrets")
        .unwrap();
    let ultimate = card
        .primary_face()
        .activated_abilities
        .get(2)
        .expect("Jace has a -8 ability");
    assert_eq!(ultimate.ability_id.as_str(), "activated_03");
    assert_eq!(
        ultimate.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(ultimate.costs, [AbilityCost::Loyalty(-8)]);
    let [SpellEffectKind::CreateTriggeredEmblem { ability, .. }] = ultimate.effect.as_slice()
    else {
        panic!("-8 creates the triggered emblem");
    };
    assert_eq!(ability.ability_id.as_str(), "emblem_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(
        ability.trigger,
        TriggerCondition::WheneverPlayerCastsSpell {
            caster: CastTriggerPlayer::Opponent,
            filter: SpellCastFilter::default(),
            ordinal: Some(1),
            ordinal_scope: CastOrdinalScope::AllSpells,
        }
    );
    assert!(ability.targeting.is_none() && !ability.may && ability.intervening_if.is_none());
    assert!(!ability.triggers_only_once && ability.max_triggers_per_turn.is_none());
    assert_eq!(ability.effect.len(), 1);
    assert_eq!(format!("{:?}", ability.effect[0]), "CounterTriggeringSpell");
}

const CAST_EMBLEM_FIXTURE: &str = r#"(
    id: "jace_emblem_profile_fixture", name: "Jace Emblem Profile Fixture",
    face_id: "jace_emblem_profile_fixture", types: ["Artifact"],
    activated_abilities: [(
        ability_id: "activated_01", presentation: Fallback, costs: [Tap],
        effect: [CreateTriggeredEmblem(
            emblem_id: "profile_emblem", display_name: "Profile Emblem",
            ability: (
                ability_id: "emblem_01", presentation: Fallback,
                trigger: WheneverPlayerCastsSpell(caster: Opponent, filter: (), ordinal: Some(1), ordinal_scope: AllSpells),
                effect: [CounterTriggeringSpell],
            ),
        )],
    )],
)"#;

const ORDINARY_CAST_TRIGGER_FIXTURE: &str = r#"(
    id: "ordinary_counter_trigger_fixture", name: "Ordinary Counter Trigger Fixture",
    face_id: "ordinary_counter_trigger_fixture", types: ["Enchantment"],
    triggered_abilities: [(
        ability_id: "triggered_01", presentation: Fallback,
        trigger: WheneverPlayerCastsSpell(caster: Opponent, filter: (card_type: Some(Creature))),
        effect: [CounterTriggeringSpell],
    )],
)"#;

fn reject_after_parsing(label: &str, definition: &str) {
    match tricerules_cards::CardRegistry::from_chunks_and_tokens(&[definition], &[]) {
        Ok(_) => panic!("{label} unexpectedly admitted CounterTriggeringSpell"),
        Err(error) => assert!(
            !error.to_string().contains("ron parse:"),
            "{label} must reach model validation rather than fail parsing: {error}"
        ),
    }
}

#[test]
fn counter_triggering_spell_admits_exact_emblem_and_ordinary_cast_trigger() {
    for (label, definition) in [
        ("exact Jace emblem child", CAST_EMBLEM_FIXTURE),
        ("ordinary flat cast trigger", ORDINARY_CAST_TRIGGER_FIXTURE),
    ] {
        tricerules_cards::CardRegistry::from_chunks_and_tokens(&[definition], &[])
            .unwrap_or_else(|error| panic!("{label} must be admitted: {error}"));
    }
}

#[test]
fn counter_triggering_spell_rejects_wrong_emblem_shapes_and_contexts() {
    // Establish the fixture parses and is admitted before checking semantic rejection.
    tricerules_cards::CardRegistry::from_chunks_and_tokens(&[CAST_EMBLEM_FIXTURE], &[])
        .expect("exact emblem profile is admitted before negative mutations");
    for (label, from, to) in [
        ("controller cast", "caster: Opponent", "caster: Controller"),
        ("filtered spell", "filter: ()", "filter: (card_type: Some(Creature))"),
        ("second spell", "ordinal: Some(1)", "ordinal: Some(2)"),
        ("matching-filter ordinal", "ordinal_scope: AllSpells", "ordinal_scope: MatchingFilter"),
        ("optional emblem trigger", "trigger: WheneverPlayerCastsSpell", "may: true, trigger: WheneverPlayerCastsSpell"),
        ("once-only emblem trigger", "trigger: WheneverPlayerCastsSpell", "triggers_only_once: true, trigger: WheneverPlayerCastsSpell"),
        ("per-turn emblem trigger cap", "trigger: WheneverPlayerCastsSpell", "max_triggers_per_turn: Some(1), trigger: WheneverPlayerCastsSpell"),
        ("intervening condition", "trigger: WheneverPlayerCastsSpell", "intervening_if: Some(ControlsCreatureTiedForGreatestPower), trigger: WheneverPlayerCastsSpell"),
        ("targeted emblem child", "trigger: WheneverPlayerCastsSpell", "targeting: Some((groups: [(min: 1, max: 1, prompt: \"Choose target\", effect_indices: [])])), trigger: WheneverPlayerCastsSpell"),
        ("unowned child Program", "ability_id: \"emblem_01\",", "ability_id: \"emblem_01\", program: Some((body: (steps: [(id: \"draw\", kind: Perform(Draw(count: 1)))]))),"),
        ("wrapped counter", "effect: [CounterTriggeringSpell]", "effect: [Conditional(condition: ControlsCreatureTiedForGreatestPower, effect: CounterTriggeringSpell)]"),
    ] {
        let invalid = CAST_EMBLEM_FIXTURE.replacen(from, to, 1);
        assert_ne!(invalid, CAST_EMBLEM_FIXTURE, "{label} mutation applies");
        reject_after_parsing(label, &invalid);
    }
    for (label, definition) in [
        (
            "spell",
            r#"(id: "counter_spell_fixture", name: "Counter Spell Fixture", face_id: "counter_spell_fixture", types: ["Instant"], spell_effect: [CounterTriggeringSpell])"#,
        ),
        (
            "activated ability",
            r#"(id: "counter_active_fixture", name: "Counter Active Fixture", face_id: "counter_active_fixture", types: ["Artifact"], activated_abilities: [(ability_id: "activated_01", presentation: Fallback, costs: [Tap], effect: [CounterTriggeringSpell])])"#,
        ),
        (
            "unrelated trigger",
            r#"(id: "counter_etb_fixture", name: "Counter ETB Fixture", face_id: "counter_etb_fixture", types: ["Creature"], power: 1, toughness: 1, triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback, trigger: WhenSelfEntersBattlefield, effect: [CounterTriggeringSpell])])"#,
        ),
        (
            "spell Program",
            r#"(id: "counter_program_fixture", name: "Counter Program Fixture", face_id: "counter_program_fixture", types: ["Instant"], spell_program: Some((body: (steps: [(id: "counter", kind: Perform(CounterTriggeringSpell))]))))"#,
        ),
        (
            "cast-triggered Program",
            r#"(id: "counter_trigger_program_fixture", name: "Counter Trigger Program Fixture", face_id: "counter_trigger_program_fixture", types: ["Enchantment"], triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback, trigger: WheneverPlayerCastsSpell(caster: Opponent), effect: [], program: Some((body: (steps: [(id: "counter", kind: Perform(CounterTriggeringSpell))]))))])"#,
        ),
        (
            "delayed child",
            r#"(id: "counter_delay_fixture", name: "Counter Delay Fixture", face_id: "counter_delay_fixture", types: ["Enchantment"], triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback, trigger: WheneverPlayerCastsSpell(caster: Opponent), effect: [CreateDelayedTrigger(ability: (ability_id: "delayed_01", presentation: Fallback, trigger: AtBeginningOfNextEndStep, effect: [CounterTriggeringSpell]))])])"#,
        ),
    ] {
        reject_after_parsing(label, definition);
    }
}
