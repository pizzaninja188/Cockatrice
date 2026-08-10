/**
 * @file ruled_actions.h
 * @ingroup GameLogic
 * @brief Ruled-mode click interpretation and CardItem lookup, kept out of upstream files.
 *
 * The functions here are the fork's side of upstream call sites: they return `bool consumed`
 * so the upstream code reads as a one-line guard —
 * `if (RuledActions::tryHandleCombatClick(this)) return;` — instead of an inlined ruled block.
 *
 * Unlike `RuledClientState` / `RuledEventDispatcher`, this unit knowingly depends on the Qt game
 * objects (`AbstractGame`, `Player`, `CardItem`): resolving a click *is* a UI concern. It is the
 * bridge between the two worlds, and the home of the single `isRuledGame()` predicate that
 * replaces the verbatim `game->getGameMetaInfo()->proto().ruled_game()` chains.
 */

#ifndef COCKATRICE_RULED_ACTIONS_H
#define COCKATRICE_RULED_ACTIONS_H

#include <QList>
#include <QString>
#include <QtGlobal>
#include <functional>

class AbstractGame;
class ArrowTarget;
class CardItem;
class Player;
class RuledClientState;
/// Defined in ruled_client_state.h; forward-declared (fixed underlying type) to keep this header
/// free of that include.
enum class RuledTargetItemKind : int;
/// Defined in ruled_client_state.h; declared opaquely here so this header stays proto-free.
namespace ruled::v1
{
enum HandActionKind : int;
class RuledCommand;
}
using RuledHandActionKind = ruled::v1::HandActionKind;

