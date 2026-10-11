// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once

#include <QObject>
#include <QVariantList>
#include <QVariantMap>
#include <QtQml/qqmlregistration.h>

/// Installed applications, read from desktop entries, for per-app profiles.
class AppDirectory : public QObject
{
    Q_OBJECT
    QML_NAMED_ELEMENT(Apps)
    QML_SINGLETON

    Q_PROPERTY(QVariantList all READ all NOTIFY changed)

public:
    explicit AppDirectory(QObject *parent = nullptr);

    QVariantList all();

    /// Looks up an application by desktop file id, Wayland app id or window
    /// class. Returns {id, name, icon} (falls back to the given id).
    Q_INVOKABLE QVariantMap lookup(const QString &appId);
    Q_INVOKABLE void reload();

Q_SIGNALS:
    void changed();

private:
    void scan();

    bool m_scanned = false;
    QVariantList m_apps;
};
