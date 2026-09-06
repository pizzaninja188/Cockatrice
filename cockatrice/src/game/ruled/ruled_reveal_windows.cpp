#include "ruled_reveal_windows.h"

#include "../abstract_game.h"
#include "../game_scene.h"
#include "../player/player.h"
#include "../zones/view_zone.h"
#include "../zones/view_zone_widget.h"
#include "ruled_client_state.h"

#include <libcockatrice/protocol/pb/serverinfo_card.pb.h>
#include <libcockatrice/utility/zone_names.h>

RuledRevealWindows::RuledRevealWindows(AbstractGame *_game,
                                       GameScene *_scene,
                                       RuledClientState *_state,
                                       QObject *parent)
    : QObject(parent), game(_game), scene(_scene), state(_state)
{
    connect(&state->reveals, &RuledRevealState::changed, this, &RuledRevealWindows::refresh);
}

RuledRevealWindows::~RuledRevealWindows()
{
    for (auto view : windows) {
        if (view) {
            disconnect(view, nullptr, this, nullptr);
            view->close();
        }
    }
}

void RuledRevealWindows::refresh()
{
    if (!game || !scene || !state)
        return;
    const auto &entries = state->reveals.entries();
    for (const auto &id : windows.keys()) {
        if (!entries.contains(id) || state->reveals.windowsSuppressed()) {
            auto view = windows.take(id);
            if (view) {
                disconnect(view, nullptr, this, nullptr);
                view->close();
            }
        }
    }
    if (state->reveals.windowsSuppressed())
        return;
    int position = 0;
    for (const QString &id : state->reveals.order()) {
        const auto &entry = entries[id];
        Player *owner = game->getPlayerManager()->getPlayers().value(entry.owner, nullptr);
        // The hand is only a layout scaffold; the engine snapshot supplies all display data.
        auto *scaffold = owner ? owner->getZones().value(ZoneNames::HAND) : nullptr;
        if (!scaffold)
            continue;
        QString zone;
        switch (entry.sourceZone) {
            case ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_HAND:
                zone = tr("hand");
                break;
            case ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY:
                zone = tr("library");
                break;
            case ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_GRAVEYARD:
                zone = tr("graveyard");
                break;
            case ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_BATTLEFIELD:
                zone = tr("battlefield");
                break;
            case ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_EXILE:
                zone = tr("exile");
                break;
            case ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_STACK:
                zone = tr("stack");
                break;
            default:
                zone = tr("cards");
                break;
        }
        const bool interactive =
            entry.phase == RuledRevealState::Phase::Choice && entry.choiceCardIds.size() == entry.cards.size();
        QList<ServerInfo_Card> storage;
        for (int i = 0; i < entry.cards.size(); ++i) {
            ServerInfo_Card card;
            card.set_name(entry.cards[i].name.toStdString());
            card.set_id(interactive ? entry.choiceCardIds[i] : -1 - i);
            card.set_face_down(false);
            card.set_annotation((entry.phase == RuledRevealState::Phase::Completed
                                     ? tr("Revealed earlier from %1").arg(zone)
                                     : tr("Revealed from %1").arg(zone))
                                    .toStdString());
            storage.append(std::move(card));
        }
        QList<const ServerInfo_Card *> cards;
        for (const auto &card : storage)
            cards.append(&card);
        auto view = windows.value(id);
        if (!view) {
            view = new ZoneViewWidget(owner, scaffold, -1, true, false, cards, false, false, true,
                                      entry.phase == RuledRevealState::Phase::Completed);
            windows.insert(id, view);
            scene->addItem(view);
            view->setPos(340 + (position % 4) * 40, 80 + (position % 4) * 40);
            connect(view, &ZoneViewWidget::closePressed, this, [this, id](ZoneViewWidget *) {
                windows.remove(id);
                if (state)
                    state->reveals.dismiss(id);
            });
        } else {
            view->getZone()->getLogic()->clearContents();
            view->getZone()->initializeCards(cards);
        }
        // Fork-owned lifecycle control through the single friend seam, without adding ruled
        // behavior to the generic zone widget. Resolution completion enables local dismissal.
        view->closeable = entry.phase == RuledRevealState::Phase::Completed;
        view->setWindowFlags(view->closeable ? view->windowFlags() | Qt::WindowCloseButtonHint
                                             : view->windowFlags() & ~Qt::WindowCloseButtonHint);
        view->getZone()->getLogic()->setProperty("ruledRevealId", interactive ? id : QString{});
        view->getZone()->getLogic()->setProperty("ruledRevealSnapshot", true);
        QString title = tr("%1's revealed %2").arg(owner->getPlayerInfo()->getName(), zone);
        if (!entry.sourceDescription.isEmpty())
            title += tr(" — %1").arg(entry.sourceDescription);
        view->setWindowTitle(title);
        ++position;
    }
}
