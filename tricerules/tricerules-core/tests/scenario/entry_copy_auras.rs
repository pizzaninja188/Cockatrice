use crate::helpers::*;
use tricerules_cards::primitives::{SpellEffectKind, TargetController};
use tricerules_cards::{CardFace, CardRegistry};
use tricerules_core::{
    state::{AttachmentRecipient, CopiableValues},
    EngineDeck, GameEngine, Zone,
};
use tricerules_proto::ruled::v1::{ruled_event::Ev, ChoiceKind, StackResolveDestination};

const ENTRY_COPY_DRAFT: &str = r#"(
  id: "entry_copy_fixture",
  name: "Entry Copy Fixture",
  face_id: "entry_copy_fixture",
  mana_cost: "{0}",
  types: ["Creature", "Shapeshifter"],
  power: 2,
  toughness: 2,
  static_abilities: [(ability_id: "static_copy", presentation: Fallback,
    definition: EntersAsCopy(filter: (kind: AnyPermanent)))],
)"#;

const TOKEN_COPY_AURA_DRAFT: &str = r#"(
  id: "entry_copy_aura_token_spell",
  name: "Entry Copy Aura Token Spell",
  face_id: "entry_copy_aura_token_spell",
  mana_cost: "{2}{U}",
  types: ["Sorcery"],
  spell_effect: [CreateTokenCopies(count: 1, source: Chosen((kind: AnyPermanent)))],
  targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose target permanent", effect_indices: [0])])),
)"#;

const COPY_AURA_SOURCE_DRAFT: &str = r#"(
  id: "entry_copy_aura_source",
  name: "Entry Copy Aura Source",
  face_id: "entry_copy_aura_source",
  mana_cost: "{1}{W}",
  types: ["Enchantment", "Aura"],
  spell_effect: [AuraAttach(target: (kind: Creature, controller: You))],
)"#;

const ENTRY_COPY_AURA_DRAFT: &str = r#"(
  id: "entry_copy_aura_spell",
  name: "Entry Copy Aura Spell",
  face_id: "entry_copy_aura_spell",
  mana_cost: "{0}",
  types: ["Enchantment", "Aura"],
  spell_effect: [AuraAttach(target: (kind: Creature))],
  static_abilities: [(ability_id: "static_copy", presentation: Fallback,
    definition: EntersAsCopy(filter: (kind: AnyPermanent)))],
)"#;

const ENTRY_COPY_PLAIN_CREATURE_DRAFT: &str = r#"(
  id: "entry_copy_plain_creature",
  name: "Entry Copy Plain Creature",
  face_id: "entry_copy_plain_creature",
  mana_cost: "{0}",
  types: ["Creature"],
  power: 2,
  toughness: 2,
)"#;

