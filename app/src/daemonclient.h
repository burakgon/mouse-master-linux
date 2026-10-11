// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once

#include <QDBusServiceWatcher>
#include <functional>
#include <QJSValue>
#include <QObject>
#include <QVariantList>
#include <QVariantMap>
#include <QtQml/qqmlregistration.h>

/// Talks to mouse-masterd over the session bus and mirrors its state for QML.
class DaemonClient : public QObject
{
    Q_OBJECT
    QML_NAMED_ELEMENT(Daemon)
    QML_SINGLETON

    Q_PROPERTY(bool available READ available NOTIFY availableChanged)
    Q_PROPERTY(bool loaded READ loaded NOTIFY loadedChanged)
    Q_PROPERTY(QVariantList devices READ devices NOTIFY devicesChanged)
    Q_PROPERTY(QVariantMap catalog READ catalog NOTIFY catalogChanged)
    Q_PROPERTY(QVariantMap general READ general NOTIFY generalChanged)
    Q_PROPERTY(QVariantMap activeApp READ activeApp NOTIFY activeAppChanged)
    Q_PROPERTY(QString desktop READ desktop NOTIFY availableChanged)
    Q_PROPERTY(QString version READ version NOTIFY availableChanged)
    Q_PROPERTY(bool inputAvailable READ inputAvailable NOTIFY availableChanged)

public:
    explicit DaemonClient(QObject *parent = nullptr);

    bool available() const { return m_available; }
    bool loaded() const { return m_loaded; }
    QVariantList devices() const { return m_devices; }
    QVariantMap catalog() const { return m_catalog; }
    QVariantMap general() const { return m_general; }
    QVariantMap activeApp() const { return m_activeApp; }
    QString desktop() const { return m_desktop; }
    QString version() const { return m_version; }
    bool inputAvailable() const { return m_inputAvailable; }

    /// Snapshot of one device (empty map if unknown).
    Q_INVOKABLE QVariantMap device(const QString &id) const;
    /// Runs a device command; `callback(error, result)` is called when done.
    Q_INVOKABLE void command(const QString &id, const QVariantMap &command, const QJSValue &callback = QJSValue());
    Q_INVOKABLE void setGeneral(const QVariantMap &patch);
    Q_INVOKABLE void runAction(const QVariantMap &action);
    Q_INVOKABLE void setMonitoring(bool enabled);
    Q_INVOKABLE void forgetDevice(const QString &id);
    Q_INVOKABLE QString exportConfig() const;
    Q_INVOKABLE void importConfig(const QString &text, const QJSValue &callback = QJSValue());
    Q_INVOKABLE void reload();

Q_SIGNALS:
    void availableChanged();
    void loadedChanged();
    void devicesChanged();
    void catalogChanged();
    void generalChanged();
    void activeAppChanged();
    void deviceUpdated(const QString &id);
    void buttonEvent(const QString &id, const QString &control, bool pressed);
    void gestureEvent(const QString &id, const QString &control, const QString &gesture);
    void motionEvent(const QString &id, int dx, int dy);
    /// A command failed; `message` is suitable for showing to the user.
    void error(const QString &message);

private Q_SLOTS:
    void onDevicesChanged();
    void onDeviceChanged(const QString &id, const QString &snapshot);
    void onButtonEvent(const QString &id, const QString &control, bool pressed);
    void onGestureEvent(const QString &id, const QString &control, const QString &gesture);
    void onMotionEvent(const QString &id, int dx, int dy);
    void onFocusChanged(const QString &app);

private:
    void setAvailable(bool available);
    void fetchDevices();
    void fetchCatalog();
    void fetchGeneral();
    void fetchProperties();
    void callJson(const QString &method, const QVariantList &args, std::function<void(const QVariant &)> done);
    void invokeCallback(QJSValue callback, const QString &error, const QVariant &result);

    QDBusServiceWatcher *m_watcher = nullptr;
    bool m_available = false;
    bool m_loaded = false;
    QVariantList m_devices;
    QVariantMap m_catalog;
    QVariantMap m_general;
    QVariantMap m_activeApp;
    QString m_desktop;
    QString m_version;
    bool m_inputAvailable = true;
};
