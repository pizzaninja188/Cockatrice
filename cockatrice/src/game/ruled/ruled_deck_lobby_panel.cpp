#include "ruled_deck_lobby_panel.h"

#include "../../interface/widgets/cards/deck_preview_card_picture_widget.h"

#include <QHBoxLayout>
#include <QLabel>
#include <QVBoxLayout>
#include <libcockatrice/card/database/card_database_manager.h>
#include <libcockatrice/card/database/card_database_querier.h>
#include <libcockatrice/deck_list/deck_list.h>
#include <libcockatrice/protocol/ruled_deck_validation_report.h>

RuledDeckLobbyPanel::RuledDeckLobbyPanel(QWidget *parent) : QWidget(parent)
{
    auto *layout = new QVBoxLayout(this);
    layout->setContentsMargins(11, 0, 11, 0);
    status = new QLabel(this);
    status->setTextFormat(Qt::PlainText);
    status->setWordWrap(true);
    layout->addWidget(status);
    commanders = new QWidget(this);
    cards = new QHBoxLayout(commanders);
    cards->setContentsMargins(0, 0, 0, 0);
    layout->addWidget(commanders);
    commanders->hide();
}
void RuledDeckLobbyPanel::setDeck(const DeckList &deck)
{
    status->setText(tr("The server checks this deck's format legality and engine support before readiness."));
    status->setToolTip({});
    while (auto *item = cards->takeAt(0)) {
        delete item->widget();
        delete item;
    }
    const auto declarations = deck.getCommanders();
    commanders->setVisible(!declarations.isEmpty());
    if (!declarations.isEmpty()) {
        cards->addWidget(new QLabel(tr("Commanders:"), commanders));
    }
    for (const auto &commander : declarations) {
        auto *picture = new DeckPreviewCardPictureWidget(commanders, true);
        picture->setFixedSize(72, 102);
        picture->setCard(CardDatabaseManager::query()->getCard(commander));
        picture->setOverlayText(commander.name);
        picture->setToolTip(commander.name);
        cards->addWidget(picture);
    }
    cards->addStretch();
}
void RuledDeckLobbyPanel::showReport(const ruled::v1::DeckValidationReport &report, bool rejected)
{
    const auto detail = RuledDeckValidationReport::describe(report);
    auto headline = detail.section('\n', 0, 0);
    if (rejected) {
        headline = tr("Replacement rejected; the previous deck is kept. ") + headline;
    }
    if (!report.catalog_source_date().empty()) {
        headline += tr(" Catalogue: %1.").arg(QString::fromStdString(report.catalog_source_date()));
    }
    if (!report.notice().empty()) {
        headline += " " + QString::fromStdString(report.notice());
    }
    status->setText(headline);
    status->setToolTip(detail);
}
