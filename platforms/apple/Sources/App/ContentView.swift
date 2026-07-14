import SwiftUI

struct ContentView: View {
    @StateObject private var viewModel = CouncilViewModel()
    
    var body: some View {
        NavigationSplitView {
            // Sidebar controls
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    HStack {
                        Text("Council Control")
                            .font(.headline)
                            .foregroundColor(.primary)
                        Spacer()
                        SettingsLink {
                            Image(systemName: "gearshape")
                                .font(.title3)
                        }
                        .buttonStyle(.plain)
                        .help("Open Settings")
                    }
                    
                    Toggle("Enable Critique Loop", isOn: $viewModel.enableCritique)
                        .font(.subheadline)
                    
                    Divider()
                    
                    // Workspace Directory Integration (Milestone 7)
                    VStack(alignment: .leading, spacing: 10) {
                        Text("Workspace Directory")
                            .font(.subheadline)
                            .fontWeight(.semibold)
                            .foregroundColor(.secondary)
                        
                        if viewModel.workspacePath.isEmpty {
                            Button(action: {
                                viewModel.selectDirectory()
                            }) {
                                HStack {
                                    Image(systemName: "folder.badge.plus")
                                    Text("Select Directory...")
                                }
                                .frame(maxWidth: .infinity)
                            }
                            .buttonStyle(.bordered)
                        } else {
                            VStack(alignment: .leading, spacing: 6) {
                                HStack {
                                    Image(systemName: "folder.fill")
                                        .foregroundColor(.purple)
                                    Text(URL(fileURLWithPath: viewModel.workspacePath).lastPathComponent)
                                        .fontWeight(.medium)
                                        .lineLimit(1)
                                    Spacer()
                                    Button(action: {
                                        viewModel.selectDirectory()
                                    }) {
                                        Image(systemName: "pencil")
                                    }
                                    .buttonStyle(.plain)
                                    .help("Change Directory")
                                    
                                    Button(action: {
                                        viewModel.refreshFiles()
                                    }) {
                                        Image(systemName: "arrow.clockwise")
                                    }
                                    .buttonStyle(.plain)
                                    .help("Refresh Files")
                                }
                                .font(.caption)
                                
                                Text(viewModel.workspacePath)
                                    .font(.system(size: 9))
                                    .foregroundColor(.secondary)
                                    .lineLimit(1)
                                
                                if !viewModel.scannedFiles.isEmpty {
                                    DisclosureGroup("Workspace Files (\(viewModel.scannedFiles.count))") {
                                        ScrollView {
                                            VStack(alignment: .leading, spacing: 6) {
                                                ForEach(viewModel.scannedFiles, id: \.path) { fileURL in
                                                    let relPath = fileURL.path.replacingOccurrences(of: viewModel.workspacePath + "/", with: "")
                                                    let isSelected = viewModel.selectedFilePaths.contains(fileURL.path)
                                                    
                                                    Button(action: {
                                                        viewModel.toggleFileSelection(path: fileURL.path)
                                                    }) {
                                                        HStack(alignment: .top) {
                                                            Image(systemName: isSelected ? "checkmark.square.fill" : "square")
                                                                .foregroundColor(isSelected ? .purple : .secondary)
                                                            Text(relPath)
                                                                .font(.system(size: 10, design: .monospaced))
                                                                .foregroundColor(.primary)
                                                                .multilineTextAlignment(.leading)
                                                                .lineLimit(2)
                                                            Spacer()
                                                        }
                                                    }
                                                    .buttonStyle(.plain)
                                                }
                                            }
                                            .padding(.top, 4)
                                        }
                                        .frame(maxHeight: 150)
                                    }
                                    .font(.caption)
                                } else {
                                    Text("No text/code files found.")
                                        .font(.caption2)
                                        .foregroundColor(.secondary)
                                        .italic()
                                }
                            }
                            .padding(8)
                            .background(Color(NSColor.controlBackgroundColor).opacity(0.4))
                            .cornerRadius(6)
                        }
                    }
                    
                    Divider()
                    
                    // Collapsible Council configurations
                    VStack(alignment: .leading, spacing: 14) {
                        Text("Council Panel Setup")
                            .font(.subheadline)
                            .fontWeight(.semibold)
                            .foregroundColor(.secondary)
                        
                        ExpertConfigSection(title: "Expert 1 (Claudia)", config: $viewModel.expert1Config)
                        
                        ExpertConfigSection(title: "Expert 2 (Oliver)", config: $viewModel.expert2Config)
                        
                        ExpertConfigSection(title: "Chairman (Gaston)", config: $viewModel.chairmanConfig)
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
                    .padding(.top, 10)
                }
                .padding()
            }
            .frame(minWidth: 240)
            .navigationSplitViewColumnWidth(min: 240, ideal: 260, max: 320)
            
        } detail: {
            // Main Content Dashboard split vertically into Chat area and active drafts grid
            VStack(spacing: 0) {
                // Header Bar
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Council of Experts")
                            .font(.system(size: 20, weight: .bold, design: .rounded))
                        Text("Multi-turn consensus chat dashboard")
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                    Spacer()
                    Text("v0.7.0")
                        .font(.caption2)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(Color.purple.opacity(0.15))
                        .cornerRadius(6)
                }
                .padding()
                .background(.thinMaterial)
                
                Divider()
                
                // Conversational Chat History List
                ScrollViewReader { proxy in
                    ScrollView {
                        VStack(spacing: 16) {
                            if viewModel.messages.isEmpty {
                                VStack(spacing: 12) {
                                    Spacer()
                                    Image(systemName: "crown.fill")
                                        .font(.system(size: 40))
                                        .foregroundColor(.purple.opacity(0.3))
                                    Text("Begin a consensus dialogue with Gaston and his panel experts.")
                                        .font(.subheadline)
                                        .foregroundColor(.secondary)
                                        .multilineTextAlignment(.center)
                                        .padding(.horizontal, 40)
                                    Spacer()
                                }
                                .frame(minHeight: 250)
                            } else {
                                ForEach(viewModel.messages) { msg in
                                    ChatBubble(msg: msg)
                                        .id(msg.id)
                                }
                            }
                            
                            // Live Streaming Synthesis Bubble
                            if viewModel.isExecuting && viewModel.chairmanStatus == "synthesis" {
                                StreamingSynthesisBubble(text: viewModel.chairmanText)
                                    .id("streaming-synthesis")
                            }
                        }
                        .padding()
                    }
                    .onChange(of: viewModel.messages.count) { _, _ in
                        if let last = viewModel.messages.last {
                            withAnimation {
                                proxy.scrollTo(last.id, anchor: .bottom)
                            }
                        }
                    }
                    .onChange(of: viewModel.chairmanText) { _, _ in
                        if viewModel.chairmanStatus == "synthesis" {
                            proxy.scrollTo("streaming-synthesis", anchor: .bottom)
                        }
                    }
                }
                .background(Color(NSColor.windowBackgroundColor).opacity(0.95))
                
                Divider()
                
                // Collapsible active grid showing expert drafting progress
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        Text("Active Consensus Drafting Grid")
                            .font(.caption)
                            .fontWeight(.bold)
                            .foregroundColor(.secondary)
                        Spacer()
                        if viewModel.isExecuting {
                            ProgressView()
                                .controlSize(.small)
                        }
                    }
                    .padding(.horizontal)
                    .padding(.top, 8)
                    