fn copy_engine(seed: u64) -> GameEngine {
    let registry = Box::leak(Box::new(
        CardRegistry::from_authoring_draft(ENTRY_COPY_DRAFT).expect("entry-copy draft"),
    ));
    let decks = vec![
        EngineDeck {
            mainboard: vec!["entry_copy_fixture".into(); 20],
            commanders: Vec::new(),
        };
        2
    ];
    let mut engine = GameEngine::new_for_authoring(seed, &[0, 1], 20, Some(decks), true, registry)
        .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn token_copy_engine(seed: u64) -> GameEngine {
    let registry = Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(
            &[
                ENTRY_COPY_DRAFT,
                TOKEN_COPY_AURA_DRAFT,
                COPY_AURA_SOURCE_DRAFT,
            ],
            &[],
        )
        .expect("token-copy Aura fixture registry"),
    ));
    let decks = vec![
        EngineDeck {
            mainboard: vec!["entry_copy_aura_token_spell".into(); 20],
            commanders: Vec::new(),
        };
        2
    ];
    let mut engine = GameEngine::new_for_authoring(seed, &[0, 1], 20, Some(decks), true, registry)
        .expect("token-copy Aura engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn aura_spell_copy_engine(seed: u64) -> GameEngine {
    let registry = Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(
            &[
                ENTRY_COPY_DRAFT,
                ENTRY_COPY_AURA_DRAFT,
                ENTRY_COPY_PLAIN_CREATURE_DRAFT,
            ],
            &[],
        )
        .expect("Aura entry-copy fixture registry"),
    ));
    let decks = vec![
        EngineDeck {
            mainboard: vec!["entry_copy_aura_spell".into(); 20],
            commanders: Vec::new(),
        };
        2
    ];
    let mut engine = GameEngine::new_for_authoring(seed, &[0, 1], 20, Some(decks), true, registry)
        .expect("Aura entry-copy engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn source_copy_object(
    engine: &mut GameEngine,
    player: usize,
    mut face: CardFace,
    attached_to: AttachmentRecipient,
) -> u32 {
    if let Some(SpellEffectKind::AuraAttach { target }) = face.spell_effect.first_mut() {
        target.controller = TargetController::You;
    }
    let source_id = put_creature_on_battlefield(engine, player, "entry_copy_fixture");
    let values = CopiableValues {
        source_card_id: "entry_copy_source".into(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    };
    let source = engine.state.objects.get_mut(&source_id).unwrap();
    source.card_id = "entry_copy_source".into();
    source.copiable_values = Some(values);
    source.copy_revision = source.copy_revision.saturating_add(1);
    source.attached_to = Some(attached_to);
    source_id
}

fn cast_entry_copy(engine: &mut GameEngine) -> u32 {
    let copy = engine.state.players[0]
        .hand
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == "entry_copy_fixture")
        .expect("copy card in hand");
    let slot = hand_index_for_card(engine, 0, "entry_copy_fixture");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast entry-copy card");
    pass_both_players(engine);
    copy
}

#[test]
fn entry_copy_of_an_aura_asks_for_and_attaches_to_a_legal_permanent() {
    let mut engine = copy_engine(303_401);
    let host = put_creature_on_battlefield(&mut engine, 0, "entry_copy_fixture");
    let face_down_host = put_creature_on_battlefield(&mut engine, 0, "entry_copy_fixture");
    engine
        .state
        .objects
        .get_mut(&face_down_host)
        .unwrap()
        .face_down = true;
    let source_host = put_creature_on_battlefield(&mut engine, 1, "entry_copy_fixture");
    let aura = CardRegistry::global()
        .get("pacifism")
        .expect("Pacifism")
        .primary_face()
        .clone();
    let source = source_copy_object(
        &mut engine,
        1,
        aura,
        AttachmentRecipient::Object(source_host),
    );
    let copy = cast_entry_copy(&mut engine);
    let batch = engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .expect("choose Aura copy source");
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Aura choice");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::AuraPermanent);
    assert_eq!(pending.presentation.candidates, vec![face_down_host, host]);
    let aura_choice = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ResolutionChoiceRequired(choice))
                if choice.choice_kind == ChoiceKind::AuraPermanent as i32 =>
            {
                Some(choice)
            }
            _ => None,
        })
        .expect("Aura recipient choice event");
    let face_down_index = aura_choice
        .candidate_object_ids
        .iter()
        .position(|object_id| *object_id == face_down_host)
        .expect("face-down creature remains a legal Aura recipient");
    assert_eq!(aura_choice.candidate_card_ids[face_down_index], "");
    assert_eq!(
        aura_choice.candidate_names[face_down_index],
        "Face-down creature"
    );
    engine
        .apply_command(0, &submit_resolution_choice(vec![host]))
        .expect("choose legal Aura recipient");
    assert_eq!(engine.state.objects[&copy].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&copy].attached_to,
        Some(AttachmentRecipient::Object(host))
    );
    assert!(
        engine.state.objects[&copy]
            .copiable_values
            .as_ref()
            .unwrap()
            .face
            .is_aura
    );
}

