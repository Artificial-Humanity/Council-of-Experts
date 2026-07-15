import SwiftUI
import CouncilOfExpertsKit

struct ContentView: View {
    @StateObject private var viewModel = CouncilViewModel()
    @State private var columnVisibility: NavigationSplitViewVisibility = .all
    @State private var draftingGridHeight: CGFloat = 130
    @State private var draftingGridDragStartHeight: CGFloat?

    var body: some View {
        NavigationSplitView(columnVisibility: $columnVisibility) {
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
                    
                    // Stepper for Active Experts Count (Limit 8)
                    VStack(alignment: .leading, spacing: 6) {
                        Stepper("Active Experts: \(viewModel.activeExpertCount)", value: $viewModel.activeExpertCount, in: 1...8)
                            .font(.subheadline)

                        Text("Limit: 1 - 8 panel members")
                            .font(.system(size: 9))
                            .foregroundColor(.secondary)
                    }

                    // Stepper for panel discussion rounds (opening statement ... closing statement)
                    VStack(alignment: .leading, spacing: 6) {
                        Stepper("Discussion Rounds: \(viewModel.councilRounds)", value: $viewModel.councilRounds, in: 2...10)
                            .font(.subheadline)

                        Text("Round 1 is opening statements, the last round is closing statements.")
                            .font(.system(size: 9))
                            .foregroundColor(.secondary)
                    }

                    VStack(alignment: .leading, spacing: 8) {
                        Toggle("Retry Once on Build Failure", isOn: $viewModel.enableCritique)
                            .font(.subheadline)
                            .help("When Agent Coding Mode's build fails, let the panel critique and retry once")

                        Toggle("Agent Coding Mode", isOn: $viewModel.isAgentCodingMode)
                            .font(.subheadline)
                            .foregroundColor(.purple)
                            .help("Allows experts to write workspace files and run builds automatically")
                    }
                    
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
                                
                                if viewModel.isAgentCodingMode {
                                    VStack(alignment: .leading, spacing: 4) {
                                        Text("Build & Test Command")
                                            .font(.system(size: 9, weight: .bold))
                                            .foregroundColor(.purple)
                                        TextField("e.g. cargo test or npm test", text: $viewModel.buildCommand)
                                            .textFieldStyle(.roundedBorder)
                                            .font(.system(size: 10, design: .monospaced))
                                    }
                                    .padding(.top, 4)
                                }
                                
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
                        
                        // Dynamically list settings only for active experts
                        ForEach(0..<viewModel.activeExpertCount, id: \.self) { idx in
                            ExpertConfigSection(
                                title: "Expert \(idx + 1) (\(viewModel.expertsConfig[idx].name))",
                                config: $viewModel.expertsConfig[idx]
                            )
                        }
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
                            
                            Text(viewModel.isExecuting ? (viewModel.isAgentCodingMode ? "Coding Agent Active..." : "Orchestrating...") : "System Idle")
                                .font(.caption)
                                .fontWeight(.medium)
                        }
                        
                        if let executionError = viewModel.executionError {
                            Text(executionError)
                                .font(.caption2)
                                .foregroundColor(.red)
                                .textSelection(.enabled)
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
                    Button(action: {
                        withAnimation {
                            columnVisibility = (columnVisibility == .detailOnly) ? .all : .detailOnly
                        }
                    }) {
                        Image(systemName: "sidebar.left")
                            .font(.title3)
                            .foregroundColor(.secondary)
                    }
                    .buttonStyle(.plain)
                    .help("Toggle Sidebar")

                    VStack(alignment: .leading, spacing: 2) {
                        Text("Council of Experts")
                            .font(.system(size: 20, weight: .bold, design: .rounded))
                        Text(viewModel.isAgentCodingMode ? "Multi-Source Agentic Coding Console" : "Multi-turn consensus chat dashboard")
                            .font(.caption)
                            .foregroundColor(viewModel.isAgentCodingMode ? .purple : .secondary)
                    }
                    Spacer()
                    Text("v0.8.0")
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
                                    Image(systemName: viewModel.isAgentCodingMode ? "terminal.fill" : "person.3.fill")
                                        .font(.system(size: 40))
                                        .foregroundColor(.purple.opacity(0.3))
                                    Text(viewModel.isAgentCodingMode ? "State a software engineering goal. The council will patch files and test their compilation." : "Begin a discussion with your panel of experts.")
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
                }
                .background(Color(NSColor.windowBackgroundColor).opacity(0.95))

                VerticalResizeHandle(height: $draftingGridHeight, dragStartHeight: $draftingGridDragStartHeight, minHeight: 60, maxHeight: 400)

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
                    
                    // Arrange columns dynamically (max 2 columns wide)
                    let columns = Array(repeating: GridItem(.flexible(), spacing: 10), count: min(viewModel.activeExpertCount, 2))
                    
                    ScrollView {
                        LazyVGrid(columns: columns, spacing: 10) {
                            ForEach(1...viewModel.activeExpertCount, id: \.self) { idx in
                                let expertId = "expert-\(idx)"
                                if let state = viewModel.expertStates[expertId] {
                                    ExpertCardView(state: state)
                                } else {
                                    let config = viewModel.expertsConfig[idx - 1]
                                    ExpertCardView(state: ExpertState(
                                        id: expertId,
                                        name: "\(config.name) (\(config.modelName.isEmpty ? "mock" : config.modelName))",
                                        status: "idle",
                                        response: "",
                                        critiqueStatus: "idle",
                                        critiqueResponse: ""
                                    ))
                                }
                            }
                        }
                        .padding(.horizontal)
                        .padding(.bottom, 8)
                    }
                    .frame(height: draftingGridHeight)
                }
                .background(.thinMaterial)
                
