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

class AbstractGame;
class ArrowTarget;
class CardItem;
class Player;
class RuledClientState;

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
/// Arrow endpoint for a spell/ability target: a player seat, a stack item, or a permanent.
[[nodiscard]] ArrowTarget *resolveSpellTargetItem(AbstractGame *game, RuledClientState *state, quint32 targetOid);

// ---------------------------------------------------------------------------------------
// Clicked hand card → engine hand slot.
// ---------------------------------------------------------------------------------------
/// Maps a clicked hand card to an engine hand index given a precomputed set of legal slots
/// (e.g. the slots for a particular split-card face name).
[[nodiscard]] int engineHandIndexFromLegalSlots(const RuledClientState *state,
                                                const CardItem *card,
                                                const QList<int> &sortedLegalHandIndices);
[[nodiscard]] int resolveSpellCastHandIndex(const RuledClientState *state, const CardItem *card);
[[nodiscard]] int resolveLandPlayHandIndex(const RuledClientState *state, const CardItem *card);
[[nodiscard]] int resolveCleanupDiscardHandIndex(const RuledClientState *state, const CardItem *card);
[[nodiscard]] int resolveOpeningBottomHandIndex(const RuledClientState *state, const CardItem *card);

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