                    LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 10) {
                        ForEach(Array(viewModel.expertStates.values.sorted(by: { $0.id < $1.id })), id: \.id) { state in
                            ExpertCardView(state: state)
                        }
                    }
                    .padding(.horizontal)
                    .padding(.bottom, 8)
                }
                .background(.thinMaterial)
                
                Divider()
                
                // Bottom Input Area
                VStack(spacing: 0) {
                    HStack(spacing: 12) {
                        VStack(alignment: .leading, spacing: 4) {
                            if !viewModel.selectedFilePaths.isEmpty {
                                HStack(spacing: 4) {
                                    Image(systemName: "paperclip")
                                        .font(.caption2)
                                    Text("\(viewModel.selectedFilePaths.count) workspace files attached")
                                        .font(.system(size: 9, weight: .semibold))
                                }
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(Color.purple.opacity(0.15))
                                .foregroundColor(.purple)
                                .cornerRadius(4)
                            }
                            
                            TextEditor(text: $viewModel.prompt)
                                .font(.system(.body, design: .monospaced))
                                .frame(height: 50)
                                .padding(4)
                                .background(Color(NSColor.controlBackgroundColor))
                                .cornerRadius(6)
                                .overlay(
                                    RoundedRectangle(cornerRadius: 6)
                                        .stroke(Color.secondary.opacity(0.2), lineWidth: 1)
                                )
                        }
                        
                        VStack(spacing: 6) {
                            Button(action: {
                                viewModel.runCouncil()
                            }) {
                                HStack {
                                    Image(systemName: "paperplane.fill")
                                    Text("Send")
                                }
                                .frame(width: 80)
                            }
                            .buttonStyle(.borderedProminent)
                            .tint(Color.purple)
                            .disabled(viewModel.isExecuting || viewModel.prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                            
                            Button(action: {
                                viewModel.clearHistory()
                            }) {
                                Text("Clear")
                                    .frame(width: 80)
                            }
                            .buttonStyle(.bordered)
                            .disabled(viewModel.isExecuting)
                        }
                    }
                    .padding()
                }
                .background(.thinMaterial)
            }
        }
        .frame(minWidth: 950, minHeight: 700)
    }
}

struct ExpertConfigSection: View {
    let title: String
    @Binding var config: ExpertConfigInput
    
