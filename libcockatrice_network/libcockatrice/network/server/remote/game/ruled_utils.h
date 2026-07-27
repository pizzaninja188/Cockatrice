#ifndef RULED_UTILS_H
#define RULED_UTILS_H

#include <QString>
#include <google/protobuf/message.h>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

bool isRuledModeManaPoolCounterName(const QString &name);

/// Maps an engine turn-structure position onto the Cockatrice phases-toolbar slot index, or -1
/// when the phase has no slot (opening procedure, assign-combat-damage, unknown values).
int ruledPhaseToCockatricePhase(ruled::v1::PhaseId phase);

/// True when a resolution-choice kind exposes a zone concealed from the other players, so the
/// relay must strip the candidate ids/names from every participant but the deciding player.
/// Public kinds (revealed cards, battlefield targets, legend keep) pass through untouched.
bool isPrivateChoiceKind(ruled::v1::ChoiceKind kind);

/// Reflection-based fail-closed clearing used by ruled broadcast redaction. Every field
/// reachable from RuledEventBatch is classified in ruled_v1.proto; fields with `visibility`
/// are cleared recursively, including fields introduced by future protocol changes.
void clearRuledFieldsByVisibility(google::protobuf::Message *message, ruled::v1::FieldVisibility visibility);

#endif
