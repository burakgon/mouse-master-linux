import QtQuick
import QtQuick.Layouts

// Shown when the service is not running or no device is connected.
Item {
    id: page
    readonly property bool serviceDown: !Daemon.available
    readonly property bool needsRules: System.deniedDevices > 0 || !System.uinputAccessible

    Timer {
        // Permissions and service state can change outside the app.
        interval: 3000
        running: page.visible
        repeat: true
        onTriggered: System.refresh()
    }

    EmptyState {
        anchors.centerIn: parent
        width: Math.min(parent.width - 64, 560)
        visible: page.serviceDown
        iconName: "power"
        title: "Mouse Master isn't running"
        text: "The background service applies your settings and performs actions. Start it to configure your devices."
        AppButton {
            text: "Start service"
            kind: "primary"
            iconName: "power"
            onClicked: System.startService()
        }
        AppButton {
            visible: System.serviceInstalled && !System.serviceEnabled
            text: "Start at login"
            onClicked: System.setServiceEnabled(true)
        }
    }

    EmptyState {
        anchors.centerIn: parent
        width: Math.min(parent.width - 64, 560)
        visible: !page.serviceDown && page.needsRules
        iconName: "lock"
        title: "Allow access to your devices"
        text: "Mouse Master needs permission to talk to Logitech devices and to send input events. This installs a small udev rule (you will be asked for your password)."
        AppButton {
            text: "Grant access"
            kind: "primary"
            iconName: "check"
            enabled: !System.busy
            onClicked: System.installRules(function (ok) {
                Notify.show(ok ? "Device access granted" : "Could not install the rules", !ok)
                if (ok) System.restartService()
            })
        }
    }

    EmptyState {
        anchors.centerIn: parent
        width: Math.min(parent.width - 64, 560)
        visible: !page.serviceDown && !page.needsRules
        iconName: "mouse"
        title: "Connect a Logitech mouse"
        text: "Turn your mouse on. Pair it over Bluetooth in your system settings, or plug in its Bolt, Unifying or Lightspeed receiver. It will show up here automatically."
        AppButton {
            text: "Bluetooth settings"
            iconName: "bluetooth"
            onClicked: System.openUrl(Daemon.desktop === "kde" ? "systemsettings://kcm_bluetooth" : "bluetooth:")
        }
    }
}
