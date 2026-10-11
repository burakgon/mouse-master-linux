import QtQuick
import QtQuick.Controls.Basic as C

// A pill used for profiles, presets and filters.
C.AbstractButton {
    id: root
    property string iconName: ""
    property string imageSource: ""
    property bool selected: false
    property bool closable: false
    signal closeClicked()

    implicitHeight: 32
    implicitWidth: row.implicitWidth + 24
    hoverEnabled: true
    focusPolicy: Qt.StrongFocus

    background: Rectangle {
        radius: height / 2
        color: root.selected ? Theme.accentAlpha(Theme.dark ? 0.22 : 0.14) : root.hovered ? Theme.surfaceHover : Theme.surfaceAlt
        border.width: 1
        border.color: root.selected ? Theme.accentAlpha(0.55) : Theme.stroke
        Behavior on color { ColorAnimation { duration: Theme.animFast } }
    }
    contentItem: Item {
        Row {
            id: row
            anchors.centerIn: parent
            spacing: 6
            Icon {
                visible: root.iconName !== "" && root.imageSource === ""
                name: root.iconName
                size: 15
                color: root.selected ? Theme.accent : Theme.textSecondary
                anchors.verticalCenter: parent.verticalCenter
            }
            Image {
                visible: root.imageSource !== ""
                source: root.imageSource
                width: 18
                height: 18
                sourceSize: Qt.size(36, 36)
                anchors.verticalCenter: parent.verticalCenter
            }
            Text {
                text: root.text
                color: root.selected ? Theme.text : Theme.textSecondary
                font.pixelSize: Theme.fontSmall
                font.weight: root.selected ? Font.DemiBold : Font.Medium
                anchors.verticalCenter: parent.verticalCenter
            }
            Icon {
                visible: root.closable
                name: "x"
                size: 13
                color: closeArea.containsMouse ? Theme.text : Theme.textTertiary
                anchors.verticalCenter: parent.verticalCenter
                MouseArea {
                    id: closeArea
                    anchors.fill: parent
                    anchors.margins: -5
                    hoverEnabled: true
                    onClicked: root.closeClicked()
                }
            }
        }
    }
}
