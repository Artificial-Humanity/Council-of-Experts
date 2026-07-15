import SwiftUI
import CouncilOfExpertsKit

@main
struct CouncilOfExpertsApp: App {
    init() {
        let verifyText = verifyFfiBridge()
        print("FFI Initialization Check: \(verifyText)")
    }
    
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
        .windowStyle(.hiddenTitleBar)
        
        Settings {
            SettingsView()
        }
    }
}

struct SettingsView: View {
    @AppStorage("openAiKey") private var openAiKey = ""
    @AppStorage("anthropicKey") private var anthropicKey = ""
    @AppStorage("geminiKey") private var geminiKey = ""
    @AppStorage("grokKey") private var grokKey = ""
    @AppStorage("maxResponseWords") private var maxResponseWords = 300

    var body: some View {
        Form {
            VStack(alignment: .leading, spacing: 14) {
                Text("API Credentials")
                    .font(.headline)
                    .foregroundColor(.secondary)

                VStack(alignment: .leading, spacing: 4) {
                    Text("OpenAI API Key:")
                        .font(.caption)
                    SecureField("sk-...", text: $openAiKey)
                        .textFieldStyle(.roundedBorder)
                }

                VStack(alignment: .leading, spacing: 4) {
                    Text("Anthropic API Key:")
                        .font(.caption)
                    SecureField("sk-ant-...", text: $anthropicKey)
                        .textFieldStyle(.roundedBorder)
                }

                VStack(alignment: .leading, spacing: 4) {
                    Text("Gemini API Key:")
                        .font(.caption)
                    SecureField("AIzaSy...", text: $geminiKey)
                        .textFieldStyle(.roundedBorder)
                }

                VStack(alignment: .leading, spacing: 4) {
                    Text("Grok (xAI) API Key:")
                        .font(.caption)
                    SecureField("xai-...", text: $grokKey)
                        .textFieldStyle(.roundedBorder)
                }

                Divider()

                Text("Response Length")
                    .font(.headline)
                    .foregroundColor(.secondary)

                VStack(alignment: .leading, spacing: 4) {
                    Stepper(
                        "Maximum response size: \(maxResponseWords) words",
                        value: $maxResponseWords,
                        in: 100...2000,
                        step: 25
                    )
                    .font(.caption)
                    .onChange(of: maxResponseWords) {
                        if maxResponseWords < 100 { maxResponseWords = 100 }
                    }

                    Text("Applied invisibly as an instruction to every model's prompt, each round, to keep the chat readable.")
                        .font(.system(size: 9))
                        .foregroundColor(.secondary)
                }
            }
            .padding()
        }
        .frame(width: 450, height: 420)
    }
}
