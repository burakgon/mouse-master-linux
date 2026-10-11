import QtQuick
import QtQuick.Layouts

ScrollPage {
    id: page
    property var device
    readonly property var hosts: device ? (device.state.hosts || []) : []
    readonly property int count: device && device.capabilities.hosts ? device.capabilities.hosts : 0

    PageHeader {
        title: "Easy-Switch"
        subtitle: "Your mouse remembers up to " + page.count + " computers. Switch here or with the button under the mouse."
    }

    GridLayout {
        Layout.fillWidth: true
        columns: width > 760 ? 3 : 1
        columnSpacing: 16
        rowSpacing: 16
        Repeater {
            model: page.count
            delegate: Card {
                id: hostCard
                required property int index
                readonly property var host: index < page.hosts.length ? page.hosts[index] : null
                readonly property bool current: page.device.state.current_host === index
                Layout.fillWidth: true
                Layout.preferredHeight: 196
                border.width: current ? 2 : 1
                border.color: current ? Theme.accentAlpha(0.7) : Theme.stroke
                ColumnLayout {
                    anchors.fill: parent
                    spacing: 10
                    RowLayout {
                        Layout.fillWidth: true
                        Rectangle {
                            width: 40
                            height: 40
                            radius: 20
                            color: hostCard.current ? Theme.accent : Theme.surfaceAlt
                            border.width: hostCard.current ? 0 : 1
                            border.color: Theme.strokeStrong
                            Text {
                                anchors.centerIn: parent
                                text: hostCard.index + 1
                                color: hostCard.current ? Theme.accentText : Theme.text
                                font.pixelSize: 18
                                font.weight: Font.Bold
                            }
                        }
                        Item { Layout.fillWidth: true }
                        Rectangle {
                            visible: hostCard.current
                            height: 24
                            width: thisLabel.implicitWidth + 18
                            radius: 12
                            color: Theme.accentAlpha(Theme.dark ? 0.2 : 0.12)
                            Text {
                                id: thisLabel
                                anchors.centerIn: parent
                                text: "This computer"
                                color: Theme.accent
                                font.pixelSize: Theme.fontCaption
                                font.weight: Font.DemiBold
                            }
                        }
                    }
                    Label {
                        Layout.fillWidth: true
                        text: hostCard.host && hostCard.host.paired ? (hostCard.host.name || "Computer " + (hostCard.index + 1)) : "Not paired"
                        size: Theme.fontHeadline
                        strong: true
                        tertiary: !(hostCard.host && hostCard.host.paired)
                    }
                    RowLayout {
                        spacing: 6
                        Icon {
                            name: hostCard.host && hostCard.host.bus === "usb" ? "usb" : hostCard.host && hostCard.host.bus === "receiver" ? "radio" : "bluetooth"
                            size: 14
                            color: Theme.textTertiary
                        }
                        Label {
                            text: !hostCard.host || !hostCard.host.paired ? "Hold the Easy-Switch button to pair"
                                : hostCard.host.bus === "receiver" ? "Receiver" : hostCard.host.bus === "usb" ? "USB" : "Bluetooth"
                            tertiary: true
                            size: Theme.fontCaption
                        }
                    }
                    Item { Layout.fillHeight: true }
                    RowLayout {
                        Layout.fillWidth: true
                        AppButton {
                            visible: !hostCard.current && !!hostCard.host && hostCard.host.paired
                            text: "Switch"
                            iconName: "monitor-smartphone"
                            compact: true
                            onClicked: {
                                confirm.host = hostCard.index
                                confirm.open()
                            }
                        }
                        AppButton {
                            visible: hostCard.current && page.device.capabilities.host_names
                            text: "Rename"
                            iconName: "pencil"
                            kind: "ghost"
                            compact: true
                            onClicked: {
                                rename.text = hostCard.host ? hostCard.host.name : ""
                                rename.open()
                            }
                        }
                    }
                }
            }
        }
    }

    Modal {
        id: confirm
        property int host: 0
        title: "Switch to computer " + (host + 1) + "?"
        subtitle: "Your mouse will disconnect from this computer right away. Press its Easy-Switch button to come back."
        footer: [
            Item { Layout.fillWidth: true },
            AppButton { text: "Cancel"; kind: "ghost"; onClicked: confirm.close() },
            AppButton {
                text: "Switch"
                kind: "primary"
                onClicked: {
                    Daemon.command(page.device.id, { command: "switch-host", host: confirm.host })
                    confirm.close()
                }
            }
        ]
    }

    Modal {
        id: rename
        property alias text: nameField.text
        title: "Rename this computer"
        subtitle: "The name is stored in the mouse and shown on your other computers."
        InputField {
            id: nameField
            Layout.fillWidth: true
            maximumLength: 24
            placeholderText: System.hostName
        }
        footer: [
            AppButton {
                text: "Use “" + System.hostName + "”"
                kind: "ghost"
                onClicked: nameField.text = System.hostName
            },
            Item { Layout.fillWidth: true },
            AppButton { text: "Cancel"; kind: "ghost"; onClicked: rename.close() },
            AppButton {
                text: "Save"
                kind: "primary"
                enabled: nameField.text.trim() !== ""
                onClicked: {
                    Daemon.command(page.device.id, { command: "set-host-name", host: page.device.state.current_host, name: nameField.text.trim() }, function (err) {
                        if (err) Notify.show(err, true)
                    })
                    rename.close()
                }
            }
        ]
    }
}
