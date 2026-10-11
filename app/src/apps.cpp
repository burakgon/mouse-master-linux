// SPDX-License-Identifier: GPL-3.0-or-later
#include "apps.h"

#include <QDir>
#include <QFile>
#include <QLocale>
#include <QSet>
#include <QStandardPaths>
#include <QTextStream>

#include <algorithm>

namespace {
struct Entry {
    QString id;
    QString name;
    QString icon;
    QString comment;
    QString wmClass;
};

/// Minimal desktop entry parser: the [Desktop Entry] group, localized names.
bool parseDesktopFile(const QString &path, Entry &out)
{
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly | QIODevice::Text)) {
        return false;
    }
    const QString lang = QLocale().name();            // e.g. tr_TR
    const QString shortLang = lang.section('_', 0, 0); // e.g. tr
    QString name, localizedName, comment, type;
    bool inEntry = false, hidden = false;
    QTextStream in(&f);
    while (!in.atEnd()) {
        const QString line = in.readLine().trimmed();
        if (line.startsWith('[')) {
            inEntry = line == QLatin1String("[Desktop Entry]");
            continue;
        }
        if (!inEntry || line.startsWith('#')) {
            continue;
        }
        const int eq = line.indexOf('=');
        if (eq <= 0) {
            continue;
        }
        const QString key = line.left(eq).trimmed();
        const QString value = line.mid(eq + 1).trimmed();
        if (key == QLatin1String("Name")) {
            name = value;
        } else if (key == QStringLiteral("Name[%1]").arg(lang) || (localizedName.isEmpty() && key == QStringLiteral("Name[%1]").arg(shortLang))) {
            localizedName = value;
        } else if (key == QLatin1String("Icon")) {
            out.icon = value;
        } else if (key == QLatin1String("Comment")) {
            comment = value;
        } else if (key == QLatin1String("Type")) {
            type = value;
        } else if (key == QLatin1String("StartupWMClass")) {
            out.wmClass = value;
        } else if ((key == QLatin1String("NoDisplay") || key == QLatin1String("Hidden")) && value == QLatin1String("true")) {
            hidden = true;
        }
    }
    if (hidden || type != QLatin1String("Application") || name.isEmpty()) {
        return false;
    }
    out.name = localizedName.isEmpty() ? name : localizedName;
    out.comment = comment;
    return true;
}
} // namespace

AppDirectory::AppDirectory(QObject *parent)
    : QObject(parent)
{
}

void AppDirectory::scan()
{
    m_scanned = true;
    QVariantList apps;
    QSet<QString> seen;
    const auto dirs = QStandardPaths::standardLocations(QStandardPaths::ApplicationsLocation);
    for (const auto &dir : dirs) {
        QDir d(dir);
        // Desktop file ids use '-' for subdirectories; only top level is common in practice.
        for (const auto &file : d.entryList({QStringLiteral("*.desktop")}, QDir::Files)) {
            QString id = file;
            id.chop(8);
            if (seen.contains(id)) {
                continue; // earlier directories take precedence
            }
            seen.insert(id);
            Entry e;
            e.id = id;
            if (!parseDesktopFile(d.filePath(file), e)) {
                continue;
            }
            apps.append(QVariantMap{
                {QStringLiteral("id"), e.id},
                {QStringLiteral("name"), e.name},
                {QStringLiteral("icon"), e.icon},
                {QStringLiteral("comment"), e.comment},
                {QStringLiteral("wmClass"), e.wmClass},
            });
        }
    }
    std::sort(apps.begin(), apps.end(), [](const QVariant &a, const QVariant &b) {
        return QString::localeAwareCompare(a.toMap().value(QStringLiteral("name")).toString(),
                                           b.toMap().value(QStringLiteral("name")).toString()) < 0;
    });
    m_apps = apps;
}

QVariantList AppDirectory::all()
{
    if (!m_scanned) {
        scan();
    }
    return m_apps;
}

void AppDirectory::reload()
{
    scan();
    Q_EMIT changed();
}

QVariantMap AppDirectory::lookup(const QString &appId)
{
    if (!m_scanned) {
        scan();
    }
    QString needle = appId.trimmed();
    if (needle.endsWith(QLatin1String(".desktop"))) {
        needle.chop(8);
    }
    const QString lower = needle.toLower();
    const QString last = lower.section('.', -1);
    QVariantMap best;
    int bestScore = 0;
    for (const auto &v : std::as_const(m_apps)) {
        const QVariantMap map = v.toMap();
        const QString id = map.value(QStringLiteral("id")).toString().toLower();
        const QString wm = map.value(QStringLiteral("wmClass")).toString().toLower();
        int score = 0;
        if (id == lower) {
            score = 4;
        } else if (!wm.isEmpty() && wm == lower) {
            score = 3;
        } else if (id.section('.', -1) == last || id.section('.', -1) == lower) {
            score = 2;
        } else if (map.value(QStringLiteral("name")).toString().toLower() == lower) {
            score = 1;
        }
        if (score > bestScore) {
            bestScore = score;
            best = map;
        }
    }
    if (bestScore > 0) {
        return best;
    }
    return {{QStringLiteral("id"), appId}, {QStringLiteral("name"), appId}, {QStringLiteral("icon"), QString()}};
}
