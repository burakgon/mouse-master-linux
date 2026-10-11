import QtQuick
import QtQuick.Layouts

// A settings row: icon, title, description and a control on the right.
Item {
    id: root
    property string iconName: ""
    property string title
    property string description: ""
    default property alias control: slot.data
    property bool divider: true

    implicitHeight: Math.max(56, texts.implicitHeight + 24)
    Layout.fillWidth: true

    RowLayout {
        anchors.fill: parent
        spacing: 14
        Rectangle {
            visible: root.iconName !== ""
            Layout.preferredWidth: 34
            Layout.preferredHeight: 34
            radius: 10
            color: Theme.accentAlpha(Theme.dark ? 0.16 : 0.12)
            Icon {
                anchors.centerIn: parent
                name: root.iconName
                color: Theme.accent
                size: 18
            }
        }
        ColumnLayout {
            id: texts
            Layout.fillWidth: true
            spacing: 2
            Label {
                text: root.title
                strong: true
                Layout.fillWidth: true
            }
            Label {
                visible: root.description !== ""
                text: root.description
                secondary: true
                size: Theme.fontSmall
                wrapMode: Text.WordWrap
                elide: Text.ElideNone
                Layout.fillWidth: true
            }
        }
        Item {
            id: slot
            Layout.preferredWidth: childrenRect.width
            Layout.preferredHeight: childrenRect.height
            Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
        }
    }
    Rectangle {
        visible: root.divider
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: 1
        color: Theme.stroke
    }
}
