// SPDX-License-Identifier: GPL-3.0-or-later
#include "daemonclient.h"

#include <QDBusConnection>
#include <QDBusConnectionInterface>
#include <QDBusMessage>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QDBusVariant>
#include <QJSEngine>
#include <QJsonDocument>
#include <QQmlEngine>

namespace {
const QString Service = QStringLiteral("io.github.burakgon.MouseMaster.Daemon");
const QString Path = QStringLiteral("/io/github/burakgon/MouseMaster/Daemon");
const QString Interface = QStringLiteral("io.github.burakgon.MouseMaster.Daemon1");

QVariant parseJson(const QString &text)
{
    QJsonParseError err;
    const auto doc = QJsonDocument::fromJson(text.toUtf8(), &err);
    if (err.error != QJsonParseError::NoError) {
        return {};
    }
    return doc.toVariant();
}

QString toJson(const QVariant &value)
{
    return QString::fromUtf8(QJsonDocument::fromVariant(value).toJson(QJsonDocument::Compact));
}

QDBusMessage call(const QString &method, const QVariantList &args = {})
{
    auto msg = QDBusMessage::createMethodCall(Service, Path, Interface, method);
    msg.setArguments(args);
    return msg;
}
} // namespace

DaemonClient::DaemonClient(QObject *parent)
    : QObject(parent)
{
    auto bus = QDBusConnection::sessionBus();
    m_watcher = new QDBusServiceWatcher(Service, bus, QDBusServiceWatcher::WatchForOwnerChange, this);
    connect(m_watcher, &QDBusServiceWatcher::serviceRegistered, this, [this] { reload(); });
    connect(m_watcher, &QDBusServiceWatcher::serviceUnregistered, this, [this] { setAvailable(false); });

    bus.connect(Service, Path, Interface, QStringLiteral("DevicesChanged"), this, SLOT(onDevicesChanged()));
    bus.connect(Service, Path, Interface, QStringLiteral("DeviceChanged"), this, SLOT(onDeviceChanged(QString, QString)));
    bus.connect(Service, Path, Interface, QStringLiteral("ButtonEvent"), this, SLOT(onButtonEvent(QString, QString, bool)));
    bus.connect(Service, Path, Interface, QStringLiteral("GestureEvent"), this, SLOT(onGestureEvent(QString, QString, QString)));
    bus.connect(Service, Path, Interface, QStringLiteral("MotionEvent"), this, SLOT(onMotionEvent(QString, int, int)));
    bus.connect(Service, Path, Interface, QStringLiteral("FocusChanged"), this, SLOT(onFocusChanged(QString)));

    reload();
}

void DaemonClient::reload()
{
    auto bus = QDBusConnection::sessionBus();
    const bool registered = bus.interface() && bus.interface()->isServiceRegistered(Service).value();
    if (!registered) {
        setAvailable(false);
        if (!m_loaded) {
            m_loaded = true;
            Q_EMIT loadedChanged();
        }
        return;
    }
    fetchProperties();
    fetchDevices();
    fetchCatalog();
    fetchGeneral();
}

void DaemonClient::setAvailable(bool available)
{
    if (m_available == available) {
        return;
    }
    m_available = available;
    if (!available) {
        // Keep the last known devices but show them offline.
        for (auto &d : m_devices) {
            auto map = d.toMap();
            map[QStringLiteral("online")] = false;
            d = map;
        }
        Q_EMIT devicesChanged();
    }
    Q_EMIT availableChanged();
}

void DaemonClient::callJson(const QString &method, const QVariantList &args, std::function<void(const QVariant &)> done)
{
    auto pending = QDBusConnection::sessionBus().asyncCall(call(method, args));
    auto watcher = new QDBusPendingCallWatcher(pending, this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [done = std::move(done)](QDBusPendingCallWatcher *w) {
        QDBusPendingReply<QString> reply = *w;
        w->deleteLater();
        if (reply.isError()) {
            return;
        }
        done(parseJson(reply.value()));
    });
}

void DaemonClient::fetchProperties()
{
    auto msg = QDBusMessage::createMethodCall(Service, Path, QStringLiteral("org.freedesktop.DBus.Properties"), QStringLiteral("GetAll"));
    msg << Interface;
    auto watcher = new QDBusPendingCallWatcher(QDBusConnection::sessionBus().asyncCall(msg), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *w) {
        QDBusPendingReply<QVariantMap> reply = *w;
        w->deleteLater();
        if (reply.isError()) {
            setAvailable(false);
            return;
        }
        const auto props = reply.value();
        m_version = props.value(QStringLiteral("Version")).toString();
        m_desktop = props.value(QStringLiteral("Desktop")).toString();
        m_inputAvailable = props.value(QStringLiteral("InputAvailable"), true).toBool();
        m_activeApp = parseJson(props.value(QStringLiteral("ActiveApp")).toString()).toMap();
        Q_EMIT activeAppChanged();
        m_available = false; // force the notification below
        setAvailable(true);
    });
}

void DaemonClient::fetchDevices()
{
    callJson(QStringLiteral("ListDevices"), {}, [this](const QVariant &v) {
        m_devices = v.toList();
        Q_EMIT devicesChanged();
        if (!m_loaded) {
            m_loaded = true;
            Q_EMIT loadedChanged();
        }
    });
}

void DaemonClient::fetchCatalog()
{
    callJson(QStringLiteral("GetCatalog"), {}, [this](const QVariant &v) {
        m_catalog = v.toMap();
        Q_EMIT catalogChanged();
    });
}

