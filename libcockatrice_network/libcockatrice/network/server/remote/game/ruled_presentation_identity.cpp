// Fork-owned client identity translation. See ruled_presentation_identity.h.
#include "ruled_presentation_identity.h"

#include "server_game.h"
#include <QCryptographicHash>
#include <QDebug>
#include <QRandomGenerator>
#include <QUuid>
#include <google/protobuf/descriptor.h>
#include <google/protobuf/message.h>
#include <google/protobuf/reflection.h>
#include <algorithm>
#include <type_traits>
#include <vector>

using google::protobuf::FieldDescriptor;
using google::protobuf::Message;

namespace {

void appendName(std::string &out, std::string_view value)
{
    out.append(value.data(), value.size());
    out.push_back('|');
}

void appendEnum(std::string &out, const google::protobuf::EnumDescriptor *enumeration)
{
    appendName(out, enumeration->full_name());
    for (int i = 0; i < enumeration->value_count(); ++i) {
        appendName(out, enumeration->value(i)->name());
        out.append(std::to_string(enumeration->value(i)->number())).push_back('|');
    }
}

void appendDescriptor(std::string &out, const google::protobuf::Descriptor *descriptor)
{
    appendName(out, descriptor->full_name());
    for (int i = 0; i < descriptor->field_count(); ++i) {
        const auto *field = descriptor->field(i);
        appendName(out, field->name());
        out.append(std::to_string(field->number())).push_back('|');
        out.append(std::to_string(field->type())).push_back('|');
        out.append(std::to_string(field->is_repeated())).push_back('|');
        out.append(std::to_string(field->has_presence())).push_back('|');
        out.append(std::to_string(field->options().HasExtension(ruled::v1::field_visibility)
                                      ? field->options().GetExtension(ruled::v1::field_visibility) : -1))
            .push_back('|');
        if (field->containing_oneof())
            appendName(out, field->containing_oneof()->name());
        if (field->message_type())
            appendName(out, field->message_type()->full_name());
        if (field->enum_type())
            appendName(out, field->enum_type()->full_name());
    }
    for (int i = 0; i < descriptor->nested_type_count(); ++i)
        appendDescriptor(out, descriptor->nested_type(i));
    for (int i = 0; i < descriptor->enum_type_count(); ++i)
        appendEnum(out, descriptor->enum_type(i));
}

bool supportedSchema()
{
    // Changing any wire field, its visibility, type, domain enum, or nesting requires an explicit
    // audit of the projection above/below before updating this fingerprint. Unknown wire fields
    // are rejected separately, including when an older relay talks to a newer engine.
    static const bool supported = [] {
        const auto *file = ruled::v1::RuledCommand::descriptor()->file();
        std::string contract;
        for (int i = 0; i < file->message_type_count(); ++i)
            appendDescriptor(contract, file->message_type(i));
        for (int i = 0; i < file->enum_type_count(); ++i)
            appendEnum(contract, file->enum_type(i));
        const QByteArray digest = QCryptographicHash::hash(
            QByteArray(contract.data(), static_cast<qsizetype>(contract.size())), QCryptographicHash::Sha256).toHex();
        const QByteArray expected("1c564af2ac3897f85414eff56d940e7d7428da601f66e007659a2b2d438b9558");
        if (digest != expected)
            qCritical() << "ruled presentation schema needs an identity audit:" << digest;
        return digest == expected;
    }();
    return supported;
}

// These are object references without a companion generation. Other numeric fields, including
// player ids, slots, application ids and transaction ids, deliberately do not appear here.
const QSet<QString> &objectFields()
{
    static const QSet<QString> fields = {
        "SubmitTriggerOrder.trigger_object_id", "DamagePair.blocker_id", "AssignCombatDamage.attacker_id",
        "BlockPair.attacker_id", "BlockPair.blocker_id", "LegalTargetGroup.valid_permanent_ids",
        "LegalTargetGroup.valid_stack_ids", "LegalTargetGroup.valid_graveyard_ids",
        "ExilePlayPermissionGroup.object_ids", "LegalActions.attack_requirement_ids",
        "LegalActions.required_blocker_ids", "LegalActions.selectable_attacker_ids",
        "PendingAbilityActivation.reserved_object_id", "PendingSpellCast.reserved_object_id",
        "TriggerOrderCandidate.trigger_object_id", "PermanentsUntapped.object_ids", "AttachmentRecipient.object_id",
        "AuraAttached.aura_object_id", "FaceChanged.object_id", "ResolutionChoiceProgress.source_object_id",
        "ResolutionChoiceRequired.source_object_id", "TokenCreated.object_id", "CombatDamageAssigned.attacker_id",
        "StackPushed.object_id", "StackPushed.copy_source_object_id",
        "CostSelection.permanent_id", "StackResolved.object_id", "StackObjectCountered.object_id",
        "StackObjectRemoved.object_id", "EmblemMarkerView.object_id", "RuledPerPlayerView.graveyard_object_ids",
        "RuledPerPlayerView.exile_object_ids", "RuledPerPlayerView.command_zone_object_ids", "PermanentMoved.object_id",
        "GraveyardObjectMap.Entry.engine_object_id", "ExileObjectMap.Entry.engine_object_id",
        "BattlefieldObjectMap.Entry.engine_object_id", "CreaturesRemovedFromCombat.object_ids"
    };
    return fields;
}

QString shortName(const Message &message, const FieldDescriptor *field)
{
    const auto typeName = message.GetDescriptor()->full_name();
    const auto fieldName = field->name();
    QString type = QString::fromUtf8(typeName.data(), static_cast<qsizetype>(typeName.size()));
    type.remove(0, QStringLiteral("ruled.v1.").size());
    return type + QLatin1Char('.') + QString::fromUtf8(fieldName.data(), static_cast<qsizetype>(fieldName.size()));
}

quint64 randomGenerationMask()
{
    quint64 value = 0;
    while (!value)
        value = QRandomGenerator::system()->generate64();
    return value;
}

std::optional<bool> choiceUsesObjects(const ruled::v1::ResolutionChoiceRequired *choice)
{
    if (!choice)
        return std::nullopt;
    switch (choice->choice_kind()) {
        case ruled::v1::CHOICE_KIND_AURA_PLAYER:
        case ruled::v1::CHOICE_KIND_BATTLE_PROTECTOR:
        case ruled::v1::CHOICE_KIND_REPLACEMENT_EFFECT:
            return false;
        case ruled::v1::CHOICE_KIND_PRIVATE_REPLACEMENT:
            return choice->ordered();
        case ruled::v1::CHOICE_KIND_HAND_CARDS:
        case ruled::v1::CHOICE_KIND_REVEALED:
        case ruled::v1::CHOICE_KIND_LIBRARY_SEARCH:
        case ruled::v1::CHOICE_KIND_TARGET_OBJECTS:
        case ruled::v1::CHOICE_KIND_OPPONENT_HAND:
        case ruled::v1::CHOICE_KIND_LEGEND_KEEP:
        case ruled::v1::CHOICE_KIND_LIBRARY_TOP:
        case ruled::v1::CHOICE_KIND_COPY_SOURCE:
        case ruled::v1::CHOICE_KIND_MANA_PAYMENT:
        case ruled::v1::CHOICE_KIND_RESOLUTION_BRANCH:
        case ruled::v1::CHOICE_KIND_LIBRARY_LOOK:
        case ruled::v1::CHOICE_KIND_MANIFEST_DREAD:
        case ruled::v1::CHOICE_KIND_ZONE_SEARCH:
        case ruled::v1::CHOICE_KIND_GRAVEYARD_CARDS:
        case ruled::v1::CHOICE_KIND_AURA_PERMANENT:
        case ruled::v1::CHOICE_KIND_SPECIAL_CAST:
        case ruled::v1::CHOICE_KIND_ATTACKING_TOKEN_DEFENDER:
        case ruled::v1::CHOICE_KIND_COST_OBJECTS:
        case ruled::v1::CHOICE_KIND_PERMANENT_OBJECTS:
        case ruled::v1::CHOICE_KIND_BEHOLD:
        case ruled::v1::CHOICE_KIND_PROLIFERATE:
        case ruled::v1::CHOICE_KIND_SIMULTANEOUS_ENTRY_ORDER:
        case ruled::v1::CHOICE_KIND_LINKED_EXILE_CARDS:
            return true;
        default:
            return std::nullopt;
    }
}

bool isPlayerTarget(const Message &message, const Server_Game *game)
{
    const auto *descriptor = message.GetDescriptor();
    const auto *reflection = message.GetReflection();
    const auto *kind = descriptor->FindFieldByName("kind");
    if (!kind)
        return false;
    const int value = reflection->GetEnumValue(message, kind);
    if (value == ruled::v1::TARGET_REF_KIND_PLAYER)
        return true;
    if (value != ruled::v1::TARGET_REF_KIND_UNSPECIFIED)
        return false;
    const auto *id = descriptor->FindFieldByName("object_id");
    const auto *generation = descriptor->FindFieldByName("expected_zone_change_generation");
    if (generation && reflection->HasField(message, generation))
        return false;
    return id && game->getPlayers().contains(static_cast<int>(reflection->GetUInt32(message, id)));
}

// Used only when accepting a first, unmarked face-down startup snapshot. A preceding event
// mentioning the same number is ambiguous, so the whole batch is withheld instead of linking it.
bool containsUnsignedId(const Message &message, quint32 id)
{
    const auto *reflection = message.GetReflection();
    std::vector<const FieldDescriptor *> fields;
    reflection->ListFields(message, &fields);
    for (const auto *field : fields) {
        const int count = field->is_repeated() ? reflection->FieldSize(message, field) : 1;
        for (int i = 0; i < count; ++i) {
            if (field->cpp_type() == FieldDescriptor::CPPTYPE_MESSAGE) {
                const auto &child = field->is_repeated() ? reflection->GetRepeatedMessage(message, field, i)
                                                        : reflection->GetMessage(message, field);
                if (containsUnsignedId(child, id))
                    return true;
            } else if (field->cpp_type() == FieldDescriptor::CPPTYPE_UINT32) {
                const quint32 value = field->is_repeated() ? reflection->GetRepeatedUInt32(message, field, i)
                                                           : reflection->GetUInt32(message, field);
                if (value == id)
                    return true;
            }
        }
    }
    return false;
}

} // namespace

