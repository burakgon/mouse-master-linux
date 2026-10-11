// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once

#include <QJSValue>
#include <functional>
#include <QObject>
#include <QtQml/qqmlregistration.h>

/// Integration with the rest of the system: the user service, device
/// permissions, files and URLs.
class SystemIntegration : public QObject
{
    Q_OBJECT
    QML_NAMED_ELEMENT(System)
    QML_SINGLETON

    Q_PROPERTY(bool serviceInstalled READ serviceInstalled NOTIFY changed)
    Q_PROPERTY(bool serviceEnabled READ serviceEnabled NOTIFY changed)
    Q_PROPERTY(bool rulesInstalled READ rulesInstalled NOTIFY changed)
    Q_PROPERTY(bool uinputAccessible READ uinputAccessible NOTIFY changed)
    Q_PROPERTY(int deniedDevices READ deniedDevices NOTIFY changed)
    Q_PROPERTY(QString hostName READ hostName CONSTANT)
    Q_PROPERTY(QString appVersion READ appVersion CONSTANT)
    Q_PROPERTY(bool busy READ busy NOTIFY busyChanged)

public:
    explicit SystemIntegration(QObject *parent = nullptr);

    bool serviceInstalled() const { return m_serviceInstalled; }
    bool serviceEnabled() const { return m_serviceEnabled; }
    bool rulesInstalled() const { return m_rulesInstalled; }
    bool uinputAccessible() const { return m_uinputAccessible; }
    int deniedDevices() const { return m_deniedDevices; }
    QString hostName() const;
    QString appVersion() const;
    bool busy() const { return m_busy; }

    Q_INVOKABLE void refresh();
    /// Starts the service now (systemd unit, or the binary for development builds).
    Q_INVOKABLE void startService();
    Q_INVOKABLE void restartService();
    /// Enables or disables starting the service at login.
    Q_INVOKABLE void setServiceEnabled(bool enabled);
    /// Installs the udev rules with administrator rights (polkit prompt).
    Q_INVOKABLE void installRules(const QJSValue &callback = QJSValue());
    Q_INVOKABLE void openUrl(const QString &url);
    Q_INVOKABLE void copyToClipboard(const QString &text);
    Q_INVOKABLE QString readTextFile(const QString &url) const;
    Q_INVOKABLE bool writeTextFile(const QString &url, const QString &text) const;
    Q_INVOKABLE QString configPath() const;

Q_SIGNALS:
    void changed();
    void busyChanged();

private:
    void runSystemctl(const QStringList &args, std::function<void(bool)> done = {});
    void setBusy(bool busy);

    bool m_serviceInstalled = false;
    bool m_serviceEnabled = false;
    bool m_rulesInstalled = false;
    bool m_uinputAccessible = false;
    int m_deniedDevices = 0;
    bool m_busy = false;
};
