import Foundation
import SwiftUI
import UniformTypeIdentifiers
import CouncilOfExpertsKit

struct ExpertState {
    var id: String
    var name: String
    var status: String // "idle", "drafting", "completed", "error"
    var response: String
    var thinking: String = "" // reasoning trace for the opening round, if the provider returned any
    var critiqueStatus: String // "idle", "drafting", "completed", "error"
    var critiqueResponse: String
    var critiqueThinking: String = "" // reasoning trace for the most recent later round
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
    var role: String // "user", "expert-draft", "expert-critique" (legacy sessions may have "chairman" or "assistant")
    var content: String
    var timestamp: UInt64
    var attachedImagePaths: [String]?
    var speakerName: String? // display name for expert-draft/expert-critique bubbles
    var stageLabel: String? // "Opening Statement", "Round 2", "Closing Statement", etc.
}

class CouncilViewModel: ObservableObject, FfiCouncilCallback, FfiCodingCallback {
    @Published var expertStates: [String: ExpertState] = [:]
    // Top-level errors not tied to any single expert (e.g. a failure before any expert starts).
    @Published var executionError: String?
    @Published var isExecuting: Bool = false
    @Published var prompt: String = ""
    
    // Conversation history log
    @Published var messages: [CodableMessage] = []
    
    // Config values
    @Published var enableCritique: Bool = true

    // Panel discussion rounds: round 1 is the opening statement, the last round is the
    // closing statement, anything in between is a reaction round. Minimum 2.
    @Published var councilRounds: Int = 3 {
        didSet {
            let clamped = min(max(2, councilRounds), 10)
            if councilRounds != clamped {
                councilRounds = clamped
                return
            }
            UserDefaults.standard.set(councilRounds, forKey: "councilRounds")
        }
    }
    
    // Workspace Directory Integration (Milestone 7)
    @Published var workspacePath: String = ""
    @Published var scannedFiles: [URL] = []
    @Published var selectedFilePaths: Set<String> = []
    
    // Coding Agent Mode Integration (Milestone 8)
    @Published var isAgentCodingMode: Bool = false {
        didSet {
            UserDefaults.standard.set(isAgentCodingMode, forKey: "isAgentCodingMode")
        }
    }
    @Published var buildCommand: String = "" {
        didSet {
            UserDefaults.standard.set(buildCommand, forKey: "buildCommand")
        }
    }
    @Published var buildStatusLog: String = ""
    
    // Multimodal Image Attachments (Milestone 6)
    @Published var attachedImages: [URL] = []
    