RuledPresentationIdentity::RuledPresentationIdentity(const Server_Game *_game) : game(_game) {}

void RuledPresentationIdentity::reset()
{
    epochs.clear();
    handles.clear();
    usedHandles.clear();
    issued.clear();
    batchStartEpochs.clear();
    visibleIncarnations.clear();
    faceDownExiles.clear();
    revealOccurrences.clear();
}

quint32 RuledPresentationIdentity::allocateHandle()
{
    // Keep handles positive in legacy Qt paths which store public-zone identities as int.
    for (int attempt = 0; attempt < 4096; ++attempt) {
        const quint32 candidate = (QRandomGenerator::system()->generate() & 0x3fffffffu) | 0x40000000u;
        if (candidate != 0 && !usedHandles.contains(candidate) &&
            !game->getPlayers().contains(static_cast<int>(candidate)))
            return candidate;
    }
    return 0;
}

bool RuledPresentationIdentity::ensureObject(quint32 objectId)
{
    if (objectId == 0 || epochs.contains(objectId))
        return true;
    quint32 handle = objectId;
    if (handle > 0x7fffffffu || usedHandles.contains(handle) || game->getPlayers().contains(static_cast<int>(handle)))
        handle = allocateHandle();
    if (!handle)
        return false;
    epochs[objectId].append({0, handle, handle == objectId ? 0 : randomGenerationMask()});
    handles.insert(handle, {objectId, 0});
    usedHandles.insert(handle);
    return true;
}

