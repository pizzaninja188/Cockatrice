#include "ruled_interaction_ui.h"

#include "../../client/settings/cache_settings.h"
#include "../../game_graphics/board/arrow_item.h"
#include "../../game_graphics/board/card_item.h"
#include "../../game_graphics/game_scene.h"
#include "../../game_graphics/player/player_target.h"
#include "ruled_actions.h"
#include "ruled_card_display_identity.h"
#include "ruled_interaction_focus.h"
#include "ruled_player_actions.h"

#include <QTimer>
#include <libcockatrice/game/abstract_game.h>
#include <libcockatrice/game/board/card_state.h>
#include <libcockatrice/game/player/player_info.h>
#include <libcockatrice/game/player/player_logic.h>
#include <libcockatrice/game/player/player_manager.h>
#include <libcockatrice/game/zones/card_zone_logic.h>
#include <libcockatrice/settings/cards_display_settings.h>
#include <libcockatrice/utility/zone_names.h>

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

bool selectionInProgress(RuledPlayerActions *actions, const RuledClientState &state, PlayerLogic *owner)
{
    return actions && (state.pendingChoice || state.payment.active ||
                       actions->ruledPlayerTargetEligibility(owner) != RuledTargetClickEligibility::NotTargeting ||
                       actions->isAwaitingRuledSpellCostSelection() || actions->isAwaitingRuledAbilityCostSelection() ||
                       actions->isAwaitingRuledCastCostObject() || actions->isAwaitingRuledCastCostOption());
}

quint32 physicalOid(const RuledClientState &state, const CardItem *card)
{
    if (!card || !card->getOwner() || !card->getZone() || !ruledCardSurfaceHasPhysicalIdentity(card->getZone())) {
        return 0;
    }
    const auto &zone = card->getZone()->getName();
    if (zone == ZoneNames::GRAVE || zone == ZoneNames::EXILE || zone == ZoneNames::COMMAND) {
        return RuledActions::resolvePublicZoneObjectId(&state, card);
    }
    return state.engineOidForCardId(card->getOwner()->getPlayerInfo()->getId(), card->getId());
}

RuledInteractionFocus hoverFocus(PlayerLogic *owner)
{
    auto *game = owner ? owner->getGame() : nullptr;
    auto *state = RuledActions::stateFor(game);
    auto *scene = RuledActions::scene(game);
    auto *hovered = scene ? scene->getHoveredCard() : nullptr;
    if (!state || !RuledActions::isRuledGame(game) || !hovered || !hovered->getIsHovered() || !hovered->getZone() ||
        selectionInProgress(localActions(owner), *state, owner)) {
        return {};
    }
    const quint32 oid = physicalOid(*state, hovered);
    if (!oid) {
        return {};
    }
    if (hovered->getZone()->getName() == ZoneNames::STACK) {
        return ruledStackFocus(*state, oid);
    }
    if (hovered->getZone()->getName() != ZoneNames::TABLE) {
        return {};
    }
    auto focus = ruledCombatFocus(*state, oid, game->getPlayerManager()->getLocalPlayerId());
    // Attachment edges are physical relationships already maintained by the relay.
    auto attached = hovered->getState()->getAttachedCards();
    if (auto *host = hovered->getState()->getAttachedTo()) {
        attached.append(host);
    }
    for (auto *attachment : attached) {
        auto *view = RuledActions::cardView(attachment);
        const auto related = physicalOid(*state, view);
        if (related && view->getZone()->getName() == ZoneNames::TABLE && !state->isEngineOidPhasedOut(related)) {
            focus.active = true;
            focus.battlefield.insert(oid);
            focus.battlefield.insert(related);
        }
    }
    return focus;
}

