#include "ruled_utils.h"

#include "server_cardzone.h"
#include "server_game.h"

#include <google/protobuf/descriptor.h>
#include <google/protobuf/reflection.h>
#include <libcockatrice/utility/ruled_debug.h>
#include <libcockatrice/utility/zone_names.h>
#include <vector>

bool ruledAllowsCrossPlayerMove(const Server_Game *game,
                                const Server_CardZone *startZone,
                                const Server_CardZone *targetZone)
{
    if (!game || !game->getRuledGame() || !startZone || !targetZone) {
        return false;
    }
    if (startZone->getPlayer() == targetZone->getPlayer()) {
        return false; // Not a cross-player move; upstream's guard does not apply.
    }
    const QString from = startZone->getName();
    const QString to = targetZone->getName();
    // Whether a ruled move is same-player depends on which seat owns the canonical stack, i.e. on
    // join order — so a path can be exercised constantly on one seat and never on the other.
    RULED_TRACE("relay") << "crossPlayerMove: " << from << " -> " << to;

    // 1. Anything into or out of a stack zone. Casting and resolving are the two halves of the
    //    same thing, and both are engine-decided: the relay only ever issues these while mirroring
    //    a RuledEventBatch the engine already validated.
    //
    //    Deliberately the invariant rather than a list of (from, to) pairs. The old list had to
    //    grow for every castable-from zone and every resolves-to zone, and a missing entry broke
    //    only the seat that does *not* own the canonical stack — so it passed review, passed tests
    //    and passed play on the host. Flashback (GRAVE -> STACK) shipped exactly that way; foretell
    //    and adventure (EXILE -> STACK) would have been next.
    //
    //    This does not widen what a client can reach. `cmdMoveCard` already requires write
    //    permission on the start zone and that the sender is party to the move, so no private zone
    //    becomes reachable; the most a rogue client gains is parking its own card on the shared
    //    stack visually, which the engine never sees and the next stack event contradicts.
    if (from == ZoneNames::STACK || to == ZoneNames::STACK) {
        return true;
    }
    // 2. Leaving the battlefield: a permanent controlled by someone who does not own it goes to
    //    its OWNER's zone (CR 400.3). The reverse trip (into the controller's TABLE) needs no
    //    exemption — upstream already allows cross-player moves into a public zone with coords.
    if (from == ZoneNames::TABLE) {
        return true;
    }
    return false;
}

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
        case ruled::v1::CHOICE_KIND_LIBRARY_TOP:    // CR 701.18 scry: the decider's library top
        case ruled::v1::CHOICE_KIND_LIBRARY_LOOK:   // a fixed cohort looked at from library top
            return true;
        case ruled::v1::CHOICE_KIND_REVEALED:
        case ruled::v1::CHOICE_KIND_TARGET_OBJECTS:
        case ruled::v1::CHOICE_KIND_LEGEND_KEEP:
        case ruled::v1::CHOICE_KIND_REPLACEMENT_EFFECT:
        case ruled::v1::CHOICE_KIND_COPY_SOURCE:
        case ruled::v1::CHOICE_KIND_MANA_PAYMENT:
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
