#include "ruled_utils.h"

#include <google/protobuf/descriptor.h>
#include <google/protobuf/reflection.h>
#include <vector>

void clearRuledFieldsByVisibility(google::protobuf::Message *message, ruled::v1::FieldVisibility visibility)
{
    if (!message) {
        return;
    }
    const google::protobuf::Reflection *reflection = message->GetReflection();
    std::vector<const google::protobuf::FieldDescriptor *> presentFields;
    reflection->ListFields(*message, &presentFields);
    for (const google::protobuf::FieldDescriptor *field : presentFields) {
        const auto classified = field->options().HasExtension(ruled::v1::field_visibility)
                                    ? field->options().GetExtension(ruled::v1::field_visibility)
                                    : ruled::v1::FIELD_VISIBILITY_UNSPECIFIED;
        if (classified == visibility) {
            reflection->ClearField(message, field);
            continue;
        }
        if (field->cpp_type() != google::protobuf::FieldDescriptor::CPPTYPE_MESSAGE) {
            continue;
        }
        if (field->is_repeated()) {
            const int count = reflection->FieldSize(*message, field);
            for (int i = 0; i < count; ++i) {
                clearRuledFieldsByVisibility(reflection->MutableRepeatedMessage(message, field, i), visibility);
            }
        } else if (reflection->HasField(*message, field)) {
            clearRuledFieldsByVisibility(reflection->MutableMessage(message, field), visibility);
        }
    }
}

bool isRuledModeManaPoolCounterName(const QString &name)
{
    const QString n = name.trimmed().toLower();
    if (n.length() != 1) {
        return false;
    }
    return QStringLiteral("wubrgxc").contains(n.at(0), Qt::CaseInsensitive);
}

bool isPrivateChoiceKind(ruled::v1::ChoiceKind kind)
{
    switch (kind) {
        case ruled::v1::CHOICE_KIND_HAND_CARDS:     // the decider's own hand
        case ruled::v1::CHOICE_KIND_LIBRARY_SEARCH: // the decider's library
        case ruled::v1::CHOICE_KIND_OPPONENT_HAND:  // another player's hand, CR 701.7 "look"
            return true;
        case ruled::v1::CHOICE_KIND_REVEALED:
        case ruled::v1::CHOICE_KIND_TARGET_OBJECTS:
        case ruled::v1::CHOICE_KIND_LEGEND_KEEP:
            return false;
        default:
            // Unknown kind from a newer engine: assume it conceals something.
            return true;
    }
}

int ruledPhaseToCockatricePhase(ruled::v1::PhaseId phase)
{
    switch (phase) {
        case ruled::v1::PHASE_ID_UNTAP:
            return 0;
        case ruled::v1::PHASE_ID_UPKEEP:
            return 1;
        case ruled::v1::PHASE_ID_DRAW:
            return 2;
        case ruled::v1::PHASE_ID_MAIN1:
            return 3;
        case ruled::v1::PHASE_ID_BEGIN_COMBAT:
            return 4;
        case ruled::v1::PHASE_ID_DECLARE_ATTACKERS:
            return 5;
        case ruled::v1::PHASE_ID_DECLARE_BLOCKERS:
            return 6;
        case ruled::v1::PHASE_ID_COMBAT_DAMAGE:
        case ruled::v1::PHASE_ID_FIRST_STRIKE_DAMAGE:
            // CR 510.4: the first-strike substep shares the "Combat Damage Step" slot in the
            // Cockatrice phases toolbar; the prompt widget distinguishes them via the
            // `first_strike_step_pending` flag on the per-player view.
            return 7;
        case ruled::v1::PHASE_ID_END_COMBAT:
            return 8;
        case ruled::v1::PHASE_ID_MAIN2:
            return 9;
        case ruled::v1::PHASE_ID_END_STEP:
        case ruled::v1::PHASE_ID_CLEANUP:
            return 10;
        default:
            // Opening pseudo-phases, assign-combat-damage and anything unknown have no toolbar
            // slot; -1 tells callers to leave the highlight where it is.
            return -1;
    }
}
