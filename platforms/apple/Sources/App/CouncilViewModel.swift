import Foundation
import SwiftUI
import PanelOfExpertsKit

struct ExpertState {
    var id: String
    var name: String
    var status: String // "idle", "drafting", "completed", "error"
    var response: String
    var error: String?
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
    @Published var selectedMode: String = "Mock Sandbox" // "Mock Sandbox", "Live APIs"
    
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
    
    func runCouncil() {
        guard !prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        
        isExecuting = true
        chairmanText = ""
        chairmanStatus = "idle"
        chairmanError = nil
        
        let mode = selectedMode
        
        let modelName1: String
        let modelName2: String
        let chairmanModel: String
        let apiKey1: String?
        let apiKey2: String?
        let apiChKey: String?
        
        if mode == "Mock Sandbox" {
            modelName1 = "mock-expert-1"
            modelName2 = "mock-expert-2"
            chairmanModel = "mock-chairman"
            apiKey1 = nil
            apiKey2 = nil
            apiChKey = nil
        } else {
            modelName1 = "claude-3-5-sonnet-latest"
            modelName2 = "gpt-4o"
            chairmanModel = "gemini-1.5-pro"
            apiKey1 = anthropicKey.isEmpty ? nil : anthropicKey
            apiKey2 = openAiKey.isEmpty ? nil : openAiKey
            apiChKey = geminiKey.isEmpty ? nil : geminiKey
        }
        
        let expert1 = FfiExpert(
            id: "expert-claudia",
            name: "Claudia (Claude)",
            config: FfiProviderConfig(
                name: "Anthropic Claude",
                providerType: mode == "Mock Sandbox" ? .mock : .anthropic,
                modelName: modelName1,
                baseUrl: nil,
                apiKey: apiKey1,
                temperature: 0.5
            ),
            systemPrompt: "You are Claudia, a software developer focusing on type-safety, clean compilation boundaries, and architecture."
        )
        
        let expert2 = FfiExpert(
            id: "expert-oliver",
            name: "Oliver (OpenAI)",
            config: FfiProviderConfig(
                name: "OpenAI GPT-4o",
                providerType: mode == "Mock Sandbox" ? .mock : .openAi,
                modelName: modelName2,
                baseUrl: nil,
                apiKey: apiKey2,
                temperature: 0.7
            ),
            systemPrompt: "You are Oliver, a systems programmer focusing on extreme optimizations, memory management, and performance in low-level Rust/C++."
        )
        
        let chairman = FfiExpert(
            id: "chairman-gem",
            name: "Gaston (Gemini)",
            config: FfiProviderConfig(
                name: "Google Gemini Pro",
                providerType: mode == "Mock Sandbox" ? .mock : .gemini,
                modelName: chairmanModel,
                baseUrl: nil,
                apiKey: apiChKey,
                temperature: 0.6
            ),
            systemPrompt: "You are Gaston, the Chairman of the Council. Review the expert proposals and synthesize them into a clean, complete response."
        )
        
        let council = FfiCouncil(
            id: "panel-development",
            name: "Software Architecture Council",
            experts: [expert1, expert2],
            chairman: chairman
        )
        
        self.expertStates = [
            expert1.id: ExpertState(id: expert1.id, name: expert1.name, status: "idle", response: ""),
            expert2.id: ExpertState(id: expert2.id, name: expert2.name, status: "idle", response: "")
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