bool RuledPresentationIdentity::ensureProtectedObject(quint32 objectId)
{
    if (!objectId || epochs.contains(objectId))
        return false;
    // Even an initially unseen canonical ID must not be selected as its protected handle.
    usedHandles.insert(objectId);
    const quint32 handle = allocateHandle();
    if (!handle)
        return false;
    epochs[objectId].append({0, handle, randomGenerationMask()});
    handles.insert(handle, {objectId, 0});
    usedHandles.insert(handle);
    return true;
}

bool RuledPresentationIdentity::startEpoch(quint32 objectId, quint64 generation)
{
    if (!objectId || !ensureObject(objectId))
        return false;
    auto &history = epochs[objectId];
    for (int i = 1; i < history.size(); ++i)
        if (history[i].firstGeneration == generation)
            return true; // A repeated publication of this same boundary is idempotent.
    if (generation <= history.last().firstGeneration)
        return false;
    const quint32 handle = allocateHandle();
    if (!handle)
        return false;
    history.append({generation, handle, randomGenerationMask()});
    handles.insert(handle, {objectId, static_cast<int>(history.size() - 1)});
    usedHandles.insert(handle);
    return true;
}

bool RuledPresentationIdentity::beginBatch(const ruled::v1::RuledEventBatch &batch)
{
    if (!supportedSchema())
        return false;
    batchStartEpochs.clear();
    for (auto it = epochs.cbegin(); it != epochs.cend(); ++it)
        batchStartEpochs.insert(it.key(), it.value().size() - 1);
    QSet<quint32> markedObjects;
    QHash<quint32, quint64> lastMarkers;
    for (const auto &event : batch.events()) {
        if (!event.has_presentation_identity_break())
            continue;
        const auto &boundary = event.presentation_identity_break();
        if (lastMarkers.contains(boundary.object_id()) &&
            boundary.zone_change_generation() < lastMarkers.value(boundary.object_id()))
            return false;
        lastMarkers.insert(boundary.object_id(), boundary.zone_change_generation());
        if (!ensureObject(boundary.object_id()))
            return false;
        if (!batchStartEpochs.contains(boundary.object_id()))
            batchStartEpochs.insert(boundary.object_id(), epochs[boundary.object_id()].size() - 1);
        if (!startEpoch(boundary.object_id(), boundary.zone_change_generation()))
            return false;
        if (!markedObjects.contains(boundary.object_id())) {
            const auto &history = epochs[boundary.object_id()];
            int index = -1;
            for (int i = 1; i < history.size(); ++i)
                if (history[i].firstGeneration == boundary.zone_change_generation())
                    index = i;
            if (index < 1)
                return false;
            batchStartEpochs.insert(boundary.object_id(), index - 1);
            markedObjects.insert(boundary.object_id());
        }
    }
    for (auto it = lastMarkers.cbegin(); it != lastMarkers.cend(); ++it)
        if (epochs[it.key()].last().firstGeneration != it.value())
            return false;

    // A full zone snapshot supplies the current incarnation. A same-generation battlefield
    // turn-down stays trackable; an unmarked new incarnation is never allowed to reuse a handle.
    constexpr int battlefield = 1;
    constexpr int exile = 2;
    QHash<quint32, int> liveEpochs = batchStartEpochs;
    for (int eventIndex = 0; eventIndex < batch.events_size(); ++eventIndex) {
        const auto &event = batch.events(eventIndex);
        if (event.has_presentation_identity_break()) {
            const auto &boundary = event.presentation_identity_break();
            const auto &history = epochs[boundary.object_id()];
            for (int i = 1; i < history.size(); ++i)
                if (history[i].firstGeneration == boundary.zone_change_generation())
                    liveEpochs.insert(boundary.object_id(), i);
            continue;
        }
        if (!event.has_zone_view())
            continue;
        const auto &view = event.zone_view();
        QHash<quint32, SeenIncarnation> next = visibleIncarnations;
        if (!view.battlefields_unchanged()) {
            for (auto it = next.begin(); it != next.end();) {
                if (it->zone == battlefield)
                    it = next.erase(it);
                else
                    ++it;
            }
        }
        for (auto it = next.begin(); it != next.end();) {
            if (it->zone == exile)
                it = next.erase(it);
            else
                ++it;
        }
        QHash<quint32, quint64> newFaceDownExiles;
        const auto validate = [&](quint32 id, quint64 generation, int zone) {
            if (!id)
                return false;
            if (!epochs.contains(id)) {
                for (int i = 0; i < eventIndex; ++i)
                    if (containsUnsignedId(batch.events(i), id))
                        return false;
                if (!ensureProtectedObject(id))
                    return false;
            } else {
                const auto &history = epochs[id];
                const int index = liveEpochs.value(id, history.size() - 1);
                if (index < 0 || index >= history.size())
                    return false;
                const auto previous = visibleIncarnations.constFind(id);
                const bool sameVisible = previous != visibleIncarnations.cend() &&
                                         previous->zone == zone && previous->generation == generation;
                const bool marked = index > 0 && history[index].firstGeneration == generation;
                if ((!sameVisible && !marked) || history[index].firstGeneration > generation ||
                    (index + 1 < history.size() && history[index + 1].firstGeneration <= generation))
                    return false;
            }
            return true;
        };
        for (const auto &player : view.per_player()) {
            for (const auto &object : player.battlefield_objects()) {
                if (object.face_down() && !validate(object.object_id(), object.zone_change_generation(), battlefield))
                    return false;
                next.insert(object.object_id(), {battlefield, object.zone_change_generation()});
            }
            for (const auto &object : player.presentation_face_down_exiles()) {
                if (!validate(object.object_id(), object.zone_change_generation(), exile))
                    return false;
                newFaceDownExiles.insert(object.object_id(), object.zone_change_generation());
                next.insert(object.object_id(), {exile, object.zone_change_generation()});
            }
        }
        visibleIncarnations = std::move(next);
        faceDownExiles = std::move(newFaceDownExiles);
    }
    // Check independently against the physical exile map; metadata omission must fail closed.
    for (const auto &event : batch.events()) {
        if (!event.has_exile_object_map())
            continue;
        for (const auto &entry : event.exile_object_map().entries())
            if (entry.face_down() && !faceDownExiles.contains(entry.engine_object_id()))
                return false;
    }
    for (const auto &event : batch.events()) {
        if (!event.has_face_down_object_map())
            continue;
        for (const auto &entry : event.face_down_object_map().entries()) {
            const auto seen = visibleIncarnations.constFind(entry.engine_object_id());
            if (seen == visibleIncarnations.cend() || seen->generation != entry.zone_change_generation())
                return false;
        }
    }
    return true;
}

