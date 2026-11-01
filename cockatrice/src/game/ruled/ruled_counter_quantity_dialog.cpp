#include "ruled_counter_quantity_dialog.h"

#include <QDialogButtonBox>
#include <QFormLayout>
#include <QLabel>
#include <QLineEdit>
#include <QPushButton>
#include <QSet>
#include <QVBoxLayout>

#include <algorithm>
#include <limits>

RuledCounterQuantityDialog::RuledCounterQuantityDialog(const Context &context, QWidget *parent)
    : QDialog(parent)
{
    setWindowTitle(tr("Choose counter quantities"));

    auto *layout = new QVBoxLayout(this);
    m_contextLabel = new QLabel(this);
    m_contextLabel->setObjectName("counterQuantityContextLabel");
    m_contextLabel->setWordWrap(true);
    layout->addWidget(m_contextLabel);

    m_rowsLayout = new QFormLayout;
    layout->addLayout(m_rowsLayout);

    m_buttonBox = new QDialogButtonBox(QDialogButtonBox::Apply | QDialogButtonBox::Cancel, this);
    m_buttonBox->setObjectName("counterQuantityButtonBox");
    m_applyButton = m_buttonBox->button(QDialogButtonBox::Apply);
    m_applyButton->setObjectName("counterQuantityApplyButton");
    m_applyButton->setDefault(true);
    m_buttonBox->button(QDialogButtonBox::Cancel)->setObjectName("counterQuantityCancelButton");
    layout->addWidget(m_buttonBox);

    connect(m_applyButton, &QPushButton::clicked, this, &RuledCounterQuantityDialog::applySelection);
    connect(m_buttonBox, &QDialogButtonBox::rejected, this, &RuledCounterQuantityDialog::reject);

    setContext(context);
}

void RuledCounterQuantityDialog::setContext(const Context &context)
{
    m_context = context;
    m_acceptedResult.reset();
    clearInputRows();

    const QString objectName = m_context.displayName.isEmpty() ? tr("this card") : m_context.displayName;
    m_contextLabel->setText(tr("Choose counter quantities for %1 (up to %2 total for this card).")
                                .arg(objectName)
                                .arg(m_context.remainingForThisObject));

    for (const CounterOption &option : m_context.offeredOptions) {
        auto *input = new QLineEdit(this);
        input->setObjectName(QStringLiteral("counterQuantityInput_") + QString::number(option.optionId));
        input->setText(QString::number(m_context.initialQuantities.value(option.optionId, 0)));
        const QString rowLabel = tr("%1 (available: %2)").arg(option.label).arg(option.availableCount);
        m_rowsLayout->addRow(rowLabel, input);
        m_inputRows.append({option, input});
        connect(input, &QLineEdit::textChanged, this, [this]() { refreshApplyEnabled(); });
    }

    refreshApplyEnabled();
}

std::optional<RuledCounterQuantityDialog::Result> RuledCounterQuantityDialog::acceptedQuantityResult() const
{
    return m_acceptedResult;
}

bool RuledCounterQuantityDialog::isResultCurrent(const Context &captured,
                                                 const Context &current,
                                                 const Result &result)
{
    if (!isWellFormedContext(captured) || !isWellFormedContext(current)) {
        return false;
    }
    if (captured.costIndex != current.costIndex || captured.objectId != current.objectId ||
        captured.zoneChangeGeneration != current.zoneChangeGeneration ||
        captured.remainingForThisObject != current.remainingForThisObject) {
        return false;
    }
    if (result.costIndex != captured.costIndex || result.objectId != captured.objectId ||
        result.zoneChangeGeneration != captured.zoneChangeGeneration) {
        return false;
    }
    if (!sameOfferedInventory(captured, current)) {
        return false;
    }

    QSet<quint32> selectedOptions;
    quint64 total = 0;
    for (const Selection &selection : result.selections) {
        if (selection.optionId == 0 || selection.count == 0 || selectedOptions.contains(selection.optionId)) {
            return false;
        }
        selectedOptions.insert(selection.optionId);

        bool foundCaptured = false;
        quint32 capturedAvailability = 0;
        for (const CounterOption &option : captured.offeredOptions) {
            if (option.optionId == selection.optionId) {
                foundCaptured = true;
                capturedAvailability = option.availableCount;
                break;
            }
        }
        bool foundCurrent = false;
        quint32 currentAvailability = 0;
        for (const CounterOption &option : current.offeredOptions) {
            if (option.optionId == selection.optionId) {
                foundCurrent = true;
                currentAvailability = option.availableCount;
                break;
            }
        }
        if (!foundCaptured || !foundCurrent || selection.count > capturedAvailability ||
            selection.count > currentAvailability) {
            return false;
        }
        if (total > std::numeric_limits<quint64>::max() - selection.count) {
            return false;
        }
        total += selection.count;
        if (total > captured.remainingForThisObject || total > current.remainingForThisObject) {
            return false;
        }
    }
    return true;
}