RuledTargetClickEligibility cardSelection(RuledPlayerActions *actions, CardItem *card)
{
    using E = RuledTargetClickEligibility;
    if (!actions || !card || !card->getOwner() || !card->getZone()) {
        return E::NotTargeting;
    }
    auto *state = RuledActions::stateFor(card->getOwner()->getGame());
    if (!state) {
        return E::NotTargeting;
    }
    const auto candidate = [](bool legal) { return legal ? E::Legal : E::Illegal; };
    if (state->isResolutionHandPickActive()) {
        return candidate(RuledActions::isResolutionPickZoneCard(state, card) &&
                         state->isResolutionHandPickCardSelectable(card->getId()));
    }
    if (!ruledCardSurfaceHasPhysicalIdentity(card->getZone())) {
        if (state->hasPendingReplacementEffect()) {
            return candidate(RuledActions::isReplacementEffectCard(card) &&
                             card->getZone()->property("ruledReplacementRevision").toULongLong() ==
                                 state->pendingChoiceRevision);
        }
        if (state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::TriggerOrder)) {
            return candidate(state->isTriggerOrderPickCard(card->getId()));
        }
        return E::NotTargeting;
    }
    const auto target = actions->ruledCardTargetEligibility(card);
    if (target != E::NotTargeting) {
        return target;
    }
    const auto cost = actions->ruledCardCostEligibility(card);
    if (cost != E::NotTargeting) {
        return cost;
    }
    if (state->localPlayerMustCleanupDiscard() ||
        state->openingUiKind == RuledClientState::RuledOpeningUiKind::BottomLibrary) {
        const auto *owner = card->getOwner()->getPlayerInfo();
        return candidate(owner->getLocal() && card->getZone()->getName() == ZoneNames::HAND &&
                         state->engineHandSlotForServerCard(owner->getId(), card->getId()) >= 0);
    }
    if (state->payment.active) {
        const auto oid = physicalOid(*state, card);
        bool legal = false;
        if (oid != 0 && card->getZone()->getName() == ZoneNames::TABLE) {
            const auto *offered = state->payment.candidate(oid);
            legal = state->payment.selected(oid) ||
                    (offered && offered->options_size() > 0 &&
                     offered->object().zone_change_generation() == state->abilitySourceGeneration(oid));
        }
        // Mana activations stay available in the same payment window as object contributions.
        for (const auto &ability : state->activatedAbilitiesForOid(oid)) {
            legal |= ability && ability->activatable && ability->isManaAbility();
        }
        return candidate(legal);
    }
    return E::NotTargeting;
}

bool relatedCard(const RuledInteractionFocus &focus, const CardItem *card)
{
    auto *owner = card ? card->getOwner() : nullptr;
    auto *state = RuledActions::stateFor(owner ? owner->getGame() : nullptr);
    if (!state || !card->getZone()) {
        return false;
    }
    const auto oid = physicalOid(*state, card);
    const auto &zone = card->getZone()->getName();
    return zone == ZoneNames::TABLE   ? focus.battlefield.contains(oid)
           : zone == ZoneNames::STACK ? focus.stack.contains(oid)
           : zone == ZoneNames::GRAVE ? focus.graveyard.contains(oid)
                                      : false;
}

bool relatedEndpoint(const RuledInteractionFocus &focus, ArrowTarget *target)
{
    if (auto *card = qobject_cast<CardItem *>(target)) {
        return relatedCard(focus, card);
    }
    if (auto *player = qobject_cast<PlayerTarget *>(target)) {
        return player->getOwner() && focus.players.contains(player->getOwner()->getPlayerInfo()->getId());
    }
    return false;
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
    QObject::connect(state, &RuledClientState::stackOrderChanged, item, refresh);
    QObject::connect(state, &RuledClientState::battlefieldMapUpdated, item, refresh);
    QObject::connect(state, &RuledClientState::spellTargetSelectionChanged, item, refresh);
    QObject::connect(state, &RuledClientState::resolutionHandPickUiChanged, item, refresh);
    QObject::connect(state, &RuledClientState::resolutionPaymentUiChanged, item, refresh);
    QObject::connect(state, &RuledClientState::resolutionCostSelectionChanged, item, refresh);
    QObject::connect(state, &RuledClientState::openingUiChanged, item, refresh);
    QObject::connect(state, &RuledClientState::cleanupDiscardUiChanged, item, refresh);
    QObject::connect(state, &RuledClientState::replacementEffectUiChanged, item, refresh);
    QObject::connect(&SettingsCache::instance().cardsDisplay(),
                     &CardsDisplaySettings::ruledInteractionAppearanceChanged, item, refresh);
}
} // namespace