#[test]
fn entry_copy_of_a_player_aura_uses_the_existing_player_recipient_choice() {
    let mut engine = copy_engine(303_402);
    let aura = CardRegistry::global()
        .get("curse_of_disturbance")
        .expect("Curse of Disturbance")
        .primary_face()
        .clone();
    let source = source_copy_object(&mut engine, 1, aura, AttachmentRecipient::Player(1));
    let copy = cast_entry_copy(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .expect("choose Aura copy source");
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("player Aura choice");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::AuraPlayer);
    assert_eq!(pending.presentation.candidates, vec![0, 1]);
    engine
        .apply_command(0, &submit_resolution_choice(vec![1]))
        .expect("choose enchanted player");
    assert_eq!(engine.state.objects[&copy].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&copy].attached_to,
        Some(AttachmentRecipient::Player(1))
    );
}

#[test]
fn entry_copy_of_an_aura_with_no_legal_recipient_goes_from_stack_to_graveyard() {
    let mut engine = copy_engine(303_403);
    let aura = CardRegistry::global()
        .get("pacifism")
        .expect("Pacifism")
        .primary_face()
        .clone();
    let source_host = put_creature_on_battlefield(&mut engine, 1, "entry_copy_fixture");
    let source = source_copy_object(
        &mut engine,
        1,
        aura,
        AttachmentRecipient::Object(source_host),
    );
    let copy = cast_entry_copy(&mut engine);
    let batch = engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .expect("select Aura without a legal creature recipient");
    assert_eq!(engine.state.objects[&copy].zone, Zone::Graveyard);
    assert!(engine.state.players[0]
        .battlefield
        .iter()
        .all(|id| *id != copy));
    assert!(engine.state.pending_resolution.is_none());
    assert!(batch.events.iter().any(|event| matches!(
        event.ev,
        Some(Ev::StackResolved(ref resolved))
            if resolved.object_id == copy
                && resolved.destination == StackResolveDestination::Graveyard as i32
    )));
    assert!(!batch.events.iter().any(|event| matches!(
        event.ev,
        Some(Ev::PermanentMoved(ref moved))
            if moved.object_id == copy
                && moved.destination == tricerules_proto::ruled::v1::permanent_moved::Destination::Battlefield as i32
    )));
    assert!(engine.state.objects[&copy].copiable_values.is_none());
    assert_eq!(engine.state.objects[&copy].copy_revision, 0);
}

#[test]
fn entry_copy_rejects_an_aura_recipient_from_an_old_zone_generation() {
    let mut engine = copy_engine(303_404);
    let host = put_creature_on_battlefield(&mut engine, 0, "entry_copy_fixture");
    let source_host = put_creature_on_battlefield(&mut engine, 1, "entry_copy_fixture");
    let aura = CardRegistry::global()
        .get("pacifism")
        .expect("Pacifism")
        .primary_face()
        .clone();
    let source = source_copy_object(
        &mut engine,
        1,
        aura,
        AttachmentRecipient::Object(source_host),
    );
    let copy = cast_entry_copy(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .expect("choose Aura copy source");

    let generation = engine
        .state
        .zone_change_generation
        .get(&host)
        .copied()
        .unwrap_or(0);
    engine
        .state
        .zone_change_generation
        .insert(host, generation + 1);
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![host]))
        .is_err());
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::AuraPermanent
    );
    assert_eq!(engine.state.objects[&copy].zone, Zone::Stack);
    assert!(
        engine.state.objects[&copy]
            .copiable_values
            .as_ref()
            .unwrap()
            .face
            .is_aura
    );
    engine.state.zone_change_generation.insert(host, generation);
    engine
        .apply_command(0, &submit_resolution_choice(vec![host]))
        .expect("choose the recipient after restoring the test generation");
    assert_eq!(engine.state.objects[&copy].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&copy].attached_to,
        Some(AttachmentRecipient::Object(host))
    );
}

