import SwiftUI
import PanelOfExpertsKit

@main
struct PanelOfExpertsApp: App {
    init() {
        let verifyText = verifyFfiBridge()
        print("FFI Initialization Check: \(verifyText)")
    }
    
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
        .windowStyle(.hiddenTitleBar)
    }
}
