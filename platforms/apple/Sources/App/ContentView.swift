import SwiftUI

struct ContentView: View {
    @StateObject private var viewModel = CouncilViewModel()
    
    var body: some View {
        NavigationSplitView {
            // Sidebar controls
            VStack(alignment: .leading, spacing: 20) {
                Text("Council Control")
                    .font(.headline)
                    .foregroundColor(.primary)
                
                Picker("Execution Mode", selection: $viewModel.selectedMode) {
                    Text("Mock Sandbox").tag("Mock Sandbox")
                    Text("Live APIs").tag("Live APIs")
                }
                .pickerStyle(.radioGroup)
                
                Toggle("Enable Critique Loop", isOn: $viewModel.enableCritique)
                    .font(.subheadline)
                    .padding(.vertical, 4)
                
                if viewModel.selectedMode == "Live APIs" {
                    VStack(alignment: .leading, spacing: 10) {
                        Text("API Credentials")
                            .font(.subheadline)
                            .fontWeight(.semibold)
                        
                        VStack(alignment: .leading, spacing: 4) {
                            Text("OpenAI Key:")
                                .font(.caption)
                            SecureField("sk-...", text: $viewModel.openAiKey)
                                .textFieldStyle(.roundedBorder)
                        }
                        
                        VStack(alignment: .leading, spacing: 4) {
                            Text("Anthropic Key:")
                                .font(.caption)
                            SecureField("sk-ant-...", text: $viewModel.anthropicKey)
                                .textFieldStyle(.roundedBorder)
                        }
                        
                        VStack(alignment: .leading, spacing: 4) {
                            Text("Gemini Key:")
                                .font(.caption)
                            SecureField("AIzaSy...", text: $viewModel.geminiKey)
                                .textFieldStyle(.roundedBorder)
                        }
                    }
                    .padding()
                    .background(Color.secondary.opacity(0.1))
                    .cornerRadius(8)
                }
                
                Spacer()
                
                // Status panel
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        Circle()
                            .fill(viewModel.isExecuting ? Color.green : Color.gray)
                            .frame(width: 8, height: 8)
                            .scaleEffect(viewModel.isExecuting ? 1.2 : 1.0)
                            .animation(viewModel.isExecuting ? .easeInOut(duration: 0.8).repeatForever(autoreverses: true) : .default, value: viewModel.isExecuting)
                        
                        Text(viewModel.isExecuting ? "Orchestrating..." : "System Idle")
                            .font(.caption)
                            .fontWeight(.medium)
                    }
                    
                    if viewModel.chairmanStatus == "synthesis" {
                        Text("Synthesizing drafts...")
                            .font(.caption2)
                            .foregroundColor(.secondary)
                    }
                }
                .padding(.bottom, 10)
            }
            .padding()
            .frame(minWidth: 220)
            .navigationSplitViewColumnWidth(min: 220, ideal: 240, max: 300)
            
        } detail: {
            // Main Content Dashboard
            ScrollView {
                VStack(spacing: 24) {
                    // Header Card
                    VStack(alignment: .leading, spacing: 6) {
                        HStack {
                            Text("Council of Experts")
                                .font(.system(size: 28, weight: .bold, design: .rounded))
                                .foregroundStyle(
                                    LinearGradient(
                                        colors: [Color.purple, Color.blue],
                                        startPoint: .leading,
                                        endPoint: .trailing
                                    )
                                )
                            Spacer()
                            Text("v0.2.0")
                                .font(.caption)
                                .padding(.horizontal, 8)
                                .padding(.vertical, 4)
                                .background(Color.blue.opacity(0.15))
                                .cornerRadius(8)
                        }
                        Text("A multi-agent consensus network synthesizing concurrent draft responses and critiques in real-time.")
                            .font(.subheadline)
                            .foregroundColor(.secondary)
                    }
                    .padding(.horizontal)
                    
                    // Prompt Input Container
                    VStack(alignment: .leading, spacing: 12) {
                        Text("Enter Query or Directive")
                            .font(.caption)
                            .fontWeight(.semibold)
                            .foregroundColor(.secondary)
                        
                        TextEditor(text: $viewModel.prompt)
                            .font(.system(.body, design: .monospaced))
                            .frame(height: 80)
                            .padding(8)
                            .background(Color(NSColor.controlBackgroundColor))
                            .cornerRadius(8)
                            .overlay(
                                RoundedRectangle(cornerRadius: 8)
                                    .stroke(Color.secondary.opacity(0.2), lineWidth: 1)
                            )
                        
                        Button(action: {
                            viewModel.runCouncil()
                        }) {
                            HStack {
                                Image(systemName: "play.fill")
                                Text("Execute Council Flow")
                            }
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 8)
                        }
                        .buttonStyle(.borderedProminent)
                        .tint(Color.purple)
                        .disabled(viewModel.isExecuting || viewModel.prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    }
                    .padding()
                    .background(.ultraThinMaterial)
                    .cornerRadius(12)
                    .padding(.horizontal)
                    
                    // Parallel Expert Stream Dashboard
                    VStack(alignment: .leading, spacing: 12) {
                        Text("Parallel Expert Draft Streams")
                            .font(.subheadline)
                            .fontWeight(.bold)
                            .foregroundColor(.secondary)
                            .padding(.horizontal)
                        
                        LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 16) {
                            ForEach(Array(viewModel.expertStates.values.sorted(by: { $0.id < $1.id })), id: \.id) { state in
                                ExpertCardView(state: state)
                            }
                        }
                        .padding(.horizontal)
                    }
                    
                    // Chairman synthesis Panel
                    VStack(alignment: .leading, spacing: 12) {
                        Text("Chairman Consensus Synthesis")
                            .font(.subheadline)
                            .fontWeight(.bold)
                            .foregroundColor(.secondary)
                            .padding(.horizontal)
                        
                        VStack(alignment: .leading, spacing: 16) {
                            HStack {
                                Image(systemName: "crown.fill")
                                    .foregroundColor(.amber)
                                Text("Gaston (Chairman Synthesis)")
                                    .font(.headline)
                                Spacer()
                                
                                if viewModel.chairmanStatus == "synthesis" {
                                    ProgressView()
                                        .controlSize(.small)
                                } else if viewModel.chairmanStatus == "completed" {
                                    Image(systemName: "checkmark.circle.fill")
                                        .foregroundColor(.green)
                                }
                            }
                            
                            Divider()
                            
                            if let err = viewModel.chairmanError {
                                Text("Synthesis Error: \(err)")
                                    .foregroundColor(.red)
                                    .font(.body)
                            } else if viewModel.chairmanText.isEmpty {
                                Text("Awaiting expert drafts...")
                                    .foregroundColor(.secondary)
                                    .font(.body)
                                    .italic()
                                    .frame(maxWidth: .infinity, alignment: .leading)
                            } else {
                                Text(viewModel.chairmanText)
                                    .font(.system(.body, design: .serif))
                                    .lineSpacing(6)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    .textSelection(.enabled)
                            }
                        }
                        .padding()
                        .background(.ultraThinMaterial)
                        .cornerRadius(12)
                        .overlay(
                            RoundedRectangle(cornerRadius: 12)
                                .stroke(viewModel.chairmanStatus == "synthesis" ? Color.purple.opacity(0.4) : Color.clear, lineWidth: 1.5)
                        )
                        .padding(.horizontal)
                    }
                }
                .padding(.vertical)
            }
            .frame(minWidth: 500)
            .background(Color(NSColor.windowBackgroundColor).opacity(0.95))
        }
        .frame(minWidth: 800, minHeight: 600)
    }
}