bool RuledPresentationIdentity::object(quint32 &value, Walk &context, std::optional<quint64> generation)
{
    if (!value)
        return true;
    if (context.inbound) {
        const auto found = handles.constFind(value);
        if (found == handles.cend() || !issued.value(context.recipient).contains(value) ||
            found->epoch != epochs.value(found->object).size() - 1)
            return false;
        value = found->object;
        return true;
    }
    if (!ensureObject(value))
        return false;
    const quint32 canonical = value;
    const auto &history = epochs[canonical];
    int index = context.epochByObject.value(canonical, history.size() - 1);
    if (generation) {
        index = 0;
        for (int i = 1; i < history.size() && history[i].firstGeneration <= *generation; ++i)
            index = i;
    }
    if (index < 0 || index >= history.size())
        return false;
    value = history[index].handle;
    context.pendingReceipts.insert(value);
    return true;
}

bool RuledPresentationIdentity::objectAndGeneration(quint32 &id, quint64 &generation, Walk &context)
{
    if (!id)
        return true;
    if (context.inbound) {
        const auto found = handles.constFind(id);
        if (found == handles.cend() || !issued.value(context.recipient).contains(id) ||
            found->epoch != epochs.value(found->object).size() - 1)
            return false;
        generation ^= epochs[found->object][found->epoch].generationMask;
        id = found->object;
        return true;
    }
    const quint32 canonical = id;
    if (!object(id, context, generation))
        return false;
    const auto found = handles.constFind(id);
    if (found == handles.cend() || found->object != canonical)
        return false;
    generation ^= epochs[canonical][found->epoch].generationMask;
    return true;
}