    // Dynamic expert configuration settings (Limit of 8 active experts)
    // Indexes directly into expertsConfig, so it must never exceed that array's size — an
    // out-of-range value restored from a stale defaults plist would crash the drafting grid.
    @Published var activeExpertCount: Int = 2 {
        didSet {
            let clamped = min(max(1, activeExpertCount), expertsConfig.count)
            if activeExpertCount != clamped {
                activeExpertCount = clamped
                return
            }
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
    
    init() {
        loadSession()
        
        // Load active expert count
        if let savedCount = UserDefaults.standard.object(forKey: "activeExpertCount") as? Int {
            self.activeExpertCount = min(max(1, savedCount), expertsConfig.count)
        }

        // Load council discussion round count
        if let savedRounds = UserDefaults.standard.object(forKey: "councilRounds") as? Int {
            self.councilRounds = min(max(2, savedRounds), 10)
        }
        
        // Load workspace path
        if let savedPath = UserDefaults.standard.string(forKey: "workspacePath") {
            self.workspacePath = savedPath
            refreshFiles()
        }
        
        // Load coding agent properties
        self.isAgentCodingMode = UserDefaults.standard.bool(forKey: "isAgentCodingMode")
        if let savedBuildCmd = UserDefaults.standard.string(forKey: "buildCommand") {
            self.buildCommand = savedBuildCmd
        }
    }
    
    // ── Session Storage Helpers ──
    private var sessionURL: URL {
        let paths = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)
        let supportDir = paths[0].appendingPathComponent("io.artificialhumanity.council-of-experts", isDirectory: true)
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
        
        // Clean staging attachments folder
        let paths = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)
        let attachmentsDir = paths[0]
            .appendingPathComponent("io.artificialhumanity.council-of-experts", isDirectory: true)
            .appendingPathComponent("attachments", isDirectory: true)
        try? FileManager.default.removeItem(at: attachmentsDir)
        
        executionError = nil
        buildStatusLog = ""
        expertStates.removeAll()
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
    
    // Build outputs and vendored dependencies: pointing the picker at any real project
    // otherwise buries its source files under thousands of generated ones.
    private static let ignoredDirectories: Set<String> = [
        ".git", ".hg", ".svn", ".build", "build", "target", "DerivedData",
        "node_modules", "vendor", "Pods", "dist", "out",
        "__pycache__", ".venv", "venv", ".tox", ".mypy_cache", ".pytest_cache",
        ".next", ".nuxt", ".parcel-cache", ".gradle", ".idea", ".cache"
    ]

    private static let binaryExtensions: Set<String> = [
        "png", "jpg", "jpeg", "gif", "webp", "ico", "icns", "pdf",
        "zip", "tar", "gz", "bz2", "xz", "7z", "rar",
        "dylib", "a", "so", "exe", "bin", "o", "obj", "d",
        "app", "framework", "xcframework", "swiftmodule", "swiftdoc", "swiftsourceinfo",
        "mp3", "mp4", "mov", "wav", "avi", "woff", "woff2", "ttf", "otf",
        "db", "sqlite", "pyc", "class", "jar"
    ]

    // Files larger than this are indexed but not inlined into a prompt — a single large
    // file can cost more than the rest of the conversation combined.
    private static let maxAttachedFileBytes = 256 * 1024

    func refreshFiles() {
        guard !workspacePath.isEmpty else {
            self.scannedFiles = []
            return
        }

        let path = workspacePath
        DispatchQueue.global(qos: .userInitiated).async {
            let fileManager = FileManager.default
            let url = URL(fileURLWithPath: path)
            guard let enumerator = fileManager.enumerator(at: url, includingPropertiesForKeys: [.isRegularFileKey, .isDirectoryKey], options: [.skipsHiddenFiles, .skipsPackageDescendants]) else {
                DispatchQueue.main.async {
                    self.scannedFiles = []
                }
                return
            }

            var files: [URL] = []
            for case let fileURL as URL in enumerator {
                do {
                    let resourceValues = try fileURL.resourceValues(forKeys: [.isRegularFileKey, .isDirectoryKey])

                    if resourceValues.isDirectory ?? false {
                        if Self.ignoredDirectories.contains(fileURL.lastPathComponent) {
                            enumerator.skipDescendants()
                        }
                        continue
                    }

                    if resourceValues.isRegularFile ?? false {
                        let ext = fileURL.pathExtension.lowercased()
                        if !Self.binaryExtensions.contains(ext) {
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
    
    // ── Media Attachments Selection (Milestone 6) ──
    func attachImage() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = true
        panel.allowedContentTypes = [.image, .png, .jpeg, .webP, .gif]
        panel.title = "Attach Images to Council Query"
        
        if panel.runModal() == .OK {
            DispatchQueue.main.async {
                self.attachedImages.append(contentsOf: panel.urls)
            }
        }
    }
    
    func removeAttachedImage(at index: Int) {
        if index >= 0 && index < attachedImages.count {
            attachedImages.remove(at: index)
        }
    }
    
    private func copyImageToStaging(_ url: URL) -> URL? {
        let paths = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)
        let attachmentsDir = paths[0]
            .appendingPathComponent("io.artificialhumanity.council-of-experts", isDirectory: true)
            .appendingPathComponent("attachments", isDirectory: true)
        try? FileManager.default.createDirectory(at: attachmentsDir, withIntermediateDirectories: true)
        
        let destination = attachmentsDir.appendingPathComponent(UUID().uuidString + "_" + url.lastPathComponent)
        do {
            try FileManager.default.copyItem(at: url, to: destination)
            return destination
        } catch {
            print("Failed to copy image: \(error)")
            return nil
        }
    }
    
    private func getMimeType(for ext: String) -> String {
        switch ext.lowercased() {
        case "png": return "image/png"
        case "jpg", "jpeg": return "image/jpeg"
        case "gif": return "image/gif"
        case "webp": return "image/webp"
        default: return "image/jpeg"
        }
    }
    
    // ── Coding Agent Callbacks (Milestone 8) ──
    func onFileWrite(path: String) {
        DispatchQueue.main.async {
            self.buildStatusLog += "📁 Wrote file to workspace: \(path)\n"
            self.refreshFiles() // Refresh indexed files view list
        }
    }

    func onWorkspaceWarning(message: String) {
        DispatchQueue.main.async {
            self.buildStatusLog += "⚠️ \(message)\n"
        }
    }
    
    func onBuildStarted(command: String) {
        DispatchQueue.main.async {
            self.buildStatusLog += "⚙️ Running workspace build command: \(command)...\n"
        }
    }
    
    func onBuildCompleted(success: Bool, output: String) {
        DispatchQueue.main.async {
            let statusEmoji = success ? "✅" : "❌"
            self.buildStatusLog += "\(statusEmoji) Build finished (Success: \(success))\n"
            self.buildStatusLog += "--- Output ---\n\(output)--------------\n\n"
        }
    }
    
    // ── Drafting Phase Callbacks ──
    func onExpertStarted(expertId: String) {
        DispatchQueue.main.async {
            if var state = self.expertStates[expertId] {
                state.status = "drafting"
                state.response = ""
                state.thinking = ""
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

    func onExpertThinkingChunk(expertId: String, chunk: String) {
        DispatchQueue.main.async {
            if var state = self.expertStates[expertId] {
                state.thinking += chunk
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

                self.messages.append(CodableMessage(
                    id: UUID().uuidString,
                    role: "expert-draft",
                    content: fullResponse,
                    timestamp: UInt64(Date().timeIntervalSince1970),
                    speakerName: state.name,
                    stageLabel: "Opening Statement"
                ))
                self.saveSession()
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
    
    // ── Later-Round Callbacks (round 2 through the closing round) ──
    func onExpertCritiqueStarted(expertId: String, roundNumber: UInt32, isFinalRound: Bool) {
        DispatchQueue.main.async {
            if var state = self.expertStates[expertId] {
                state.critiqueStatus = "drafting"
                state.critiqueResponse = ""
                state.critiqueThinking = ""
                state.error = nil
                self.expertStates[expertId] = state
            }
        }
    }

    func onExpertCritiqueChunk(expertId: String, roundNumber: UInt32, chunk: String) {
        DispatchQueue.main.async {
            if var state = self.expertStates[expertId] {
                state.critiqueResponse += chunk
                self.expertStates[expertId] = state
            }
        }
    }

    func onExpertCritiqueThinkingChunk(expertId: String, roundNumber: UInt32, chunk: String) {
        DispatchQueue.main.async {
            if var state = self.expertStates[expertId] {
                state.critiqueThinking += chunk
                self.expertStates[expertId] = state
            }
        }
    }

    func onExpertCritiqueCompleted(expertId: String, roundNumber: UInt32, isFinalRound: Bool, fullCritique: String) {
        DispatchQueue.main.async {
            if var state = self.expertStates[expertId] {
                state.critiqueStatus = "completed"
                state.critiqueResponse = fullCritique
                self.expertStates[expertId] = state

                self.messages.append(CodableMessage(
                    id: UUID().uuidString,
                    role: "expert-critique",
                    content: fullCritique,
                    timestamp: UInt64(Date().timeIntervalSince1970),
                    speakerName: state.name,
                    stageLabel: isFinalRound ? "Closing Statement" : "Round \(roundNumber)"
                ))
                self.saveSession()
            }
        }
    }

    func onExpertCritiqueError(expertId: String, roundNumber: UInt32, error: String) {
        DispatchQueue.main.async {
            if var state = self.expertStates[expertId] {
                state.critiqueStatus = "error"
                state.error = error
                self.expertStates[expertId] = state
            }
        }
    }
    
    // Resolves an app-facing provider type string into the FFI provider type,
    // effective model name, stored API key, and effective base URL. Shared by
    // request building and by the "fetch available models" probe so the two
    // stay in sync.
    static func resolveProviderDetails(providerType: String, modelName: String, baseUrl: String) -> (type: FfiProviderType, modelName: String, apiKey: String?, baseUrl: String?) {
        switch providerType {
        case "Mock Sandbox":
            return (.mock, modelName.isEmpty ? "mock-model" : modelName, nil, nil)
        case "Anthropic Claude":
            let key = KeychainStore.read("anthropicKey")
            return (.anthropic, modelName.isEmpty ? "claude-sonnet-5" : modelName, key.isEmpty ? nil : key, nil)
        case "OpenAI GPT":
            let key = KeychainStore.read("openAiKey")
            return (.openAi, modelName.isEmpty ? "gpt-4o" : modelName, key.isEmpty ? nil : key, nil)
        case "Google Gemini":
            let key = KeychainStore.read("geminiKey")
            return (.gemini, modelName.isEmpty ? "gemini-2.5-pro" : modelName, key.isEmpty ? nil : key, nil)
        case "xAI Grok":
            let key = KeychainStore.read("grokKey")
            return (.grok, modelName.isEmpty ? "grok-2" : modelName, key.isEmpty ? nil : key, nil)
        case "Local Ollama/LM Studio":
            return (.localOpenAiCompatible, modelName.isEmpty ? "llama3" : modelName, nil, baseUrl.isEmpty ? "http://localhost:11434/v1" : baseUrl)
        default:
            return (.mock, "mock-model", nil, nil)
        }
    }

    // Builds a probe config (no model name required) for querying a provider's model list.
    static func probeConfig(providerType: String, baseUrl: String) -> FfiProviderConfig {
        let resolved = resolveProviderDetails(providerType: providerType, modelName: "", baseUrl: baseUrl)
        return FfiProviderConfig(
            name: "probe",
            providerType: resolved.type,
            modelName: resolved.modelName,
            baseUrl: resolved.baseUrl,
            apiKey: resolved.apiKey,
            temperature: nil,
            enableThinking: false
        )
    }

    private func buildFfiExpert(id: String, defaultName: String, input: ExpertConfigInput) -> FfiExpert {
        let resolved = CouncilViewModel.resolveProviderDetails(providerType: input.providerType, modelName: input.modelName, baseUrl: input.baseUrl)
        let enableThinking = UserDefaults.standard.bool(forKey: "enableThinkingNotes")

        return FfiExpert(
            id: id,
            name: "\(defaultName) (\(resolved.modelName))",
            config: FfiProviderConfig(
                name: defaultName,
                providerType: resolved.type,
                modelName: resolved.modelName,
                baseUrl: resolved.baseUrl,
                apiKey: resolved.apiKey,
                temperature: 0.7,
                enableThinking: enableThinking
            ),
            systemPrompt: input.systemPrompt
        )
    }
    
    // Asks the core to stop after the in-flight requests settle. The rounds still in the
    // queue are skipped, so a runaway discussion stops costing tokens immediately.
    func stopRun() {
        guard isExecuting else { return }
        cancelActiveRun()
        buildStatusLog += "🛑 Cancellation requested — finishing the current round.\n"
    }

    func runCouncil() {
        guard !prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }

        isExecuting = true
        executionError = nil
        buildStatusLog = ""
        
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
        
        let storedMaxWords = UserDefaults.standard.object(forKey: "maxResponseWords") as? Int
        let maxResponseWords = max(100, storedMaxWords ?? 300)

        let council = FfiCouncil(
            id: "panel-development",
            name: "Software Architecture Council",
            experts: activeExperts,
            critiqueRounds: enableCritique ? 1 : 0,
            rounds: UInt32(max(2, councilRounds)),
            maxResponseWords: UInt32(maxResponseWords)
        )
        
        let currentPrompt = prompt
        
        // Format local files context to prepend to prompt
        var decoratedPrompt = ""
        var fileAttachmentsList = ""
        
        if !selectedFilePaths.isEmpty {
            decoratedPrompt += "Here are the selected files from the user's local workspace directory for your context:\n\n"
            var attachedRelativeNames: [String] = []
            // Sorted so the same selection produces the same prompt every turn, which keeps
            // provider-side prompt caching effective.
            for path in selectedFilePaths.sorted() {
                let relativePath = path.replacingOccurrences(of: workspacePath + "/", with: "")
                attachedRelativeNames.append(relativePath)

                guard let content = try? String(contentsOfFile: path, encoding: .utf8) else { continue }

                if content.utf8.count > Self.maxAttachedFileBytes {
                    decoratedPrompt += "=== File: \(relativePath) ===\n[skipped: file is larger than \(Self.maxAttachedFileBytes / 1024) KB]\n\n"
                } else {
                    decoratedPrompt += "=== File: \(relativePath) ===\n\(content)\n\n"
                }
            }
            decoratedPrompt += "=============================\n\n"
            fileAttachmentsList = "📎 Attached workspace files: [\(attachedRelativeNames.joined(separator: ", "))]\n\n"
        }
        
        decoratedPrompt += currentPrompt
        
        // ── Stage Attached Media Images and Encode to Base64 FFI records ──
        var ffiAttachments: [FfiAttachment] = []
        var stagedPaths: [String] = []
        
        for url in attachedImages {
            if let stagedURL = copyImageToStaging(url) {
                stagedPaths.append(stagedURL.path)
                if let data = try? Data(contentsOf: stagedURL) {
                    let base64 = data.base64EncodedString()
                    let mime = getMimeType(for: stagedURL.pathExtension)
                    ffiAttachments.append(
                        FfiAttachment(
                            filePath: stagedURL.path,
                            mimeType: mime,
                            base64Data: base64
                        )
                    )
                }
            }
        }
        
        // Clear media selections for next input
        attachedImages.removeAll()
        
        // Expert turns keep their author across the FFI so the core can label them. Without
        // that, every panelist is handed every other panelist's words as its own prior turn.
        let ffiHistory = messages.map { msg -> FfiMessage in
            let role: FfiRole
            switch msg.role {
            case "user":
                role = .user
            case "expert-draft":
                role = .expertDraft(expertId: msg.speakerName ?? "Expert")
            case "expert-critique":
                role = .expertCritique(expertId: msg.speakerName ?? "Expert")
            default:
                role = .assistant // legacy "chairman"/"assistant" sessions
            }

            return FfiMessage(
                id: msg.id,
                role: role,
                content: msg.content,
                timestamp: msg.timestamp
            )
        }
        
        // Append user prompt (with attachment lists and image paths) to chat log bubble
        let userMsg = CodableMessage(
            id: UUID().uuidString,
            role: "user",
            content: "\(fileAttachmentsList)\(currentPrompt)",
            timestamp: UInt64(Date().timeIntervalSince1970),
            attachedImagePaths: stagedPaths.isEmpty ? nil : stagedPaths
        )
        messages.append(userMsg)
        saveSession()
        prompt = "" // clear input box immediately
        
        if isAgentCodingMode {
            buildStatusLog = "🚀 Initializing Multi-Source Coding Agent workflow...\n"
            Task {
                do {
                    _ = try await executeCodingWorkflow(
                        prompt: decoratedPrompt,
                        workspacePath: workspacePath,
                        buildCommand: buildCommand,
                        attachments: ffiAttachments,
                        history: ffiHistory,
                        council: council,
                        callback: self
                    )
                    await MainActor.run {
                        self.isExecuting = false
                    }
                } catch {
                    let description = error.localizedDescription
                    await MainActor.run {
                        self.isExecuting = false
                        self.executionError = description
                        self.buildStatusLog += "❌ Workflow Execution Error: \(description)\n"
                    }
                }
            }
        } else {
            Task {
                do {
                    _ = try await executeCouncilWorkflow(
                        prompt: decoratedPrompt,
                        attachments: ffiAttachments,
                        history: ffiHistory,
                        council: council,
                        callback: self
                    )
                    await MainActor.run {
                        self.isExecuting = false
                    }
                } catch {
                    let description = error.localizedDescription
                    await MainActor.run {
                        self.isExecuting = false
                        self.executionError = description
                    }
                }
            }
        }
    }
}