namespace RuledActions
{

// ---------------------------------------------------------------------------------------
// Mode predicate — the one place that reads the ruled_game flag.
// ---------------------------------------------------------------------------------------
[[nodiscard]] bool isRuledGame(const AbstractGame *game);
[[nodiscard]] bool isRuledGameForPlayer(const Player *player);
[[nodiscard]] bool isRuledGameForCard(const CardItem *card);

/// The ruled view model for this game, or nullptr when the game is freeform / unavailable.
[[nodiscard]] RuledClientState *stateFor(const AbstractGame *game);
[[nodiscard]] RuledClientState *stateForCard(const CardItem *card);
/// True only while a server-authoritative gameplay command is awaiting completion. UI-only policy
/// and combat-preview messages do not enter this state.
[[nodiscard]] bool gameplayInputLocked(const AbstractGame *game);

/// Tell the view model which graveyard OIDs the pending cast of `handSlot`/`faceIndex` may target,
/// so `TabGame` can open the right players' graveyard views. Pass `handSlot < 0` to retract the
/// hint (no cast pending). No-op outside a ruled game.
///
/// Casting Raise Dead used to require the player to open their own graveyard by hand — the view
/// auto-opened only for a pending *trigger*. Reanimate makes that worse, because the card may sit
/// in an opponent's graveyard.
void updateGraveyardTargetHint(const Player *player, int handSlot, int faceIndex);

/// Send a ready-built ruled command that does not originate in the view model — today only the
/// dev console, whose input is text rather than a click or an engine event.
///
/// Every other ruled send goes through a `RuledClientState` slot, and `GameEventHandler` keeps its
/// `RuledClientHost` overrides private so that stays true. Routing the console through here keeps
/// its transport among the other ruled sends instead of putting one in an upstream file.
/// No-op outside a ruled game.
void sendRuledCommand(const AbstractGame *game, const ruled::v1::RuledCommand &command);

/// As `sendRuledCommand`, but reports whether the server accepted it. The dev console uses this:
/// the engine legitimately refuses commands (moving a card you do not own, conjuring into a zone
/// that has no minting path), and without an ack those look like nothing happened at all.
void sendRuledCommandExpectingAck(const AbstractGame *game,
                                  const ruled::v1::RuledCommand &command,
                                  std::function<void(bool accepted)> onFinished);

// ---------------------------------------------------------------------------------------
// CardItem lookup by engine identity.
// ---------------------------------------------------------------------------------------
/// Parses a "P/T" display string. Returns false for empty, malformed, or `*` values.
[[nodiscard]] bool parseCreaturePt(const QString &pt, int *outPower, int *outToughness);

/// Battlefield CardItem for an engine ObjectId, via the identity maps then a full table scan.
[[nodiscard]] CardItem *findBattlefieldCardItemByEngineOid(AbstractGame *game, quint32 engineOid);
/// Stack CardItem for a Server_Card.id — prefers the copy visible in an open stack window.
[[nodiscard]] CardItem *findStackCardItemByServerCardId(AbstractGame *game, int serverCardId);
/// Stack CardItem for an engine ObjectId (id map first, then a scan of every stack zone).
[[nodiscard]] CardItem *findStackCardItemByEngineOid(AbstractGame *game, quint32 stackOid);
/// Graveyard CardItem for an engine ObjectId: the copy in an open zone view if there is one, else
/// the card in the pile (which sits at the pile's own position, so an arrow to it points at the
/// graveyard rather than at an invisible card). Null when the oid is not in any graveyard.
[[nodiscard]] CardItem *findGraveyardCardItemByEngineOid(AbstractGame *game, quint32 engineOid);
/// Where `targetOid` currently lives, in the priority order a target is chosen: seat, stack,
/// graveyard, battlefield. Called once per target, when its arrow is first drawn; the answer is
/// latched in `RuledClientState::stackTargetKindByStackAndTargetOid` because a later zone change
/// makes the target a different object (CR 608.2b), not a moved one.
[[nodiscard]] RuledTargetItemKind classifySpellTargetItem(AbstractGame *game,
                                                          RuledClientState *state,
                                                          quint32 targetOid);
/// Arrow endpoint for a spell/ability target, resolved *only* within `kind`. Null when the target
/// is no longer there — which is the signal to drop the arrow rather than re-point it.
[[nodiscard]] ArrowTarget *
resolveSpellTargetItem(AbstractGame *game, RuledClientState *state, quint32 targetOid, RuledTargetItemKind kind);

// ---------------------------------------------------------------------------------------
// Clicked hand card → engine hand slot.
// ---------------------------------------------------------------------------------------
/// Maps a clicked hand card to an engine hand index given a precomputed set of legal slots
/// (e.g. the slots for a particular split-card face name).
[[nodiscard]] int engineHandIndexFromLegalSlots(const RuledClientState *state,
                                                const CardItem *card,
                                                const QList<int> &sortedLegalHandIndices);
/// Engine hand slot a clicked hand card would use for `kind`, or -1 when the engine does not offer
/// that action on it. The only click→slot entry point; a new hand mechanic needs no new function.
[[nodiscard]] int
resolveHandActionIndex(const RuledClientState *state, RuledHandActionKind kind, const CardItem *card);
/// Engine object id for a clicked public-zone card, using the server-injected zone identity map.
/// Returns 0 for non-graveyard/exile cards or when the current batch has no mapping.
[[nodiscard]] quint32 resolvePublicZoneObjectId(const RuledClientState *state, const CardItem *card);

/// True when `card` lives in the zone the active tier-3 resolution pick is drawing candidates
/// from. **Every id-keyed pick query must be gated on this**: a candidate's `Server_Card.id` is
/// only meaningful within its own zone. The library-search and revealed popups carry synthetic
/// sequential ids (0, 1, 2, … — see the relay's redaction pass, which has no real card ids to
/// hand out for cards in a hidden zone), and those collide with the genuine ids of cards on the
/// battlefield and in hand. Returns false when no pick is active.
[[nodiscard]] bool isResolutionPickZoneCard(const RuledClientState *state, const CardItem *card);

// ---------------------------------------------------------------------------------------
// Click interpretation. Each returns true when the click was consumed.
// ---------------------------------------------------------------------------------------
/// Engine-authoritative creature-ness on the battlefield (falls back to Oracle for freeform).
[[nodiscard]] bool isCombatEligibleCreature(const CardItem *card);
bool tryHandleCombatClick(CardItem *card);
bool tryHandleCombatRightClick(CardItem *card);
/// True when a single left-click on this hand card is a legal ruled play (land or spell), so the
/// "double click to play" setting is bypassed.
[[nodiscard]] bool isSingleClickPlayLegal(const CardItem *card);

// ---------------------------------------------------------------------------------------
// Local player's pending-cast selection (owned by PlayerActions), read by the painters.
// ---------------------------------------------------------------------------------------
[[nodiscard]] bool isSelectedSpellTarget(const AbstractGame *game, quint32 oid);
[[nodiscard]] bool isPlayerSelectedAsSpellTarget(const AbstractGame *game, int playerId);
[[nodiscard]] bool isSpellDamageAllocationMode(const AbstractGame *game);
[[nodiscard]] bool isSpellDamageAllocationDisplayActive(const AbstractGame *game);
[[nodiscard]] int spellDamageAllocationForOid(const AbstractGame *game, quint32 oid);
[[nodiscard]] int spellDamageAllocationForPlayerId(const AbstractGame *game, int playerId);

} // namespace RuledActions

#endif // COCKATRICE_RULED_ACTIONS_H
