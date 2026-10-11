pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as C
import QtQuick.Layouts
import "Layouts.js" as Layouts

Rectangle {
    id: root
    property string currentDevice: ""
    property string currentPage: "buttons"
    signal selectDevice(string id)
    signal selectPage(string page)

    property var device: ({})
    readonly property var caps: device && device.capabilities ? device.capabilities : ({})

    color: Theme.sidebar
    implicitWidth: 252

    component NavItem: Rectangle {
        id: nav
        property string page
        property string iconName
        property string label
        readonly property bool current: root.currentPage === page
        Layout.fillWidth: true
        height: 40
        radius: 10
        color: current ? Theme.accentAlpha(Theme.dark ? 0.18 : 0.12) : navHover.hovered ? Theme.surfaceHover : "transparent"
        Behavior on color { ColorAnimation { duration: Theme.animFast } }
        HoverHandler { id: navHover; cursorShape: Qt.PointingHandCursor }
        TapHandler { onTapped: root.selectPage(nav.page) }
        Rectangle {
            visible: nav.current
            x: 0
            anchors.verticalCenter: parent.verticalCenter
            width: 3
            height: 18
            radius: 2
            color: Theme.accent
        }
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 14
            anchors.rightMargin: 10
            spacing: 12
            Icon {
                name: nav.iconName
                size: 18
                color: nav.current ? Theme.accent : Theme.textSecondary
            }
            Label {
                text: nav.label
                Layout.fillWidth: true
                strong: nav.current
                color: nav.current ? Theme.text : Theme.textSecondary
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 14
        anchors.topMargin: 18
        spacing: 6

        // Brand
        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 6
            Layout.bottomMargin: 14
            spacing: 10
            Image {
                source: "qrc:/icons/app/io.github.burakgon.MouseMaster.svg"
                sourceSize: Qt.size(64, 64)
                Layout.preferredWidth: 30
                Layout.preferredHeight: 30
            }
            Label {
                text: "Mouse Master"
                size: Theme.fontHeadline + 1
                strong: true
                Layout.fillWidth: true
            }
        }

        Label {
            visible: Daemon.devices.length > 0
            text: "DEVICES"
            tertiary: true
            size: Theme.fontCaption - 1
            font.weight: Font.DemiBold
            font.letterSpacing: 0.8
            Layout.leftMargin: 8
        }

        Repeater {
            model: Daemon.devices
            delegate: Rectangle {
                id: card
                required property var modelData
                readonly property bool current: modelData.id === root.currentDevice
                Layout.fillWidth: true
                height: 64
                radius: 14
                color: current ? Theme.surface : cardHover.hovered ? Theme.surfaceHover : "transparent"
                border.width: current ? 1 : 0
                border.color: Theme.stroke
                Behavior on color { ColorAnimation { duration: Theme.animFast } }
                HoverHandler { id: cardHover; cursorShape: Qt.PointingHandCursor }
                TapHandler { onTapped: root.selectDevice(card.modelData.id) }
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 10
                    anchors.rightMargin: 12
                    spacing: 10
                    Rectangle {
                        Layout.preferredWidth: 40
                        Layout.preferredHeight: 40
                        radius: 12
                        color: card.current ? Theme.accentAlpha(Theme.dark ? 0.2 : 0.14) : Theme.surfaceAlt
                        // A miniature of the device artwork.
                        Image {
                            anchors.centerIn: parent
                            visible: card.modelData.kind !== "keyboard"
                            source: Layouts.forDevice(card.modelData).art
                            height: 34
                            width: height * Layouts.forDevice(card.modelData).width / Layouts.forDevice(card.modelData).height
                            sourceSize: Qt.size(width * 3, height * 3)
                            smooth: true
                            mipmap: true
                            opacity: card.modelData.online ? 1 : 0.5
                        }
                        Icon {
                            anchors.centerIn: parent
                            visible: card.modelData.kind === "keyboard"
                            name: "keyboard"
                            size: 20
                            color: card.current ? Theme.accent : Theme.textSecondary
                        }
                        Rectangle {
                            anchors.right: parent.right
                            anchors.bottom: parent.bottom
                            anchors.margins: -2
                            width: 12
                            height: 12
                            radius: 6
                            color: card.modelData.online ? Theme.success : Theme.textTertiary
                            border.width: 2
                            border.color: Theme.sidebar
                        }
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 3
                        Label {
                            text: card.modelData.name
                            strong: true
                            Layout.fillWidth: true
                        }
                        RowLayout {
                            spacing: 8
                            BatteryIndicator {
                                visible: card.modelData.online && !!card.modelData.battery
                                battery: card.modelData.battery
                            }
                            Label {
                                visible: !card.modelData.online
                                text: "Disconnected"
                                tertiary: true
                                size: Theme.fontCaption
                            }
                        }
                    }
                }
            }
        }

        Item { Layout.preferredHeight: 10; visible: !!root.device && root.device.id !== undefined }

        ColumnLayout {
            visible: !!root.device && !!root.device.id
            Layout.fillWidth: true
            spacing: 2
            NavItem { page: "buttons"; iconName: "mouse-pointer-click"; label: "Buttons" }
            NavItem { page: "scroll"; iconName: "gauge"; label: "Point & scroll"; visible: !!root.caps.dpi || !!root.caps.smartshift || !!root.caps.wheel || !!root.caps.thumbwheel }
            NavItem { page: "easyswitch"; iconName: "monitor-smartphone"; label: "Easy-Switch"; visible: (root.caps.hosts || 0) > 1 }
            NavItem { page: "haptics"; iconName: "vibrate"; label: "Haptic feedback"; visible: !!root.caps.haptics }
            NavItem { page: "info"; iconName: "info"; label: "Device info" }
        }

        Item { Layout.fillHeight: true }

        NavItem { page: "settings"; iconName: "settings"; label: "Settings" }
    }
}
