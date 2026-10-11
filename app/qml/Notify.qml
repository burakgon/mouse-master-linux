pragma Singleton
import QtQuick

// App wide transient messages, shown by the toast in Main.qml.
QtObject {
    signal show(string text, bool isError)
}
