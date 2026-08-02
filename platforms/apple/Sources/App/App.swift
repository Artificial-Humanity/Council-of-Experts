import SwiftUI
import CouncilOfExpertsKit

@main
struct CouncilOfExpertsApp: App {
    init() {
        // Must run before anything reads a credential: keys written by an earlier build
        // live in UserDefaults, and requests are built straight from the Keychain, so a
        // migration deferred until the user opens Settings would look like lost keys.
        CredentialStore.accounts.forEach(KeychainStore.migrateFromUserDefaults)

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
    @StateObject private var credentials = CredentialStore()
    @AppStorage("maxResponseWords") private var maxResponseWords = 300
    @AppStorage("enableThinkingNotes") private var enableThinkingNotes = false

    var body: some View {
        Form {
            VStack(alignment: .leading, spacing: 14) {
                Text("API Credentials")
                    .font(.headline)
                    .foregroundColor(.secondary)

                Text("Stored in your login Keychain.")
                    .font(.system(size: 9))
                    .foregroundColor(.secondary)

                VStack(alignment: .leading, spacing: 4) {
                    Text("OpenAI API Key:")
                        .font(.caption)
                    SecureField("sk-...", text: $credentials.openAiKey)
                        .textFieldStyle(.roundedBorder)
                }

                VStack(alignment: .leading, spacing: 4) {
                    Text("Anthropic API Key:")
                        .font(.caption)
                    SecureField("sk-ant-...", text: $credentials.anthropicKey)
                        .textFieldStyle(.roundedBorder)
                }

                VStack(alignment: .leading, spacing: 4) {
                    Text("Gemini API Key:")
                        .font(.caption)
                    SecureField("AIzaSy...", text: $credentials.geminiKey)
                        .textFieldStyle(.roundedBorder)
                }

                VStack(alignment: .leading, spacing: 4) {
                    Text("Grok (xAI) API Key:")
                        .font(.caption)
                    SecureField("xai-...", text: $credentials.grokKey)
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

                Divider()

                Text("Thinking Notes")
                    .font(.headline)
                    .foregroundColor(.secondary)

                VStack(alignment: .leading, spacing: 4) {
                    Toggle("Request thinking/reasoning notes", isOn: $enableThinkingNotes)
                        .font(.caption)

                    Text("Shown in each expert's diagnostic pane, not the main chat. Only Anthropic (extended thinking) and Gemini (thought summaries) currently return these; other providers show nothing. Anthropic's extended thinking also uses more output tokens and adds latency.")
                        .font(.system(size: 9))
                        .foregroundColor(.secondary)
                }
            }
            .padding()
        }
        .frame(width: 450, height: 500)
    }
}