    var body: some View {
        DisclosureGroup(title) {
            VStack(alignment: .leading, spacing: 8) {
                Picker("Provider", selection: $config.providerType) {
                    Text("Mock Sandbox").tag("Mock Sandbox")
                    Text("Anthropic Claude").tag("Anthropic Claude")
                    Text("OpenAI GPT").tag("OpenAI GPT")
                    Text("Google Gemini").tag("Google Gemini")
                    Text("xAI Grok").tag("xAI Grok")
                    Text("Local Model").tag("Local Ollama/LM Studio")
                }
                .pickerStyle(.menu)
                
                VStack(alignment: .leading, spacing: 2) {
                    Text("Model Name:")
                        .font(.system(size: 10))
                        .foregroundColor(.secondary)
                    TextField("e.g. llama3", text: $config.modelName)
                        .textFieldStyle(.roundedBorder)
                }
                
                if config.providerType == "Local Ollama/LM Studio" {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Base URL:")
                            .font(.system(size: 10))
                            .foregroundColor(.secondary)
                        TextField("http://localhost:11434/v1", text: $config.baseUrl)
                            .textFieldStyle(.roundedBorder)
                    }
                }
                
                VStack(alignment: .leading, spacing: 2) {
                    Text("Behavior Prompt:")
                        .font(.system(size: 10))
                        .foregroundColor(.secondary)
                    TextEditor(text: $config.systemPrompt)
                        .frame(height: 50)
                        .cornerRadius(4)
                        .overlay(
                            RoundedRectangle(cornerRadius: 4)
                                .stroke(Color.secondary.opacity(0.2), lineWidth: 1)
                        )
                }
            }
            .padding(.top, 6)
            .padding(.bottom, 4)
        }
    }
}

struct ChatBubble: View {
    let msg: CodableMessage
    
    var body: some View {
        HStack {
            if msg.role == "user" {
                Spacer()
                VStack(alignment: .trailing, spacing: 4) {
                    Text("You")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                    Text(msg.content)
                        .padding(10)
                        .background(Color.purple.opacity(0.85))
                        .foregroundColor(.white)
                        .cornerRadius(12)
                        .textSelection(.enabled)
                }
                .frame(maxWidth: 600, alignment: .trailing)
            } else {
                VStack(alignment: .leading, spacing: 4) {
                    HStack {
                        Image(systemName: "crown.fill")
                            .foregroundColor(Color.amber)
                            .font(.caption2)
                        Text("Gaston (Chairman)")
                            .font(.caption)
                            .fontWeight(.semibold)
                            .foregroundColor(.primary)
                    }
                    Text(msg.content)
                        .font(.system(.body, design: .serif))
                        .lineSpacing(4)
                        .padding(12)
                        .background(Color(NSColor.controlBackgroundColor).opacity(0.8))
                        .cornerRadius(12)
                        .overlay(
                            RoundedRectangle(cornerRadius: 12)
                                .stroke(Color.secondary.opacity(0.1), lineWidth: 1)
                        )
                        .textSelection(.enabled)
                }
                .frame(maxWidth: 700, alignment: .leading)
                Spacer()
            }
        }
    }
}

struct StreamingSynthesisBubble: View {
    let text: String
    
    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 4) {
                HStack {
                    ProgressView()
                        .controlSize(.small)
                        .padding(.trailing, 4)
                    Text("Gaston is synthesizing consensus...")
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .italic()
                }
                Text(text.isEmpty ? "Preparing final synthesis..." : text)
                    .font(.system(.body, design: .serif))
                    .lineSpacing(4)
                    .padding(12)
                    .background(Color(NSColor.controlBackgroundColor).opacity(0.5))
                    .cornerRadius(12)
                    .overlay(
                        RoundedRectangle(cornerRadius: 12)
                            .stroke(Color.purple.opacity(0.2), lineWidth: 1.5)
                    )
            }
            .frame(maxWidth: 700, alignment: .leading)
            Spacer()
        }
    }
}

struct ExpertCardView: View {
    let state: ExpertState
    @State private var selectedTab = 0
    
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text(state.name)
                    .font(.system(size: 11, weight: .bold))
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
                            .font(.system(size: 10))
                            .italic()
                    } else {
                        Text(state.response)
                            .font(.system(size: 10, design: .monospaced))
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
                            .font(.system(size: 10))
                            .italic()
                    } else {
                        Text(state.critiqueResponse)
                            .font(.system(size: 10, design: .monospaced))
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .textSelection(.enabled)
                    }
                }
            }
            .frame(height: 80)
        }
        .padding(8)
        .background(Color(NSColor.controlBackgroundColor).opacity(0.6))
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(isStreaming ? Color.blue.opacity(0.4) : Color.secondary.opacity(0.1), lineWidth: 1)
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
            .font(.system(size: 8, weight: .bold))
            .padding(.horizontal, 4)
            .padding(.vertical, 1)
            .background(badgeColor.opacity(0.15))
            .foregroundColor(badgeColor)
            .cornerRadius(3)
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