#[test]
fn copied_non_aura_does_not_keep_the_printed_auras_cast_target() {
    let mut engine = aura_spell_copy_engine(303_407);
    let original_target =
        inject_creature_on_battlefield(&mut engine, 0, "entry_copy_plain_creature");
    let source = inject_creature_on_battlefield(&mut engine, 1, "entry_copy_plain_creature");
    let copy = engine.state.players[0]
        .hand
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == "entry_copy_aura_spell")
        .expect("Aura copy in hand");
    let slot = hand_index_for_card(&engine, 0, "entry_copy_aura_spell");
    engine
        .apply_command(
            0,
            &cast_spell(
                slot,
                vec![tricerules_proto::ruled::v1::TargetRef {
                    object_id: original_target,
                    damage_amount: 0,
                    group_index: 0,
                    kind: 0,
                }],
            ),
        )
        .expect("cast Aura spell targeting a legal creature");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .expect("choose a non-Aura permanent to copy");

    assert_eq!(engine.state.objects[&copy].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&copy].attached_to, None);
    assert!(engine.characteristics(copy).unwrap().is_creature());
}

fn resolve_aura_copy_token_spell(
    engine: &mut GameEngine,
    aura: u32,
) -> tricerules_proto::ruled::v1::RuledEventBatch {
    let slot = hand_index_for_card(engine, 0, "entry_copy_aura_token_spell");
    engine.state.players[0].mana_pool.blue = 1;
    engine.state.players[0].mana_pool.colorless = 2;
    engine
        .apply_command(
            0,
            &cast_spell(
                slot,
                vec![tricerules_proto::ruled::v1::TargetRef {
                    object_id: aura,
                    damage_amount: 0,
                    group_index: 0,
                    kind: 0,
                }],
            ),
        )
        .expect("cast token-copy spell targeting the Aura");
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    engine
        .apply_command(first, &pass())
        .expect("first player passes");
    engine
        .apply_command(second, &pass())
        .expect("second player resolves the token-copy spell")
}

fn token_copy_aura_source(engine: &mut GameEngine) -> u32 {
    let host = inject_creature_on_battlefield(engine, 1, "entry_copy_fixture");
    let aura = inject_permanent_on_battlefield(engine, 1, "entry_copy_aura_source");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(AttachmentRecipient::Object(host));
    aura
}

#[test]
fn copied_aura_token_with_no_legal_recipient_is_never_created() {
    let mut engine = token_copy_engine(303_405);
    let aura = token_copy_aura_source(&mut engine);
    let batch = resolve_aura_copy_token_spell(&mut engine, aura);

    assert!(engine.state.pending_resolution.is_none());
    assert!(
        engine
            .state
            .objects
            .values()
            .all(|object| !object.is_token()),
        "the token must not be created"
    );
    assert!(!batch
        .events
        .iter()
        .any(|event| matches!(event.ev, Some(Ev::TokenCreated(_)))));
}

#[test]
fn copied_aura_token_requires_and_uses_a_legal_attachment_recipient() {
    let mut engine = token_copy_engine(303_406);
    let aura = token_copy_aura_source(&mut engine);
    let recipient = inject_creature_on_battlefield(&mut engine, 0, "entry_copy_fixture");
    let _ = resolve_aura_copy_token_spell(&mut engine, aura);

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("copied Aura token must ask for a recipient before entry");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::AuraPermanent);
    assert_eq!(pending.presentation.candidates, vec![recipient]);
    engine
        .apply_command(0, &submit_resolution_choice(vec![recipient]))
        .expect("choose legal recipient for copied Aura token");

    let token = engine
        .state
        .objects
        .values()
        .find(|object| object.is_token())
        .expect("copied Aura token entered");
    assert_eq!(token.zone, Zone::Battlefield);
    assert_eq!(
        token.attached_to,
        Some(AttachmentRecipient::Object(recipient))
    );
}