bool RuledPresentationIdentity::project(ruled::v1::RuledEventBatch &batch, int recipient)
{
    if (!supportedSchema() || batch.GetReflection()->GetUnknownFields(batch).field_count())
        return false;
    Walk context;
    context.recipient = recipient;
    const bool hasBoundary = std::any_of(batch.events().begin(), batch.events().end(),
        [](const auto &event) { return event.has_presentation_identity_break(); });
    if (hasBoundary)
        context.epochByObject = batchStartEpochs;
    for (auto &event : *batch.mutable_events()) {
        if (event.has_presentation_identity_break()) {
            const auto &boundary = event.presentation_identity_break();
            const auto &history = epochs.value(boundary.object_id());
            int index = -1;
            for (int i = 1; i < history.size(); ++i)
                if (history[i].firstGeneration == boundary.zone_change_generation())
                    index = i;
            if (index < context.epochByObject.value(boundary.object_id(), 0))
                return false;
            context.epochByObject.insert(boundary.object_id(), index);
        } else if (!walk(&event, context)) {
            return false;
        }
    }
    for (auto &item : *batch.mutable_legal_by_player())
        if (!walk(&item.second, context))
            return false;
    if (batch.has_payment_preview() && !walk(batch.mutable_payment_preview(), context))
        return false;
    if (batch.has_resolution_prompt_state() && !walk(batch.mutable_resolution_prompt_state(), context))
        return false;
    issued[recipient].unite(context.pendingReceipts);
    return true;
}

