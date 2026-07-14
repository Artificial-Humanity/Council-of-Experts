import Foundation
import SwiftUI
import CouncilOfExpertsKit

struct ExpertState {
    var id: String
    var name: String
    var status: String // "idle", "drafting", "completed", "error"
    var response: String
    var critiqueStatus: String // "idle", "drafting", "completed", "error"
    var critiqueResponse: String
    var error: String?
}

struct ExpertConfigInput {
    var name: String
    var providerType: String // "Mock Sandbox", "Anthropic Claude", "OpenAI GPT", "Google Gemini", "Local Ollama/LM Studio"
    var modelName: String
    var baseUrl: String
    var systemPrompt: String
}

struct CodableMessage: Codable, Identifiable {
    var id: String
    var role: String // "user", "assistant"
    var content: String
    var timestamp: UInt64
}

class CouncilViewModel: ObservableObject, FfiCouncilCallback {
    @Published var expertStates: [String: ExpertState] = [:]
    @Published var chairmanText: String = ""
    @Published var chairmanStatus: String = "idle" // "idle", "synthesis", "completed", "error"
    @Published var chairmanError: String?
    @Published var isExecuting: Bool = false
    @Published var prompt: String = ""
    
    // Conversation history log
    @Published var messages: [CodableMessage] = []
    
    // Config values
    @Published var enableCritique: Bool = true
    
    // Workspace Directory Integration (Milestone 7)
    @Published var workspacePath: String = ""
    @Published var scannedFiles: [URL] = []
    @Published var selectedFilePaths: Set<String> = []
    
    // Dynamic expert configuration settings (Limit of 8 active experts)
    @Published var activeExpertCount: Int = 2 {
        didSet {
            UserDefaults.standard.set(activeExpertCount, forKey: "activeExpertCount")
        }
    }
    
    @Published var expertsConfig: [ExpertConfigInput] = [
        ExpertConfigInput(
            name: "Claudia",
            providerType: "Mock Sandbox",
            modelName: "mock-expert-1",
            baseUrl: "http://localhost:11434/v1",
            systemPrompt: "You are Claudia, a software developer focusing on type-safety, clean compilation boundaries, and architecture."
        ),
        ExpertConfigInput(
            name: "Oliver",
            providerType: "Mock Sandbox",
            modelName: "mock-expert-2",
            baseUrl: "http://localhost:11434/v1",
            systemPrompt: "You are Oliver, a systems programmer focusing on extreme optimizations, memory management, and performance in low-level Rust/C++."
        ),
        ExpertConfigInput(
            name: "Sarah",
            providerType: "Mock Sandbox",
            modelName: "mock-expert-3",
            baseUrl: "http://localhost:11434/v1",
            systemPrompt: "You are Sarah, a product manager & UX designer focusing on user-centered design, workflows, and accessibility."
        ),
        ExpertConfigInput(
            name: "David",
            providerType: "Mock Sandbox",
            modelName: "mock-expert-4",
            baseUrl: "http://localhost:11434/v1",
            systemPrompt: "You are David, a security auditor focusing on vulnerability analysis, threat modeling, and cryptographic best practices."
        ),
        ExpertConfigInput(
            name: "Elena",
            providerType: "Mock Sandbox",
            modelName: "mock-expert-5",
            baseUrl: "http://localhost:11434/v1",
            systemPrompt: "You are Elena, a DevOps specialist focusing on build pipelines, deployment configurations, and containerization."
        ),
        ExpertConfigInput(
            name: "Marcus",
            providerType: "Mock Sandbox",
            modelName: "mock-expert-6",
            baseUrl: "http://localhost:11434/v1",
            systemPrompt: "You are Marcus, a database engineer focusing on schema design, query optimization, and transaction safety."
        ),
        ExpertConfigInput(
            name: "Chloe",
            providerType: "Mock Sandbox",
            modelName: "mock-expert-7",
            baseUrl: "http://localhost:11434/v1",
            systemPrompt: "You are Chloe, a technical writer focusing on clean documentation, API usability, and code readability."
        ),
        ExpertConfigInput(
            name: "Yuki",
            providerType: "Mock Sandbox",
            modelName: "mock-expert-8",
            baseUrl: "http://localhost:11434/v1",
            systemPrompt: "You are Yuki, a quality assurance engineer focusing on edge cases, unit test coverage, and regression prevention."
        )
    ]
    