void RuledInteractionUi::connectCard(CardItem *card)
{
    connectItem(card);
    auto *owner = card->getOwner();
    auto *scene = RuledActions::scene(owner ? owner->getGame() : nullptr);
    if (scene && RuledActions::isRuledGameForCard(card)) {
        if (!scene->property("ruledInteractionScene").toBool()) {
            QObject::connect(&SettingsCache::instance().cardsDisplay(),
                             &CardsDisplaySettings::ruledInteractionAppearanceChanged, scene,
                             [scene] { refreshScene(scene); });
        }
        scene->setProperty("ruledInteractionScene", true);
        QObject::connect(card, &QObject::destroyed, scene,
                         [scene] { QTimer::singleShot(0, scene, [scene] { refreshScene(scene); }); });
    }
}

void RuledInteractionUi::connectPlayer(PlayerTarget *player)
{
    connectItem(player);
}

void RuledInteractionUi::paintCard(CardItem *card, QPainter *painter)
{
    if (!RuledActions::isRuledGameForCard(card)) {
        return;
    }
    auto *actions = localActions(card->getOwner());
    const auto eligibility = cardSelection(actions, card);
    auto visual = selectionVisual(eligibility);
    if (eligibility == RuledTargetClickEligibility::NotTargeting) {
        const auto focus = hoverFocus(card->getOwner());
        if (focus.active) {
            visual.related = relatedCard(focus, card);
            visual.dimmed = !visual.related && SettingsCache::instance().cardsDisplay().getRuledHoverDimming();
        } else if (actions && ruledCardSurfaceHasPhysicalIdentity(card->getZone())) {
            const auto *state = RuledActions::stateFor(card->getOwner()->getGame());
            if (state && !selectionInProgress(actions, *state, card->getOwner())) {
                visual = selectionVisual(ruledCombatSelection(
                    *state, card->getOwner()->getGame()->getPlayerManager()->getLocalPlayerId(),
                    card->getZone()->getName() == ZoneNames::TABLE ? physicalOid(*state, card) : 0));
            }
        }
    }
    RuledInteractionPainter::paint(painter, card->shape(), visual,
                                   SettingsCache::instance().cardsDisplay().getRuledDimPercent());
}

void RuledInteractionUi::paintPlayer(PlayerTarget *player, QPainter *painter, const QRectF &avatar)
{
    if (!RuledActions::isRuledGameForPlayer(player->getOwner())) {
        return;
    }
    auto *actions = localActions(player->getOwner());
    const auto eligibility =
        actions ? actions->ruledPlayerTargetEligibility(player->getOwner()) : RuledTargetClickEligibility::NotTargeting;
    auto visual = selectionVisual(eligibility);
    if (eligibility == RuledTargetClickEligibility::NotTargeting) {
        const auto focus = hoverFocus(player->getOwner());
        if (focus.active) {
            visual.related = focus.players.contains(player->getOwner()->getPlayerInfo()->getId());
            visual.dimmed = !visual.related && SettingsCache::instance().cardsDisplay().getRuledHoverDimming();
        }
    }
    QPainterPath shape;
    shape.addRoundedRect(avatar, 4, 4);
    RuledInteractionPainter::paint(painter, shape, visual,
                                   SettingsCache::instance().cardsDisplay().getRuledDimPercent());
}

void RuledInteractionUi::refreshScene(GameScene *scene)
{
    if (!scene || !scene->property("ruledInteractionScene").toBool()) {
        return;
    }
    // Player avatars use a device cache, so a scene update alone cannot invalidate their tint.
    for (auto *item : scene->items()) {
        item->update();
    }
}

qreal RuledInteractionUi::arrowOpacity(const ArrowItem *arrow)
{
    if (!SettingsCache::instance().cardsDisplay().getRuledHoverDimming()) {
        return 1.0;
    }
    auto *start = arrow->getStartItem();
    const auto focus = hoverFocus(start ? start->getOwner() : nullptr);
    return !focus.active || (relatedEndpoint(focus, start) && relatedEndpoint(focus, arrow->getTargetItem())) ? 1.0
                                                                                                              : 0.2;
}
