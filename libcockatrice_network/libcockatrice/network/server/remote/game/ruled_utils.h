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

class Server_Game;
class Server_CardZone;

/// True when a ruled game legitimately moves a card between two players' zones, so the upstream
/// `Server_AbstractPlayer::moveCard` controller-change guard must let it through.
///
/// Upstream only permits a cross-player move into a public zone *with* coordinates (the table),
/// which is how freeform models giving control. Ruled mode needs two more shapes, both of them
/// engine-decided rather than client-requested:
///   1. anything into or out of a STACK zone. Stated as the invariant rather
///      than as a list of zone pairs on purpose: a spell can be cast from the hand, from a
///      graveyard (flashback), and later from exile (foretell, adventure), and can resolve to a
///      graveyard, exile, the battlefield or back to a hand. Enumerating those pairs meant every
///      new one was a silent breakage on the non-owning seat only — flashback shipped that way.
///   2. TABLE -> the *owner's* zones, when a permanent someone else controls leaves the
///      battlefield and goes home (CR 400.3) — reanimation's return trip.
///
/// Always false outside a ruled game: freeform's trust model is upstream's business.
bool ruledAllowsCrossPlayerMove(const Server_Game *game,
                                const Server_CardZone *startZone,
                                const Server_CardZone *targetZone);

/// Reflection-based fail-closed clearing used by ruled broadcast redaction. Every field
/// reachable from RuledEventBatch is classified in ruled_v1.proto; fields with `visibility`
/// are cleared recursively, including fields introduced by future protocol changes.
void clearRuledFieldsByVisibility(google::protobuf::Message *message, ruled::v1::FieldVisibility visibility);

#endif
