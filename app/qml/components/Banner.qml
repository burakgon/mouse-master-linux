import QtQuick
import QtQuick.Layouts

// A tinted notice above the page: an icon, a short message and optional actions.
Rectangle {
    id: root
    property string iconName: "triangle-alert"
    property color tone: Theme.warning
    property string title
    property string text: ""
    default property alias actions: actionRow.data

    radius: Theme.radius
    color: Qt.rgba(tone.r, tone.g, tone.b, Theme.dark ? 0.10 : 0.08)
    border.width: 1
    border.color: Qt.rgba(tone.r, tone.g, tone.b, Theme.dark ? 0.32 : 0.38)
    implicitHeight: row.implicitHeight + 24

    RowLayout {
        id: row
        anchors.fill: parent
        anchors.leftMargin: 16
        anchors.rightMargin: 12
        anchors.topMargin: 12
        anchors.bottomMargin: 12
        spacing: 12

        Icon {
            name: root.iconName
            color: root.tone
            size: 20
            Layout.alignment: Qt.AlignTop
            Layout.topMargin: 1
        }
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 2
            Label {
                text: root.title
                strong: true
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
            Label {
                visible: root.text !== ""
                text: root.text
                secondary: true
                size: Theme.fontSmall
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
        }
        Row {
            id: actionRow
            spacing: 8
            Layout.alignment: Qt.AlignVCenter
        }
    }
}
