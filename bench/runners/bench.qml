import QtQuick
import QtQuick.Window
import QtWebEngine

Window {
    width: 1280; height: 900; visible: true
    WebEngineView {
        anchors.fill: parent
        url: Qt.application.arguments[Qt.application.arguments.length - 1].startsWith("http")
             ? Qt.application.arguments[Qt.application.arguments.length - 1]
             : "about:blank"
        settings.playbackRequiresUserGesture: false
    }
}
