#include "ruled_interaction_ui.h"

#include "../../game_graphics/board/card_item.h"
#include "../../game_graphics/player/player_target.h"
#include "ruled_actions.h"
#include "ruled_card_display_identity.h"
#include "ruled_player_actions.h"

#include <libcockatrice/game/abstract_game.h>
#include <libcockatrice/game/player/player_logic.h>
#include <libcockatrice/game/player/player_manager.h>
#include <libcockatrice/game/zones/card_zone_logic.h>

namespace
{
RuledPlayerActions *localActions(PlayerLogic *owner)
{
    auto *game = owner ? owner->getGame() : nullptr;
    if (!game || !RuledActions::isRuledGame(game)) {
        return nullptr;
    }
    auto *players = game->getPlayerManager();
    return players ? RuledActions::actions(players->getPlayers().value(players->getLocalPlayerId(), nullptr)) : nullptr;
}

RuledInteractionVisual selectionVisual(RuledTargetClickEligibility eligibility)
{
    return {eligibility == RuledTargetClickEligibility::Illegal, eligibility == RuledTargetClickEligibility::Legal};
}

template <typename Item> void connectItem(Item *item)
{
    auto *owner = item->getOwner();
    auto *state = RuledActions::stateFor(owner ? owner->getGame() : nullptr);
    if (!state) {
        return;
    }
    const auto refresh = [item] { item->update(); };
    QObject::connect(state, &RuledClientState::legalActionsChanged, item, refresh);
    QObject::connect(state, &RuledClientState::targetZoneViewsChanged, item, refresh);
    QObject::connect(state, &RuledClientState::combatStateChanged, item, refresh);
    QObject::connect(state, &RuledClientState::sessionReset, item, refresh);
    QObject::connect(state, &RuledClientState::localActionAuthorityRevoked, item, refresh);
}
} // namespace

void RuledInteractionUi::connectCard(CardItem *card)
{
    connectItem(card);
}

void RuledInteractionUi::connectPlayer(PlayerTarget *player)
{
    connectItem(player);
}

void RuledInteractionUi::paintCard(CardItem *card, QPainter *painter)
{
    auto *actions = localActions(card->getOwner());
    if (!actions || !ruledCardSurfaceHasPhysicalIdentity(card->getZone())) {
        return;
    }
    RuledInteractionPainter::paint(painter, card->shape(), selectionVisual(actions->ruledCardTargetEligibility(card)));
}

void RuledInteractionUi::paintPlayer(PlayerTarget *player, QPainter *painter, const QRectF &avatar)
{
    auto *actions = localActions(player->getOwner());
    if (!actions) {
        return;
    }
    QPainterPath shape;
    shape.addRoundedRect(avatar, 4, 4);
    RuledInteractionPainter::paint(painter, shape,
                                   selectionVisual(actions->ruledPlayerTargetEligibility(player->getOwner())));
}