void RuledCounterQuantityDialog::reject()
{
    m_acceptedResult.reset();
    QDialog::reject();
}

bool RuledCounterQuantityDialog::isWellFormedContext(const Context &context)
{
    if (context.objectId == 0 || context.offeredOptions.isEmpty()) {
        return false;
    }

    QSet<quint32> optionIds;
    for (const CounterOption &option : context.offeredOptions) {
        if (option.optionId == 0 || option.availableCount == 0 || optionIds.contains(option.optionId)) {
            return false;
        }
        optionIds.insert(option.optionId);
    }

    return true;
}

bool RuledCounterQuantityDialog::sameOfferedInventory(const Context &left, const Context &right)
{
    if (left.offeredOptions.size() != right.offeredOptions.size()) {
        return false;
    }
    QHash<quint32, quint32> leftAvailability;
    for (const CounterOption &option : left.offeredOptions) {
        leftAvailability.insert(option.optionId, option.availableCount);
    }
    for (const CounterOption &option : right.offeredOptions) {
        const auto found = leftAvailability.constFind(option.optionId);
        if (found == leftAvailability.cend() || found.value() != option.availableCount) {
            return false;
        }
    }
    return true;
}

bool RuledCounterQuantityDialog::hasOnlyOfferedInitialQuantities(const Context &context)
{
    QSet<quint32> optionIds;
    for (const CounterOption &option : context.offeredOptions) {
        optionIds.insert(option.optionId);
    }
    for (auto it = context.initialQuantities.cbegin(); it != context.initialQuantities.cend(); ++it) {
        if (!optionIds.contains(it.key())) {
            return false;
        }
    }
    return true;
}

bool RuledCounterQuantityDialog::parseAsciiUint32(const QString &text, quint32 &value)
{
    if (text.isEmpty()) {
        return false;
    }

    quint64 parsed = 0;
    const quint64 maximum = std::numeric_limits<quint32>::max();
    for (const QChar character : text) {
        const ushort digitCharacter = character.unicode();
        if (digitCharacter < ushort('0') || digitCharacter > ushort('9')) {
            return false;
        }
        const quint64 digit = digitCharacter - ushort('0');
        if (parsed > (maximum - digit) / 10) {
            return false;
        }
        parsed = parsed * 10 + digit;
    }

    value = static_cast<quint32>(parsed);
    return true;
}

std::optional<RuledCounterQuantityDialog::Result> RuledCounterQuantityDialog::collectValidResult() const
{
    if (!isWellFormedContext(m_context) || !hasOnlyOfferedInitialQuantities(m_context) ||
        m_inputRows.size() != m_context.offeredOptions.size()) {
        return std::nullopt;
    }

    Result result;
    result.costIndex = m_context.costIndex;
    result.objectId = m_context.objectId;
    result.zoneChangeGeneration = m_context.zoneChangeGeneration;

    quint64 total = 0;
    for (const InputRow &row : m_inputRows) {
        quint32 count = 0;
        if (!row.input || !parseAsciiUint32(row.input->text(), count) || count > row.option.availableCount) {
            return std::nullopt;
        }
        if (total > std::numeric_limits<quint64>::max() - count) {
            return std::nullopt;
        }
        total += count;
        if (total > m_context.remainingForThisObject) {
            return std::nullopt;
        }
        if (count > 0) {
            result.selections.append(Selection{row.option.optionId, count});
        }
    }

    std::sort(result.selections.begin(), result.selections.end(), [](const Selection &left, const Selection &right) {
        return left.optionId < right.optionId;
    });
    return result;
}

void RuledCounterQuantityDialog::clearInputRows()
{
    m_inputRows.clear();
    while (QLayoutItem *item = m_rowsLayout->takeAt(0)) {
        delete item->widget();
        delete item;
    }
}

void RuledCounterQuantityDialog::refreshApplyEnabled()
{
    m_applyButton->setEnabled(collectValidResult().has_value());
}

void RuledCounterQuantityDialog::applySelection()
{
    const std::optional<Result> result = collectValidResult();
    if (!result || !isResultCurrent(m_context, m_context, *result)) {
        refreshApplyEnabled();
        return;
    }

    m_acceptedResult = *result;
    QDialog::accept();
}
