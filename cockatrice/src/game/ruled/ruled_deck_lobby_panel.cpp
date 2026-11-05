#include "ruled_deck_lobby_panel.h"

#include <QSizePolicy>
#include <libcockatrice/protocol/ruled_deck_validation_report.h>

RuledDeckLobbyPanel::RuledDeckLobbyPanel(QWidget *parent) : QLabel(parent)
{
    setContentsMargins(11, 0, 11, 0);
    setTextFormat(Qt::PlainText);
    setWordWrap(true);
    auto policy = sizePolicy();
    policy.setVerticalPolicy(QSizePolicy::Maximum);
    setSizePolicy(policy);
}
void RuledDeckLobbyPanel::resetReport()
{
    setText(tr("The server checks this deck's format legality and engine support before readiness."));
    setToolTip({});
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
    setText(headline);
    setToolTip(detail);
}