struct ExpertCardView: View {
    let state: ExpertState
    @State private var selectedTab = 0
    
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text(state.name)
                    .font(.headline)
                    .foregroundColor(.primary)
                Spacer()
                
                // Status badge
                StatusBadge(status: currentPhaseStatus)
            }
            
            // Tab Selector
            Picker("", selection: $selectedTab) {
                Text("Draft 1").tag(0)
                HStack(spacing: 4) {
                    Text("Critique")
                    if state.critiqueStatus == "drafting" {
                        Circle()
                            .fill(Color.blue)
                            .frame(width: 4, height: 4)
                    }
                }.tag(1)
            }
            .pickerStyle(.segmented)
            .onChange(of: state.critiqueStatus) { _, newValue in
                if newValue == "drafting" {
                    selectedTab = 1
                }
            }
            
            Divider()
            
            ScrollView {
                if selectedTab == 0 {
                    // Display Initial Draft
                    if let err = state.error, state.status == "error" {
                        Text("Draft Error: \(err)")
                            .foregroundColor(.red)
                            .font(.caption)
                    } else if state.response.isEmpty {
                        Text(state.status == "drafting" ? "Initiating draft stream..." : "Awaiting user query...")
                            .foregroundColor(.secondary)
                            .font(.caption)
                            .italic()
                    } else {
                        Text(state.response)
                            .font(.system(.body, design: .monospaced))
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .textSelection(.enabled)
                    }
                } else {
                    // Display Critique & Revision
                    if let err = state.error, state.critiqueStatus == "error" {
                        Text("Critique Error: \(err)")
                            .foregroundColor(.red)
                            .font(.caption)
                    } else if state.critiqueResponse.isEmpty {
                        Text(state.critiqueStatus == "drafting" ? "Streaming critiques..." : "Awaiting initial drafts...")
                            .foregroundColor(.secondary)
                            .font(.caption)
                            .italic()
                    } else {
                        Text(state.critiqueResponse)
                            .font(.system(.body, design: .monospaced))
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .textSelection(.enabled)
                    }
                }
            }
            .frame(height: 150)
        }
        .padding()
        .background(Color(NSColor.controlBackgroundColor).opacity(0.7))
        .cornerRadius(10)
        .overlay(
            RoundedRectangle(cornerRadius: 10)
                .stroke(isStreaming ? Color.blue.opacity(0.5) : Color.secondary.opacity(0.1), lineWidth: 1)
        )
    }
    
    private var currentPhaseStatus: String {
        if state.critiqueStatus != "idle" {
            return state.critiqueStatus == "drafting" ? "critiquing" : state.critiqueStatus
        }
        return state.status
    }
    
    private var isStreaming: Bool {
        state.status == "drafting" || state.critiqueStatus == "drafting"
    }
}

struct StatusBadge: View {
    let status: String
    
    var body: some View {
        Text(status.uppercased())
            .font(.system(size: 9, weight: .bold))
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(badgeColor.opacity(0.15))
            .foregroundColor(badgeColor)
            .cornerRadius(4)
    }
    
    private var badgeColor: Color {
        switch status {
        case "drafting", "critiquing": return .blue
        case "completed": return .green
        case "error": return .red
        default: return .gray
        }
    }
}

extension Color {
    static let amber = Color(red: 1.0, green: 0.75, blue: 0.0)
}