void DaemonClient::fetchGeneral()
{
    callJson(QStringLiteral("GetGeneral"), {}, [this](const QVariant &v) {
        m_general = v.toMap();
        Q_EMIT generalChanged();
    });
}

QVariantMap DaemonClient::device(const QString &id) const
{
    for (const auto &d : m_devices) {
        const auto map = d.toMap();
        if (map.value(QStringLiteral("id")).toString() == id) {
            return map;
        }
    }
    return {};
}

void DaemonClient::invokeCallback(QJSValue callback, const QString &error, const QVariant &result)
{
    if (!callback.isCallable()) {
        if (!error.isEmpty()) {
            Q_EMIT this->error(error);
        }
        return;
    }
    auto engine = qjsEngine(this);
    const QJSValue err = error.isEmpty() ? QJSValue(QJSValue::NullValue) : QJSValue(error);
    const QJSValue res = engine ? engine->toScriptValue(result) : QJSValue();
    callback.call({err, res});
}

void DaemonClient::command(const QString &id, const QVariantMap &command, const QJSValue &callback)
{
    auto pending = QDBusConnection::sessionBus().asyncCall(call(QStringLiteral("Command"), {id, toJson(command)}));
    auto watcher = new QDBusPendingCallWatcher(pending, this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this, callback](QDBusPendingCallWatcher *w) {
        QDBusPendingReply<QString> reply = *w;
        w->deleteLater();
        if (reply.isError()) {
            invokeCallback(callback, reply.error().message(), {});
        } else {
            invokeCallback(callback, {}, parseJson(reply.value()));
        }
    });
}

void DaemonClient::setGeneral(const QVariantMap &patch)
{
    // Optimistic update so bound controls do not flicker.
    for (auto it = patch.cbegin(); it != patch.cend(); ++it) {
        m_general[it.key()] = it.value();
    }
    Q_EMIT generalChanged();
    auto pending = QDBusConnection::sessionBus().asyncCall(call(QStringLiteral("SetGeneral"), {toJson(patch)}));
    auto watcher = new QDBusPendingCallWatcher(pending, this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *w) {
        QDBusPendingReply<> reply = *w;
        w->deleteLater();
        if (reply.isError()) {
            Q_EMIT error(reply.error().message());
        }
        fetchGeneral();
    });
}

void DaemonClient::runAction(const QVariantMap &action)
{
    QDBusConnection::sessionBus().asyncCall(call(QStringLiteral("RunAction"), {toJson(action)}));
}

void DaemonClient::setMonitoring(bool enabled)
{
    QDBusConnection::sessionBus().asyncCall(call(QStringLiteral("SetMonitoring"), {enabled}));
}

void DaemonClient::forgetDevice(const QString &id)
{
    auto pending = QDBusConnection::sessionBus().asyncCall(call(QStringLiteral("ForgetDevice"), {id}));
    auto watcher = new QDBusPendingCallWatcher(pending, this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *w) {
        QDBusPendingReply<> reply = *w;
        w->deleteLater();
        if (reply.isError()) {
            Q_EMIT error(reply.error().message());
        }
        fetchDevices();
    });
}

QString DaemonClient::exportConfig() const
{
    const auto reply = QDBusConnection::sessionBus().call(call(QStringLiteral("GetConfig")), QDBus::Block, 3000);
    if (reply.type() != QDBusMessage::ReplyMessage || reply.arguments().isEmpty()) {
        return {};
    }
    // Pretty print for humans.
    const auto doc = QJsonDocument::fromJson(reply.arguments().constFirst().toString().toUtf8());
    return QString::fromUtf8(doc.toJson(QJsonDocument::Indented));
}

void DaemonClient::importConfig(const QString &text, const QJSValue &callback)
{
    auto pending = QDBusConnection::sessionBus().asyncCall(call(QStringLiteral("SetConfig"), {text}));
    auto watcher = new QDBusPendingCallWatcher(pending, this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this, callback](QDBusPendingCallWatcher *w) {
        QDBusPendingReply<> reply = *w;
        w->deleteLater();
        invokeCallback(callback, reply.isError() ? reply.error().message() : QString(), {});
        fetchGeneral();
    });
}

void DaemonClient::onDevicesChanged()
{
    fetchDevices();
}

void DaemonClient::onDeviceChanged(const QString &id, const QString &snapshot)
{
    const auto map = parseJson(snapshot).toMap();
    if (map.isEmpty()) {
        return;
    }
    bool found = false;
    for (auto &d : m_devices) {
        if (d.toMap().value(QStringLiteral("id")).toString() == id) {
            d = map;
            found = true;
            break;
        }
    }
    if (!found) {
        m_devices.append(map);
    }
    Q_EMIT devicesChanged();
    Q_EMIT deviceUpdated(id);
}

void DaemonClient::onButtonEvent(const QString &id, const QString &control, bool pressed)
{
    Q_EMIT buttonEvent(id, control, pressed);
}

void DaemonClient::onGestureEvent(const QString &id, const QString &control, const QString &gesture)
{
    Q_EMIT gestureEvent(id, control, gesture);
}

void DaemonClient::onMotionEvent(const QString &id, int dx, int dy)
{
    Q_EMIT motionEvent(id, dx, dy);
}

void DaemonClient::onFocusChanged(const QString &app)
{
    m_activeApp = parseJson(app).toMap();
    Q_EMIT activeAppChanged();
}
