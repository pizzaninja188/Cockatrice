#include "ruled_replacement_picker.h"

#include "../abstract_game.h"
#include "../game_scene.h"
#include "../player/player.h"
#include "../zones/view_zone.h"
#include "../zones/view_zone_widget.h"
#include "ruled_client_state.h"

#include <libcockatrice/protocol/pb/serverinfo_card.pb.h>
#include <libcockatrice/utility/zone_names.h>

RuledReplacementPicker::RuledReplacementPicker(AbstractGame *game,
                                               GameScene *scene,
                                               RuledClientState *state,
                                               QObject *parent)
    : QObject(parent), game(game), scene(scene), state(state)
{
    connect(state, &RuledClientState::replacementEffectUiChanged, this, &RuledReplacementPicker::refresh);
    connect(state, &RuledClientState::engineCommandPendingUiChanged, this, &RuledReplacementPicker::refresh);
    connect(state, &RuledClientState::sessionReset, this, &RuledReplacementPicker::close);
}

RuledReplacementPicker::~RuledReplacementPicker()
{
    close();
}

void RuledReplacementPicker::close()
{
    if (view) {
        view->getZone()->getLogic()->setProperty("ruledReplacementRevision", QVariant{});
        view->close();
        view = nullptr;
    }
    renderedRevision = 0;
}

void RuledReplacementPicker::refresh()
{
    if (!game || !scene || !state || state->reveals.windowsSuppressed() || !state->hasPendingReplacementEffect()) {
        close();
        return;
    }
    const int localId = game->getPlayerManager()->getLocalPlayerId();
    auto *player = game->getPlayerManager()->getPlayers().value(localId, nullptr);
    auto *scaffold = player ? player->getZones().value(ZoneNames::DECK) : nullptr;
    if (!scaffold)
        return;
    if (!view || renderedRevision != state->pendingChoiceRevision) {
        QList<ServerInfo_Card> storage;
        for (const auto &image : state->replacementEffectImages()) {
            ServerInfo_Card card;
            card.set_id(image.tileIndex);
            card.set_name(image.cardName.toStdString());
            card.set_annotation(image.annotation.toStdString());
            card.set_face_down(false);
            storage.append(std::move(card));
        }
        QList<const ServerInfo_Card *> cards;
        for (const auto &card : storage)
            cards.append(&card);
        if (!view) {
            view = new ZoneViewWidget(player, scaffold, -1, true, false, cards, false, false, true, false);
            scene->addItem(view);
            view->setPos(400, 300);
        } else {
            const QSizeF size = view->size();
            view->getZone()->getLogic()->clearContents();
            view->getZone()->initializeCards(cards);
            view->resize(size);
        }
        renderedRevision = state->pendingChoiceRevision;
        view->getZone()->getLogic()->setProperty("ruledReplacementPicker", true);
        view->getZone()->getLogic()->setProperty("ruledReplacementRevision", QVariant::fromValue(renderedRevision));
    }
    // Keep the window movable while a command is pending, but do not offer another selection.
    view->getZone()->setEnabled(!state->pendingChoice->replacementSubmitting && !state->isEngineCommandPending());
    view->setWindowTitle(state->pendingChoice->replacementSubmitting ? tr("Applying replacement effect…")
                                                                     : tr("Choose the next replacement effect"));
    view->setToolTip(state->pendingChoice->promptText);
}
