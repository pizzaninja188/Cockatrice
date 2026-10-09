#include "ruled_large_choice_picker.h"

#include <QAbstractItemView>
#include <QLineEdit>
#include <QListWidget>
#include <QListWidgetItem>
#include <QPushButton>
#include <QVBoxLayout>

RuledLargeChoicePicker::RuledLargeChoicePicker(QWidget *parent) : QWidget(parent)
{
    auto *layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(4);

    searchEdit = new QLineEdit(this);
    searchEdit->setObjectName(QStringLiteral("ruledLargeChoiceSearch"));
    layout->addWidget(searchEdit);

    optionList = new QListWidget(this);
    optionList->setObjectName(QStringLiteral("ruledLargeChoiceList"));
    optionList->setSelectionMode(QAbstractItemView::SingleSelection);
    optionList->setMaximumHeight(180);
    layout->addWidget(optionList);

    confirmButton = new QPushButton(this);
    confirmButton->setObjectName(QStringLiteral("ruledLargeChoiceConfirm"));
    confirmButton->setEnabled(false);
    layout->addWidget(confirmButton);

    connect(searchEdit, &QLineEdit::textChanged, this, &RuledLargeChoicePicker::applyFilter);
    connect(optionList, &QListWidget::currentItemChanged, this,
            [this](QListWidgetItem *, QListWidgetItem *) { updateConfirmEnabled(); });
    connect(confirmButton, &QPushButton::clicked, this, &RuledLargeChoicePicker::confirmSelection);
    retranslateUi();
}

void RuledLargeChoicePicker::setOptions(const QVector<Option> &options)
{
    clear();
    optionList->setUpdatesEnabled(false);
    for (const auto &option : options) {
        auto *item = new QListWidgetItem(option.label, optionList);
        item->setData(Qt::UserRole, option.index);
        if (!option.enabled) {
            item->setFlags(item->flags() & ~Qt::ItemIsEnabled & ~Qt::ItemIsSelectable);
        }
    }
    optionList->setUpdatesEnabled(true);
    applyFilter(searchEdit->text());
    updateConfirmEnabled();
}

void RuledLargeChoicePicker::clear()
{
    searchEdit->clear();
    optionList->clear();
    confirmButton->setEnabled(false);
}

void RuledLargeChoicePicker::retranslateUi()
{
    searchEdit->setPlaceholderText(tr("Search choices"));
    confirmButton->setText(tr("Confirm"));
}

void RuledLargeChoicePicker::applyFilter(const QString &text)
{
    for (int row = 0; row < optionList->count(); ++row) {
        auto *item = optionList->item(row);
        item->setHidden(!item->text().contains(text, Qt::CaseInsensitive));
    }
    if (const auto *current = optionList->currentItem(); current && current->isHidden()) {
        optionList->setCurrentItem(nullptr);
        optionList->clearSelection();
    }
    updateConfirmEnabled();
}

void RuledLargeChoicePicker::updateConfirmEnabled()
{
    const auto *item = optionList->currentItem();
    confirmButton->setEnabled(item && (item->flags() & Qt::ItemIsEnabled));
}

void RuledLargeChoicePicker::confirmSelection()
{
    const auto *item = optionList->currentItem();
    if (item && (item->flags() & Qt::ItemIsEnabled)) {
        emit optionRequested(item->data(Qt::UserRole).toInt());
    }
}