bool RuledPresentationIdentity::projectPayment(ruled::v1::PaymentPreview &preview, int recipient)
{
    if (!supportedSchema())
        return false;
    Walk context;
    context.recipient = recipient;
    if (!walk(&preview, context))
        return false;
    issued[recipient].unite(context.pendingReceipts);
    return true;
}

bool RuledPresentationIdentity::canonicalize(ruled::v1::RuledCommand &command, int recipient,
                                              const ruled::v1::ResolutionChoiceRequired *choice)
{
    if (!supportedSchema())
        return false;
    Walk context;
    context.recipient = recipient;
    context.inbound = true;
    context.choice = choice;
    return walk(&command, context);
}

bool RuledPresentationIdentity::walk(Message *message, Walk &context)
{
    const auto *descriptor = message->GetDescriptor();
    const auto *reflection = message->GetReflection();
    if (reflection->GetUnknownFields(*message).field_count())
        return false;
    const auto typeName = descriptor->full_name();
    QString type = QString::fromUtf8(typeName.data(), static_cast<qsizetype>(typeName.size()));
    type.remove(0, QStringLiteral("ruled.v1.").size());
    if (context.inbound && type == QLatin1String("SubmitResolutionChoice")) {
        const auto *submission = static_cast<const ruled::v1::SubmitResolutionChoice *>(message);
        if (!context.choice || !context.choice->resolution_prompt_id() ||
            submission->resolution_prompt_id() != context.choice->resolution_prompt_id())
            return false;
    }

    // Each entry associates an object reference with the generation of that same incarnation.
    // Source-generation metadata on triggers is server-only, but still chooses the old epoch.
    static const QHash<QString, QString> pairs = {
        {"CastSource.graveyard_object_id", "expected_zone_change_generation"},
        {"CastSource.exile_object_id", "expected_zone_change_generation"},
        {"CastSource.library_object_id", "expected_zone_change_generation"},
        {"LandSource.graveyard_object_id", "expected_zone_change_generation"},
        {"LandSource.exile_object_id", "expected_zone_change_generation"},
        {"LandSource.library_object_id", "expected_zone_change_generation"},
        {"CastCostGroupSelection.permanent_id", "expected_zone_change_generation"},
        {"TargetRef.object_id", "expected_zone_change_generation"},
        {"AttackAssignment.attacker_object_id", "attacker_zone_change_generation"},
        {"XTargetCandidate.object_id", "zone_change_generation"},
        {"LegalZoneCastAction.object_id", "zone_change_generation"},
        {"CostObjectRef.object_id", "zone_change_generation"},
        {"LegalZoneLandAction.object_id", "zone_change_generation"},
        {"LegalPermanentAction.object_id", "zone_change_generation"},
        {"LegalZoneAbilityAction.object_id", "zone_change_generation"},
        {"AbilityActivationTarget.object_id", "zone_change_generation"},
        {"PendingAbilityActivation.source_object_id", "source_zone_change_generation"},
        {"BeginAbilityActivation.source_object_id", "expected_zone_change_generation"},
        {"AttackManaAbilityUndoOption.source_object_id", "source_zone_change_generation"},
        {"TriggerOrderCandidate.source_permanent_id", "source_zone_change_generation"},
        {"AcceptedPermanentSelection.object_id", "zone_change_generation"},
        {"ReplacementEffectOption.source_object_id", "source_zone_change_generation"},
        {"CardsRevealed.source_object_id", "source_zone_change_generation"},
        {"RevealedCard.object_id", "zone_change_generation"},
        {"ActivateAbility.source_object_id", "expected_zone_change_generation"},
        {"ExecutePermanentAction.object_id", "expected_zone_change_generation"},
        {"TriggerNeedsTarget.source_permanent_id", "source_zone_change_generation"},
        {"PreparationSource.object_id", "zone_change_generation"},
        {"PreparationState.copy_object_id", "copy_zone_change_generation"},
        {"PrepareSpellCopyView.object_id", "zone_change_generation"},
        {"BattlefieldObject.object_id", "zone_change_generation"},
        {"StackAbilitySource.object_id", "zone_change_generation"},
        {"PublicHandCard.object_id", "zone_change_generation"},
        {"PrivateLibraryTopView.object_id", "zone_change_generation"},
        {"FaceDownObjectMap.Entry.engine_object_id", "zone_change_generation"}
    };

    // These four maps pack a public object handle into the key; their low bits remain the
    // engine's ability or face index. Rebuild the map so protobuf's map index stays coherent.
    if (type == QLatin1String("LegalActions")) {
        auto *actions = static_cast<ruled::v1::LegalActions *>(message);
        const auto rekey = [&](auto *map, unsigned shift) {
            using Value = std::decay_t<decltype(map->begin()->second)>;
            std::vector<std::pair<quint64, Value>> entries;
            for (const auto &item : *map) {
                quint32 id = static_cast<quint32>(item.first >> shift);
                if (!object(id, context))
                    return false;
                const quint64 lowMask = (quint64{1} << shift) - 1;
                entries.emplace_back((static_cast<quint64>(id) << shift) | (item.first & lowMask), item.second);
            }
            map->clear();
            for (auto &item : entries)
                (*map)[item.first] = std::move(item.second);
            return true;
        };
        if (!rekey(actions->mutable_valid_targets_by_ability(), 32) ||
            !rekey(actions->mutable_valid_targets_by_zone_object(), 8) ||
            !rekey(actions->mutable_cost_choices_by_ability(), 32) ||
            !rekey(actions->mutable_mana_payment_by_ability(), 32))
            return false;
    }

    // Combat defenders carry their generation on the containing edge, while the target itself
    // uses TargetRef. Translate the two as one pair, including an optional target generation.
    if (type == QLatin1String("AttackAssignment") || type == QLatin1String("CombatDefenderOption")) {
        const auto *defenderField = descriptor->FindFieldByName("defender");
        const auto *generationField = descriptor->FindFieldByName("defender_zone_change_generation");
        if (reflection->HasField(*message, defenderField)) {
            Message *defender = reflection->MutableMessage(message, defenderField);
            const auto *targetDescriptor = defender->GetDescriptor();
            const auto *targetReflection = defender->GetReflection();
            if (!isPlayerTarget(*defender, game)) {
                const auto *idField = targetDescriptor->FindFieldByName("object_id");
                const auto *expected = targetDescriptor->FindFieldByName("expected_zone_change_generation");
                quint32 id = targetReflection->GetUInt32(*defender, idField);
                quint64 generation = reflection->GetUInt64(*message, generationField);
                const quint32 original = id;
                if (!objectAndGeneration(id, generation, context))
                    return false;
                if (original && targetReflection->HasField(*defender, expected)) {
                    const quint32 handle = context.inbound ? original : id;
                    const auto found = handles.constFind(handle);
                    if (found == handles.cend())
                        return false;
                    targetReflection->SetUInt64(defender, expected,
                        targetReflection->GetUInt64(*defender, expected) ^
                            epochs[found->object][found->epoch].generationMask);
                }
                targetReflection->SetUInt32(defender, idField, id);
                reflection->SetUInt64(message, generationField, generation);
                context.translatedTargets.insert(defender);
            }
        }
    }

    QSet<const FieldDescriptor *> processed;
    for (int i = 0; i < descriptor->field_count(); ++i) {
        const auto *field = descriptor->field(i);
        const auto it = pairs.constFind(shortName(*message, field));
        if (it == pairs.cend() || (type == QLatin1String("TargetRef") &&
                                    context.translatedTargets.contains(message)))
            continue;
        if (field->has_presence() && !reflection->HasField(*message, field))
            continue;
        if (type == QLatin1String("TargetRef") && isPlayerTarget(*message, game))
            continue;
        const auto *generationField = descriptor->FindFieldByName(it.value().toStdString());
        if (!generationField)
            return false;
        quint32 id = reflection->GetUInt32(*message, field);
        if (!id)
            continue;
        if (generationField->has_presence() && !reflection->HasField(*message, generationField)) {
            if (!object(id, context))
                return false;
        } else {
            quint64 generation = reflection->GetUInt64(*message, generationField);
            if (!objectAndGeneration(id, generation, context))
                return false;
            reflection->SetUInt64(message, generationField, generation);
            processed.insert(generationField);
        }
        reflection->SetUInt32(message, field, id);
        processed.insert(field);
    }

    if (type == QLatin1String("LegalCastCostOption")) {
        auto *option = static_cast<ruled::v1::LegalCastCostOption *>(message);
        if (option->valid_permanent_generations_size() &&
            option->valid_permanent_generations_size() != option->valid_permanent_ids_size())
            return false;
        for (int i = 0; i < option->valid_permanent_ids_size(); ++i) {
            quint32 id = option->valid_permanent_ids(i);
            if (option->valid_permanent_generations_size()) {
                quint64 generation = option->valid_permanent_generations(i);
                if (!objectAndGeneration(id, generation, context))
                    return false;
                option->set_valid_permanent_generations(i, generation);
            } else if (!object(id, context)) {
                return false;
            }
            option->set_valid_permanent_ids(i, id);
        }
        processed.insert(descriptor->FindFieldByName("valid_permanent_ids"));
        processed.insert(descriptor->FindFieldByName("valid_permanent_generations"));
    }

    if (!context.inbound && type == QLatin1String("CardsRevealed")) {
        auto *reveal = static_cast<ruled::v1::CardsRevealed *>(message);
        if (!reveal->reveal_id().empty()) {
            const QString key = QString::fromStdString(reveal->reveal_id());
            if (!revealOccurrences.contains(key))
                revealOccurrences.insert(key, QUuid::createUuid().toString(QUuid::WithoutBraces));
            reveal->set_reveal_id(revealOccurrences[key].toStdString());
        }
    }

    std::vector<const FieldDescriptor *> fields;
    reflection->ListFields(*message, &fields);
    for (const auto *field : fields) {
        if (processed.contains(field))
            continue;
        if (!context.inbound && field->options().HasExtension(ruled::v1::field_visibility) &&
            field->options().GetExtension(ruled::v1::field_visibility) == ruled::v1::FIELD_VISIBILITY_SERVER_ONLY)
            continue;
        const QString fieldName = shortName(*message, field);
        bool isObject = objectFields().contains(fieldName);
        if (fieldName == QLatin1String("TargetCandidateRef.object_id"))
            isObject = !isPlayerTarget(*message, game);
        if (fieldName == QLatin1String("LegalCostChoice.candidate_ids")) {
            const auto zone = static_cast<ruled::v1::LegalCostChoice *>(message)->zone();
            if (zone != ruled::v1::COST_CHOICE_ZONE_HAND && zone != ruled::v1::COST_CHOICE_ZONE_BATTLEFIELD &&
                zone != ruled::v1::COST_CHOICE_ZONE_GRAVEYARD)
                return false;
            isObject = zone != ruled::v1::COST_CHOICE_ZONE_HAND;
        }
        if (fieldName == QLatin1String("ResolutionChoiceRequired.candidate_object_ids")) {
            const auto usesObjects = choiceUsesObjects(static_cast<ruled::v1::ResolutionChoiceRequired *>(message));
            if (!usesObjects)
                return false;
            isObject = *usesObjects;
        }
        if (fieldName == QLatin1String("SubmitResolutionChoice.chosen_object_ids")) {
            const auto usesObjects = choiceUsesObjects(context.choice);
            if (!usesObjects)
                return false;
            isObject = *usesObjects;
        }
        if (isObject) {
            if (field->cpp_type() != FieldDescriptor::CPPTYPE_UINT32)
                return false;
            if (field->is_repeated()) {
                for (int i = 0; i < reflection->FieldSize(*message, field); ++i) {
                    quint32 id = reflection->GetRepeatedUInt32(*message, field, i);
                    if (!object(id, context))
                        return false;
                    reflection->SetRepeatedUInt32(message, field, i, id);
                }
            } else {
                quint32 id = reflection->GetUInt32(*message, field);
                if (!object(id, context))
                    return false;
                reflection->SetUInt32(message, field, id);
            }
        } else if (field->cpp_type() == FieldDescriptor::CPPTYPE_MESSAGE) {
            if (field->is_repeated()) {
                for (int i = 0; i < reflection->FieldSize(*message, field); ++i)
                    if (!walk(reflection->MutableRepeatedMessage(message, field, i), context))
                        return false;
            } else if (!walk(reflection->MutableMessage(message, field), context)) {
                return false;
            }
        }
    }
    return true;
}
