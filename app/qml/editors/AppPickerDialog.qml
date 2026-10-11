pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as C
import QtQuick.Layouts

// Lets the user pick an installed application.
Modal {
    id: root
    property var recent: []          // recently focused app ids, most recent first
    signal chosen(var app)

    preferredWidth: 520
    subtitle: "Pick an application from the list, or one you used recently."

    property string query: ""
    readonly property var filtered: {
        var all = Apps.all
        if (query === "")
            return all
        var q = query.toLowerCase()
        return all.filter(function (a) {
            return a.name.toLowerCase().indexOf(q) >= 0 || a.id.toLowerCase().indexOf(q) >= 0
        })
    }

    onOpened: {
        search.text = ""
        search.forceActiveFocus()
    }

    InputField {
        id: search
        Layout.fillWidth: true
        iconName: "search"
        placeholderText: "Search applications"
        onTextChanged: root.query = text
    }

    Flow {
        visible: root.recent.length > 0 && root.query === ""
        Layout.fillWidth: true
        spacing: 6
        Repeater {
            model: root.recent
            delegate: Chip {
                required property var modelData
                readonly property var app: Apps.lookup(modelData)
                text: app.name
                imageSource: "image://theme/" + (app.icon || "application-x-executable")
                onClicked: {
                    root.chosen(app.id === modelData ? app : { id: modelData, name: app.name, icon: app.icon, wmClass: "" })
                    root.close()
                }
            }
        }
    }

    ListView {
        id: list
        Layout.fillWidth: true
        Layout.preferredHeight: 360
        clip: true
        model: root.filtered
        spacing: 2
        C.ScrollBar.vertical: C.ScrollBar {}
        delegate: Rectangle {
            id: appRow
            required property var modelData
            width: list.width - 8
            height: 48
            radius: 10
            color: hover.hovered ? Theme.surfaceHover : "transparent"
            HoverHandler { id: hover; cursorShape: Qt.PointingHandCursor }
            TapHandler {
                onTapped: {
                    root.chosen(appRow.modelData)
                    root.close()
                }
            }
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 10
                anchors.rightMargin: 10
                spacing: 12
                AppIcon {
                    iconName: appRow.modelData.icon
                    size: 28
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 0
                    Label {
                        text: appRow.modelData.name
                        Layout.fillWidth: true
                    }
                    Label {
                        text: appRow.modelData.id
                        tertiary: true
                        size: Theme.fontCaption
                        Layout.fillWidth: true
                    }
                }
            }
        }
    }

    footer: [
        Item { Layout.fillWidth: true },
        AppButton {
            text: "Cancel"
            kind: "ghost"
            onClicked: root.close()
        }
    ]
}