    @Published var chairmanConfig = ExpertConfigInput(
        name: "Gaston (Chairman)",
        providerType: "Mock Sandbox",
        modelName: "mock-chairman",
        baseUrl: "http://localhost:11434/v1",
        systemPrompt: "You are Gaston, the Chairman of the Council. Review the expert proposals and critiques, then synthesize them into a clean, complete response."
    )
    
    init() {
        loadSession()
        
        // Load active expert count
        if let savedCount = UserDefaults.standard.object(forKey: "activeExpertCount") as? Int {
            self.activeExpertCount = savedCount
        }
        
        // Load workspace path
        if let savedPath = UserDefaults.standard.string(forKey: "workspacePath") {
            self.workspacePath = savedPath
            refreshFiles()
        }
    }
    
    // ── Session Storage Helpers ──
    private var sessionURL: URL {
        let paths = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)
        let supportDir = paths[0].appendingPathComponent("technology.mcfarlin.council-of-experts", isDirectory: true)
        try? FileManager.default.createDirectory(at: supportDir, withIntermediateDirectories: true, attributes: nil)
        return supportDir.appendingPathComponent("active_session.json")
    }
    
    func saveSession() {
        do {
            let data = try JSONEncoder().encode(messages)
            try data.write(to: sessionURL)
        } catch {
            print("Failed to save session: \(error)")
        }
    }
    
    func loadSession() {
        do {
            let data = try Data(contentsOf: sessionURL)
            let loaded = try JSONDecoder().decode([CodableMessage].self, from: data)
            self.messages = loaded
        } catch {
            print("No active session found or failed to load: \(error)")
        }
    }
    
    func clearHistory() {
        messages.removeAll()
        try? FileManager.default.removeItem(at: sessionURL)
        chairmanText = ""
        chairmanStatus = "idle"
        chairmanError = nil
    }
    
    // ── Workspace Directory (Milestone 7) ──
    func selectDirectory() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false
        panel.title = "Select Workspace Directory"
        
        if panel.runModal() == .OK {
            if let url = panel.url {
                DispatchQueue.main.async {
                    self.workspacePath = url.path
                    UserDefaults.standard.set(url.path, forKey: "workspacePath")
                    self.selectedFilePaths.removeAll()
                    self.refreshFiles()
                }
            }
        }
    }
    
    func refreshFiles() {
        guard !workspacePath.isEmpty else {
            self.scannedFiles = []
            return
        }
        
        let path = workspacePath
        DispatchQueue.global(qos: .userInitiated).async {
            let fileManager = FileManager.default
            let url = URL(fileURLWithPath: path)
            guard let enumerator = fileManager.enumerator(at: url, includingPropertiesForKeys: [.isRegularFileKey], options: [.skipsHiddenFiles, .skipsPackageDescendants]) else {
                DispatchQueue.main.async {
                    self.scannedFiles = []
                }
                return
            }
            
            var files: [URL] = []
            for case let fileURL as URL in enumerator {
                do {
                    let resourceValues = try fileURL.resourceValues(forKeys: [.isRegularFileKey])
                    if resourceValues.isRegularFile ?? false {
                        let ext = fileURL.pathExtension.lowercased()
                        let binaryExtensions = ["png", "jpg", "jpeg", "gif", "pdf", "zip", "tar", "gz", "dylib", "a", "so", "exe", "app", "framework", "xcframework", "o", "d", "swiftmodule", "swiftdoc"]
                        if !binaryExtensions.contains(ext) {
                            files.append(fileURL)
                        }
                    }
                } catch {
                    print(error)
                }
            }
            let sorted = files.sorted(by: { $0.path < $1.path })
            DispatchQueue.main.async {
                self.scannedFiles = sorted
            }
        }
    }
    
    func toggleFileSelection(path: String) {
        if selectedFilePaths.contains(path) {
            selectedFilePaths.remove(path)
        } else {
            selectedFilePaths.insert(path)
        }
    }
    
    // ── Drafting Phase Callbacks ──
    func onExpertStarted(expertId: String) {
        DispatchQueue.main.async {
            if var state = self.expertStates[expertId] {
                state.status = "drafting"
                state.response = ""
                state.error = nil
                self.expertStates[expertId] = state
            }
        }
    }
    
    func onExpertChunk(expertId: String, chunk: String) {
        DispatchQueue.main.async {
            if var state = self.expertStates[expertId] {
                state.response += chunk
                self.expertStates[expertId] = state
            }
        }
    }
    
    func onExpertCompleted(expertId: String, fullResponse: String) {
        DispatchQueue.main.async {
            if var state = self.expertStates[expertId] {
                state.status = "completed"
                state.response = fullResponse
                self.expertStates[expertId] = state
            }
        }
    }
    
    func onExpertError(expertId: String, error: String) {
        DispatchQueue.main.async {
            if var state = self.expertStates[expertId] {
                state.status = "error"
                state.error = error
                self.expertStates[expertId] = state
            }
        }
    }
    
    // ── Critique Phase Callbacks ──
    func onExpertCritiqueStarted(expertId: String) {
        DispatchQueue.main.async {
            if var state = self.expertStates[expertId] {
                state.critiqueStatus = "drafting"
                state.critiqueResponse = ""
                state.error = nil
                self.expertStates[expertId] = state
            }
        }
    }
    
    func onExpertCritiqueChunk(expertId: String, chunk: String) {
        DispatchQueue.main.async {
            if var state = self.expertStates[expertId] {
                state.critiqueResponse += chunk
                self.expertStates[expertId] = state
            }
        }
    }
    
    func onExpertCritiqueCompleted(expertId: String, fullCritique: String) {
        DispatchQueue.main.async {
            if var state = self.expertStates[expertId] {
                state.critiqueStatus = "completed"
                state.critiqueResponse = fullCritique
                self.expertStates[expertId] = state
            }
        }
    }
    
    func onExpertCritiqueError(expertId: String, error: String) {
        DispatchQueue.main.async {
            if var state = self.expertStates[expertId] {
                state.critiqueStatus = "error"
                state.error = error
                self.expertStates[expertId] = state
            }
        }
    }
    
    // ── Chairman Synthesis Callbacks ──
    func onChairmanStarted() {
        DispatchQueue.main.async {
            self.chairmanStatus = "synthesis"
            self.chairmanText = ""
            self.chairmanError = nil
        }
    }
    
    func onChairmanChunk(chunk: String) {
        DispatchQueue.main.async {
            self.chairmanText += chunk
        }
    }
    
    func onChairmanCompleted(fullResponse: String) {
        DispatchQueue.main.async {
            self.chairmanStatus = "completed"
            self.chairmanText = fullResponse
            
            // Append Gaston's finalized synthesized answer to chat log
            let assistantMsg = CodableMessage(
                id: UUID().uuidString,
                role: "assistant",
                content: fullResponse,
                timestamp: UInt64(Date().timeIntervalSince1970)
            )
            self.messages.append(assistantMsg)
            self.saveSession()
        }
    }
    
    func onChairmanError(error: String) {
        DispatchQueue.main.async {
            self.chairmanStatus = "error"
            self.chairmanError = error
        }
    }
    
    private func buildFfiExpert(id: String, defaultName: String, input: ExpertConfigInput) -> FfiExpert {
        let ffiType: FfiProviderType
        let modelName: String
        let apiKey: String?
        let baseUrl: String?
        
        switch input.providerType {
        case "Mock Sandbox":
            ffiType = .mock
            modelName = input.modelName.isEmpty ? "mock-model" : input.modelName
            apiKey = nil
            baseUrl = nil
        case "Anthropic Claude":
            ffiType = .anthropic
            modelName = input.modelName.isEmpty ? "claude-3-5-sonnet-latest" : input.modelName
            let key = UserDefaults.standard.string(forKey: "anthropicKey") ?? ""
            apiKey = key.isEmpty ? nil : key
            baseUrl = nil
        case "OpenAI GPT":
            ffiType = .openAi
            modelName = input.modelName.isEmpty ? "gpt-4o" : input.modelName
            let key = UserDefaults.standard.string(forKey: "openAiKey") ?? ""
            apiKey = key.isEmpty ? nil : key
            baseUrl = nil
        case "Google Gemini":
            ffiType = .gemini
            modelName = input.modelName.isEmpty ? "gemini-1.5-pro" : input.modelName
            let key = UserDefaults.standard.string(forKey: "geminiKey") ?? ""
            apiKey = key.isEmpty ? nil : key
            baseUrl = nil
        case "xAI Grok":
            ffiType = .grok
            modelName = input.modelName.isEmpty ? "grok-2" : input.modelName
            let key = UserDefaults.standard.string(forKey: "grokKey") ?? ""
            apiKey = key.isEmpty ? nil : key
            baseUrl = nil
        case "Local Ollama/LM Studio":
            ffiType = .localOpenAiCompatible
            modelName = input.modelName.isEmpty ? "llama3" : input.modelName
            apiKey = nil
            baseUrl = input.baseUrl.isEmpty ? "http://localhost:11434/v1" : input.baseUrl
        default:
            ffiType = .mock
            modelName = "mock-model"
            apiKey = nil
            baseUrl = nil
        }
        
        return FfiExpert(
            id: id,
            name: "\(defaultName) (\(modelName))",
            config: FfiProviderConfig(
                name: defaultName,
                providerType: ffiType,
                modelName: modelName,
                baseUrl: baseUrl,
                apiKey: apiKey,
                temperature: 0.7
            ),
            systemPrompt: input.systemPrompt
        )
    }
    
    func runCouncil() {
        guard !prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        
        isExecuting = true
        chairmanText = ""
        chairmanStatus = "idle"
        chairmanError = nil
        
        // Dynamically build active FfiExperts based on activeExpertCount
        var activeExperts: [FfiExpert] = []
        self.expertStates.removeAll()
        
        for i in 0..<activeExpertCount {
            let config = expertsConfig[i]
            let expertId = "expert-\(i + 1)"
            let ffiExpert = buildFfiExpert(
                id: expertId,
                defaultName: config.name.isEmpty ? "Expert \(i + 1)" : config.name,
                input: config
            )
            activeExperts.append(ffiExpert)
            
            // Initialize states
            self.expertStates[expertId] = ExpertState(
                id: expertId,
                name: ffiExpert.name,
                status: "idle",
                response: "",
                critiqueStatus: "idle",
                critiqueResponse: ""
            )
        }
        
        let chairman = buildFfiExpert(
            id: "chairman-gem",
            defaultName: chairmanConfig.name.isEmpty ? "Chairman Gaston" : chairmanConfig.name,
            input: chairmanConfig
        )
        
        let council = FfiCouncil(
            id: "panel-development",
            name: "Software Architecture Council",
            experts: activeExperts,
            chairman: chairman,
            critiqueRounds: enableCritique ? 1 : 0
        )
        
        let currentPrompt = prompt
        
        // Format local files context to prepend to prompt
        var decoratedPrompt = ""
        var fileAttachmentsList = ""
        
        if !selectedFilePaths.isEmpty {
            decoratedPrompt += "Here are the selected files from the user's local workspace directory for your context:\n\n"
            var attachedRelativeNames: [String] = []
            for path in selectedFilePaths {
                let relativePath = path.replacingOccurrences(of: workspacePath + "/", with: "")
                attachedRelativeNames.append(relativePath)
                if let content = try? String(contentsOfFile: path, encoding: .utf8) {
                    decoratedPrompt += "=== File: \(relativePath) ===\n\(content)\n\n"
                }
            }
            decoratedPrompt += "=============================\n\n"
            fileAttachmentsList = "📎 Attached workspace files: [\(attachedRelativeNames.joined(separator: ", "))]\n\n"
        }
        
        decoratedPrompt += currentPrompt
        
        let ffiHistory = messages.map { msg in
            FfiMessage(
                id: msg.id,
                role: msg.role == "user" ? .user : .assistant,
                content: msg.content,
                timestamp: msg.timestamp
            )
        }
        
        // Append user prompt (with attachment list) to chat log bubble
        let userMsg = CodableMessage(
            id: UUID().uuidString,
            role: "user",
            content: "\(fileAttachmentsList)\(currentPrompt)",
            timestamp: UInt64(Date().timeIntervalSince1970)
        )
        messages.append(userMsg)
        saveSession()
        prompt = "" // clear input box immediately
        
        Task {
            do {
                _ = try await executeCouncilWorkflow(
                    prompt: decoratedPrompt,
                    history: ffiHistory,
                    council: council,
                    callback: self
                )
                DispatchQueue.main.async {
                    self.isExecuting = false
                }
            } catch {
                DispatchQueue.main.async {
                    self.isExecuting = false
                    self.chairmanStatus = "error"
                    self.chairmanError = error.localizedDescription
                }
            }
        }
    }
}
