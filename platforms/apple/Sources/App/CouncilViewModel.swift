import Foundation
import SwiftUI
import PanelOfExpertsKit

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
    var providerType: String // "Mock Sandbox", "Anthropic Claude", "OpenAI GPT", "Google Gemini", "Local Ollama/LM Studio"
    var modelName: String
    var baseUrl: String
}

class CouncilViewModel: ObservableObject, FfiCouncilCallback {
    @Published var expertStates: [String: ExpertState] = [:]
    @Published var chairmanText: String = ""
    @Published var chairmanStatus: String = "idle" // "idle", "synthesis", "completed", "error"
    @Published var chairmanError: String?
    @Published var isExecuting: Bool = false
    @Published var prompt: String = "Design a zero-dependency configuration parser in Rust."
    
    // Config values for live APIs
    @Published var openAiKey: String = ""
    @Published var anthropicKey: String = ""
    @Published var geminiKey: String = ""
    @Published var grokKey: String = ""
    @Published var enableCritique: Bool = true
    
    // Dynamic expert configuration inputs
    @Published var expert1Config = ExpertConfigInput(providerType: "Mock Sandbox", modelName: "mock-expert-1", baseUrl: "http://localhost:11434/v1")
    @Published var expert2Config = ExpertConfigInput(providerType: "Mock Sandbox", modelName: "mock-expert-2", baseUrl: "http://localhost:11434/v1")
    @Published var chairmanConfig = ExpertConfigInput(providerType: "Mock Sandbox", modelName: "mock-chairman", baseUrl: "http://localhost:11434/v1")
    
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
        }
    }
    
    func onChairmanError(error: String) {
        DispatchQueue.main.async {
            self.chairmanStatus = "error"
            self.chairmanError = error
        }
    }
    
    private func buildFfiExpert(id: String, defaultName: String, input: ExpertConfigInput, systemPrompt: String) -> FfiExpert {
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
            apiKey = anthropicKey.isEmpty ? nil : anthropicKey
            baseUrl = nil
        case "OpenAI GPT":
            ffiType = .openAi
            modelName = input.modelName.isEmpty ? "gpt-4o" : input.modelName
            apiKey = openAiKey.isEmpty ? nil : openAiKey
            baseUrl = nil
        case "Google Gemini":
            ffiType = .gemini
            modelName = input.modelName.isEmpty ? "gemini-1.5-pro" : input.modelName
            apiKey = geminiKey.isEmpty ? nil : geminiKey
            baseUrl = nil
        case "xAI Grok":
            ffiType = .grok
            modelName = input.modelName.isEmpty ? "grok-2" : input.modelName
            apiKey = grokKey.isEmpty ? nil : grokKey
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
            systemPrompt: systemPrompt
        )
    }
    
    func runCouncil() {
        guard !prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        
        isExecuting = true
        chairmanText = ""
        chairmanStatus = "idle"
        chairmanError = nil
        
        let expert1 = buildFfiExpert(
            id: "expert-claudia",
            defaultName: "Claudia",
            input: expert1Config,
            systemPrompt: "You are Claudia, a software developer focusing on type-safety, clean compilation boundaries, and architecture."
        )
        
        let expert2 = buildFfiExpert(
            id: "expert-oliver",
            defaultName: "Oliver",
            input: expert2Config,
            systemPrompt: "You are Oliver, a systems programmer focusing on extreme optimizations, memory management, and performance in low-level Rust/C++."
        )
        
        let chairman = buildFfiExpert(
            id: "chairman-gem",
            defaultName: "Gaston (Chairman)",
            input: chairmanConfig,
            systemPrompt: "You are Gaston, the Chairman of the Council. Review the expert proposals and critiques, then synthesize them into a clean, complete response."
        )
        
        let council = FfiCouncil(
            id: "panel-development",
            name: "Software Architecture Council",
            experts: [expert1, expert2],
            chairman: chairman,
            critiqueRounds: enableCritique ? 1 : 0
        )
        
        self.expertStates = [
            expert1.id: ExpertState(id: expert1.id, name: expert1.name, status: "idle", response: "", critiqueStatus: "idle", critiqueResponse: ""),
            expert2.id: ExpertState(id: expert2.id, name: expert2.name, status: "idle", response: "", critiqueStatus: "idle", critiqueResponse: "")
        ]
        
        Task {
            do {
                _ = try await executeCouncilWorkflow(
                    prompt: prompt,
                    history: [],
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