                Divider()
                
                // ── Agent Coding Build Console View (Milestone 8) ──
                if viewModel.isAgentCodingMode && !viewModel.buildStatusLog.isEmpty {
                    VStack(alignment: .leading, spacing: 4) {
                        HStack {
                            Image(systemName: "terminal.fill")
                                .font(.caption)
                                .foregroundColor(.purple)
                            Text("Agent Execution & Build Console")
                                .font(.caption)
                                .fontWeight(.bold)
                                .foregroundColor(.secondary)
                            Spacer()
                            Button(action: {
                                viewModel.buildStatusLog = ""
                            }) {
                                Image(systemName: "trash")
                                    .font(.caption2)
                            }
                            .buttonStyle(.plain)
                            .help("Clear Console")
                        }
                        .padding(.horizontal)
                        .padding(.top, 8)
                        
                        ScrollView {
                            Text(viewModel.buildStatusLog)
                                .font(.system(size: 10, design: .monospaced))
                                .foregroundColor(.green)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .padding(10)
                                .textSelection(.enabled)
                        }
                        .background(Color.black)
                        .cornerRadius(8)
                        .frame(height: 120)
                        .padding(.horizontal)
                        .padding(.bottom, 8)
                    }
                    .background(.thinMaterial)
                    Divider()
                }
                
