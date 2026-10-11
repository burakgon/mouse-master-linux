pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as C
import QtQuick.Layouts
import QtCore

C.ApplicationWindow {
    id: window
    width: 1260
    height: 820
    minimumWidth: 940
    minimumHeight: 640
    visible: true
    title: "Mouse Master"
    color: Theme.window

    Settings {
        id: prefs
        category: "ui"
        property alias themeMode: themeBinding.mode
        property alias systemAccent: themeBinding.accent
        property string device: ""
        property string page: "buttons"
        property alias width: window.width
        property alias height: window.height
    }
    QtObject {
        id: themeBinding
        property int mode: 0
        property bool accent: true
        onModeChanged: Theme.mode = mode
        onAccentChanged: Theme.useSystemAccent = accent
    }
    Connections {
        target: Theme
        function onModeChanged() { if (window.devTheme === "") themeBinding.mode = Theme.mode }
        function onUseSystemAccentChanged() { themeBinding.accent = Theme.useSystemAccent }
    }
    Component.onCompleted: {
        Theme.mode = themeBinding.mode
        Theme.useSystemAccent = themeBinding.accent
        currentPage = prefs.page
    }

    // Set from main.cpp for screenshots (MM_CONTROL, MM_THEME): the control to open on
    // the Buttons page and a theme override ("light" or "dark") that is not persisted.
    property string devControl: ""
    property string devTheme: ""
    property string devDevice: ""
    // Keep live input on even when the window is not focused (recordings).
    property bool devWatch: false
    onDevThemeChanged: if (devTheme !== "") Theme.mode = devTheme === "light" ? 1 : 2

    // The list is passed in rather than looked up by id in C++ so that bindings
    // depend on Daemon.devices and follow every device update.
    function findDevice(list, id) {
        for (var i = 0; i < list.length; ++i)
            if (list[i].id === id)
                return list[i]
        return null
    }
    property string currentDevice: {
        var list = Daemon.devices
        if (devDevice !== "" && findDevice(list, devDevice))
            return devDevice
        if (prefs.device !== "" && findDevice(list, prefs.device))
            return prefs.device
        for (var i = 0; i < list.length; ++i)
            if (list[i].online)
                return list[i].id
        return list.length ? list[0].id : ""
    }
    property string currentPage: "buttons"
    readonly property var device: findDevice(Daemon.devices, currentDevice) || ({})
    readonly property bool hasDevice: !!device && device.id !== undefined
    property var recentApps: []

    // Remember recently focused applications for the profile picker.
    Connections {
        target: Daemon
        function onActiveAppChanged() {
            var id = Daemon.activeApp.desktop_file || Daemon.activeApp.app_id
            if (!id || id === "mouse-master" || id === "io.github.burakgon.MouseMaster")
                return
            var list = window.recentApps.filter(function (x) { return x !== id })
            list.unshift(id)
            window.recentApps = list.slice(0, 6)
        }
        function onError(message) { toast.show(message, true) }
    }
    Connections {
        target: Notify
        function onShow(text, isError) { toast.show(text, isError) }
    }

    RowLayout {
        anchors.fill: parent
        spacing: 0

        Sidebar {
            Layout.fillHeight: true
            Layout.preferredWidth: implicitWidth
            currentDevice: window.currentDevice
            device: window.device
            currentPage: window.currentPage
            onSelectDevice: function (id) {
                prefs.device = id
                if (window.currentPage === "settings")
                    window.currentPage = "buttons"
            }
            onSelectPage: function (p) {
                window.currentPage = p
                prefs.page = p
            }
        }
        Rectangle {
            Layout.fillHeight: true
            Layout.preferredWidth: 1
            color: Theme.stroke
        }

        Item {
            id: content
            Layout.fillWidth: true
            Layout.fillHeight: true

            readonly property string effectivePage: {
                if (window.currentPage === "settings")
                    return "settings"
                if (!Daemon.available || !window.hasDevice)
                    return "welcome"
                if (!window.device.online && window.currentPage !== "info")
                    return "offline"
                // Pages that only exist for some devices fall back to Buttons.
                var caps = window.device.capabilities || {}
                if ((window.currentPage === "haptics" && !caps.haptics)
                        || (window.currentPage === "easyswitch" && (caps.hosts || 0) < 2)
                        || (window.currentPage === "scroll" && !caps.dpi && !caps.smartshift && !caps.wheel && !caps.thumbwheel))
                    return "buttons"
                return window.currentPage
            }

            // Problems that affect every page.
            Column {
                id: notices
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 24
                anchors.bottomMargin: 0
                spacing: 8
                readonly property var conflicts: Daemon.available ? Daemon.conflicts : []

                Banner {
                    width: parent.width
                    visible: notices.conflicts.length > 0
                    title: notices.conflicts.join(" and ") + (notices.conflicts.length > 1 ? " are" : " is") + " also running"
                    text: (notices.conflicts.length > 1 ? "They change" : "It changes")
                        + " the same button and wheel settings, so the programs will undo each other's changes. Quit "
                        + (notices.conflicts.length > 1 ? "them" : "it")
                        + " and turn off autostart; Mouse Master applies your settings again as soon as "
                        + (notices.conflicts.length > 1 ? "they are" : "it is") + " gone."
                    AppButton {
                        visible: notices.conflicts.some(function (c) { return c.indexOf("logid") >= 0 })
                        text: "Copy command"
                        iconName: "copy"
                        compact: true
                        onClicked: {
                            System.copyToClipboard("sudo systemctl disable --now logid")
                            Notify.show("Copied “sudo systemctl disable --now logid”", false)
                        }
                    }
                }
                Banner {
                    width: parent.width
                    visible: Daemon.available && !Daemon.inputAvailable
                    title: "Button actions are paused"
                    text: "Mouse Master cannot create its virtual keyboard and mouse because /dev/uinput is not accessible. Device settings still work."
                    AppButton {
                        text: "Grant access"
                        kind: "primary"
                        compact: true
                        enabled: !System.busy
                        onClicked: System.installRules(function (ok) {
                            Notify.show(ok ? "Device access granted" : "Could not install the rules", !ok)
                            if (ok) System.restartService()
                        })
                    }
                }
            }

            Loader {
                id: pageLoader
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                anchors.top: notices.visibleChildren.length > 0 ? notices.bottom : parent.top
                sourceComponent: {
                    switch (content.effectivePage) {
                    case "settings": return settingsPage
                    case "welcome": return welcomePage
                    case "offline": return offlinePage
                    case "scroll": return scrollPage
                    case "easyswitch": return easySwitchPage
                    case "haptics": return hapticsPage
                    case "info": return infoPage
                    default: return buttonsPage
                    }
                }
                onLoaded: {
                    item.opacity = 0
                    fadeIn.restart()
                }
                NumberAnimation {
                    id: fadeIn
                    target: pageLoader.item
                    property: "opacity"
                    from: 0
                    to: 1
                    duration: Theme.anim
                    easing.type: Easing.OutCubic
                }
            }
        }
    }

    Component { id: settingsPage; SettingsPage {} }
    Component { id: welcomePage; WelcomePage {} }
    Component { id: buttonsPage; ButtonsPage { device: window.device; recentApps: window.recentApps; selected: window.devControl; alwaysWatch: window.devWatch } }
    Component { id: scrollPage; PointScrollPage { device: window.device } }
    Component { id: easySwitchPage; EasySwitchPage { device: window.device } }
    Component { id: hapticsPage; HapticsPage { device: window.device } }
    Component { id: infoPage; DeviceInfoPage { device: window.device } }
    Component {
        id: offlinePage
        Item {
            EmptyState {
                anchors.centerIn: parent
                width: Math.min(parent.width - 64, 520)
                iconName: "bluetooth"
                title: window.device ? window.device.name + " is not connected" : ""
                text: "Move or click the mouse to wake it up. Your settings are applied automatically when it reconnects."
                AppButton {
                    text: "Device info"
                    onClicked: window.currentPage = "info"
                }
            }
        }
    }

    Toast { id: toast }
}