                // Bottom Input Area
                VStack(spacing: 0) {
                    HStack(spacing: 12) {
                        VStack(alignment: .leading, spacing: 4) {
                            // Display active file attachments label
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
                            
                            // ── Display Attached Media Thumbnail Previews ──
                            if !viewModel.attachedImages.isEmpty {
                                ScrollView(.horizontal, showsIndicators: false) {
                                    HStack(spacing: 8) {
                                        ForEach(0..<viewModel.attachedImages.count, id: \.self) { idx in
                                            let url = viewModel.attachedImages[idx]
                                            if let nsImage = NSImage(contentsOf: url) {
                                                ZStack(alignment: .topTrailing) {
                                                    Image(nsImage: nsImage)
                                                        .resizable()
                                                        .aspectRatio(contentMode: .fill)
                                                        .frame(width: 46, height: 46)
                                                        .cornerRadius(6)
                                                        .clipped()
                                                    
                                                    Button(action: {
                                                        viewModel.removeAttachedImage(at: idx)
                                                    }) {
                                                        Image(systemName: "xmark.circle.fill")
                                                            .foregroundColor(.red)
                                                            .font(.caption2)
                                                    }
                                                    .buttonStyle(.plain)
                                                    .padding(.top, -2)
                                                    .padding(.trailing, -2)
                                                }
                                            }
                                        }
                                    }
                                    .padding(.vertical, 4)
                                }
                                .frame(height: 52)
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
                            HStack(spacing: 8) {
                                Button(action: {
                                    viewModel.attachImage()
                                }) {
                                    Image(systemName: "photo.on.rectangle.angled")
                                        .font(.title3)
                                        .foregroundColor(.secondary)
                                }
                                .buttonStyle(.plain)
                                .help("Attach Image")
                                .disabled(viewModel.isExecuting)
                                
                                Button(action: {
                                    viewModel.runCouncil()
                                }) {
                                    HStack {
                                        Image(systemName: "paperplane.fill")
                                        Text("Send")
                                    }
                                    .frame(width: 100)
                                }
                                .buttonStyle(.borderedProminent)
                                .tint(Color.purple)
                                .disabled(viewModel.isExecuting || viewModel.prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                            }
                            
                            Button(action: {
                                viewModel.clearHistory()
                            }) {
                                HStack {
                                    Image(systemName: "trash")
                                    Text("Clear Chat")
                                }
                                .frame(width: 100)
                            }
                            .buttonStyle(.bordered)
                            .disabled(viewModel.isExecuting)
                            .help("Clear the conversation and reset the drafting grid")
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

// A draggable divider that resizes the pane below it by adjusting a bound height.
struct VerticalResizeHandle: View {
    @Binding var height: CGFloat
    @Binding var dragStartHeight: CGFloat?
    let minHeight: CGFloat
    let maxHeight: CGFloat

    var body: some View {
        ZStack {
            Divider()
            Capsule()
                .fill(Color.secondary.opacity(0.4))
                .frame(width: 36, height: 4)
        }
        .frame(height: 8)
        .contentShape(Rectangle())
        .onHover { hovering in
            if hovering {
                NSCursor.resizeUpDown.push()
            } else {
                NSCursor.pop()
            }
        }
        .gesture(
            DragGesture(minimumDistance: 0)
                .onChanged { value in
                    let base = dragStartHeight ?? height
                    if dragStartHeight == nil { dragStartHeight = height }
                    height = min(maxHeight, max(minHeight, base - value.translation.height))
                }
                .onEnded { _ in
                    dragStartHeight = nil
                }
        )
    }
}

struct ExpertConfigSection: View {
    let title: String
    @Binding var config: ExpertConfigInput

    @State private var fetchedModels: [String] = []
    @State private var isFetchingModels: Bool = false
    @State private var fetchError: String?

    private func fetchModels() {
        isFetchingModels = true
        fetchError = nil
        let probeConfig = CouncilViewModel.probeConfig(providerType: config.providerType, baseUrl: config.baseUrl)

        Task {
            do {
                let models = try await listAvailableModels(config: probeConfig)
                await MainActor.run {
                    isFetchingModels = false
                    fetchedModels = models
                    if models.isEmpty {
                        fetchError = "No models returned"
                    }
                }
            } catch {
                await MainActor.run {
                    isFetchingModels = false
                    fetchedModels = []
                    fetchError = error.localizedDescription
                }
            }
        }
    }

    var body: some View {
        DisclosureGroup(title) {
            VStack(alignment: .leading, spacing: 8) {
                // Configurable Custom Name field
                VStack(alignment: .leading, spacing: 2) {
                    Text("Name:")
                        .font(.system(size: 10))
                        .foregroundColor(.secondary)
                    TextField("e.g. Claudia", text: $config.name)
                        .textFieldStyle(.roundedBorder)
                }
                
                Picker("Provider", selection: $config.providerType) {
                    Text("Mock Sandbox").tag("Mock Sandbox")
                    Text("Anthropic Claude").tag("Anthropic Claude")
                    Text("OpenAI GPT").tag("OpenAI GPT")
                    Text("Google Gemini").tag("Google Gemini")
                    Text("xAI Grok").tag("xAI Grok")
                    Text("Local Model").tag("Local Ollama/LM Studio")
                }
                .pickerStyle(.menu)
                .onChange(of: config.providerType) {
                    fetchedModels = []
                    fetchError = nil
                }

                VStack(alignment: .leading, spacing: 2) {
                    Text("Model Name:")
                        .font(.system(size: 10))
                        .foregroundColor(.secondary)
                    HStack(spacing: 6) {
                        TextField("e.g. llama3", text: $config.modelName)
                            .textFieldStyle(.roundedBorder)

                        if !fetchedModels.isEmpty {
                            Menu {
                                ForEach(fetchedModels, id: \.self) { model in
                                    Button(model) {
                                        config.modelName = model
                                    }
                                }
                            } label: {
                                Image(systemName: "list.bullet")
                            }
                            .menuStyle(.borderlessButton)
                            .fixedSize()
                            .help("Pick from the models returned by the provider")
                        }

                        Button {
                            fetchModels()
                        } label: {
                            if isFetchingModels {
                                ProgressView()
                                    .controlSize(.small)
                            } else {
                                Image(systemName: "arrow.clockwise")
                            }
                        }
                        .buttonStyle(.borderless)
                        .disabled(isFetchingModels || config.providerType == "Mock Sandbox")
                        .help("Query the provider for available models")
                    }
                    if let fetchError {
                        Text(fetchError)
                            .font(.system(size: 9))
                            .foregroundColor(.red)
                            .textSelection(.enabled)
                    }
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
                    
                    VStack(alignment: .trailing, spacing: 6) {
                        // Render Staged Image Attachments Inline inside bubble
                        if let paths = msg.attachedImagePaths, !paths.isEmpty {
                            ScrollView(.horizontal, showsIndicators: false) {
                                HStack(spacing: 8) {
                                    ForEach(paths, id: \.self) { path in
                                        let url = URL(fileURLWithPath: path)
                                        if let nsImage = NSImage(contentsOf: url) {
                                            Image(nsImage: nsImage)
                                                .resizable()
                                                .aspectRatio(contentMode: .fit)
                                                .frame(maxHeight: 120)
                                                .cornerRadius(8)
                                                .overlay(
                                                    RoundedRectangle(cornerRadius: 8)
                                                        .stroke(Color.white.opacity(0.2), lineWidth: 1)
                                                )
                                        }
                                    }
                                }
                            }
                            .frame(maxWidth: 400)
                        }
                        
                        Text(msg.content)
                            .textSelection(.enabled)
                    }
                    .padding(10)
                    .background(Color.purple.opacity(0.85))
                    .foregroundColor(.white)
                    .cornerRadius(12)
                }
                .frame(maxWidth: 600, alignment: .trailing)
            } else {
                // "expert-draft" / "expert-critique", plus legacy "chairman"/"assistant" sessions
                VStack(alignment: .leading, spacing: 4) {
                    HStack {
                        Image(systemName: msg.role == "expert-critique" ? "arrow.triangle.2.circlepath" : "person.fill")
                            .foregroundColor(.blue)
                            .font(.caption2)
                        Text(msg.speakerName ?? "Expert")
                            .font(.caption)
                            .fontWeight(.semibold)
                            .foregroundColor(.primary)
                        Text(msg.stageLabel ?? (msg.role == "expert-critique" ? "Critique & Revision" : "Draft"))
                            .font(.system(size: 9, weight: .medium))
                            .foregroundColor(.secondary)
                            .padding(.horizontal, 5)
                            .padding(.vertical, 1)
                            .background(Color.blue.opacity(0.12))
                            .cornerRadius(4)
                    }
                    Text(msg.content)
                        .lineSpacing(3)
                        .padding(12)
                        .background(Color(NSColor.controlBackgroundColor).opacity(0.6))
                        .cornerRadius(12)
                        .overlay(
                            RoundedRectangle(cornerRadius: 12)
                                .stroke(Color.blue.opacity(0.15), lineWidth: 1)
                        )
                        .textSelection(.enabled)
                }
                .frame(maxWidth: 700, alignment: .leading)
                Spacer()
            }
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
                    .lineLimit(1)
                Spacer()
                
                // Status badge
                StatusBadge(status: currentPhaseStatus)
            }
            
            // Tab Selector
            Picker("", selection: $selectedTab) {
                Text("Opening").tag(0)
                HStack(spacing: 4) {
                    Text("Latest")
                    if state.critiqueStatus == "drafting" {
                        Circle()
                            .fill(Color.blue)
                            .frame(width: 4, height: 4)
                    }
                }.tag(1)
            }
            .segment(selectedTab: $selectedTab, state: state)

            Divider()

            ScrollView {
                VStack(alignment: .leading, spacing: 4) {
                    if selectedTab == 0 {
                        if !state.thinking.isEmpty {
                            ThinkingNoteView(text: state.thinking)
                        }
                        // Display Initial Draft
                        if let err = state.error, state.status == "error" {
                            Text("Draft Error: \(err)")
                                .foregroundColor(.red)
                                .font(.caption2)
                                .textSelection(.enabled)
                        } else if state.response.isEmpty {
                            Text(state.status == "drafting" ? "Initiating draft stream..." : "Awaiting user query...")
                                .foregroundColor(.secondary)
                                .font(.system(size: 9))
                                .italic()
                        } else {
                            Text(state.response)
                                .font(.system(size: 9, design: .monospaced))
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .textSelection(.enabled)
                        }
                    } else {
                        if !state.critiqueThinking.isEmpty {
                            ThinkingNoteView(text: state.critiqueThinking)
                        }
                        // Display the most recent round's output
                        if let err = state.error, state.critiqueStatus == "error" {
                            Text("Round Error: \(err)")
                                .foregroundColor(.red)
                                .font(.caption2)
                                .textSelection(.enabled)
                        } else if state.critiqueResponse.isEmpty {
                            Text(state.critiqueStatus == "drafting" ? "Streaming..." : "Awaiting opening statements...")
                                .foregroundColor(.secondary)
                                .font(.system(size: 9))
                                .italic()
                        } else {
                            Text(state.critiqueResponse)
                                .font(.system(size: 9, design: .monospaced))
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .textSelection(.enabled)
                        }
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .frame(height: 44)
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

// Segmentation helper extension to auto-manage tab selection during streaming updates
extension View {
    func segment(selectedTab: Binding<Int>, state: ExpertState) -> some View {
        self.pickerStyle(.segmented)
            .onChange(of: state.critiqueStatus) { _, newValue in
                if newValue == "drafting" {
                    selectedTab.wrappedValue = 1
                }
            }
    }
}

// Collapsed-by-default reasoning trace, shown above a round's response when the provider
// returned one (Anthropic extended thinking, Gemini thought summaries).
struct ThinkingNoteView: View {
    let text: String
    @State private var isExpanded = false

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Button(action: { isExpanded.toggle() }) {
                HStack(spacing: 3) {
                    Image(systemName: isExpanded ? "chevron.down" : "chevron.right")
                        .font(.system(size: 7))
                    Image(systemName: "brain")
                        .font(.system(size: 8))
                    Text("Thinking")
                        .font(.system(size: 9, weight: .medium))
                }
                .foregroundColor(.purple.opacity(0.8))
            }
            .buttonStyle(.plain)

            if isExpanded {
                Text(text)
                    .font(.system(size: 9, design: .monospaced))
                    .italic()
                    .foregroundColor(.secondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .textSelection(.enabled)
                    .padding(.leading, 10)
            }
        }
        .padding(.bottom, 2)
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

