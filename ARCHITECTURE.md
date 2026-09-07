--- zen-agentic-extension/docs/ARCHITECTURE.md (原始)


+++ zen-agentic-extension/docs/ARCHITECTURE.md (修改后)
# Architecture Documentation

## System Overview

The Zen Agentic Extension implements a modular, microservice-inspired architecture that bridges Zen Browser with local AI agents through four interconnected subsystems.

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Zen Browser (Gecko)                          │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │              WebExtension (sidebar_action)                   │    │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │    │
│  │  │   Sidebar    │  │   Content    │  │   Network    │       │    │
│  │  │     UI       │  │   Scripts    │  │   Monitor    │       │    │
│  │  │  (Next.js)   │  │              │  │              │       │    │
│  │  └──────┬───────┘  └──────────────┘  └──────────────┘       │    │
│  │         │                                                    │    │
│  │  ┌──────▼────────────────────────────────────────────────┐   │    │
│  │  │           Background Script (Persistent)               │   │    │
│  │  │  • Native Messaging Connection                         │   │    │
│  │  │  • DOM Observer (MutationObserver)                     │   │    │
│  │  │  • Message Routing                                     │   │    │
│  │  └──────┬────────────────────────────────────────────────┘   │    │
│  └─────────│────────────────────────────────────────────────────┘    │
│            │ connectNative() - stdio                                  │
└────────────│──────────────────────────────────────────────────────────┘
             │
┌────────────▼──────────────────────────────────────────────────────────┐
│                    Native Host (Rust Binary)                           │
│  ┌─────────────────────────────────────────────────────────────┐     │
│  │              Message Loop (Async IO)                         │     │
│  │         32-bit Length-Prefixed JSON-RPC Protocol             │     │
│  └──────┬──────────────────────────────────────────────────────┘     │
│         │                                                             │
│  ┌──────▼──────────┐  ┌──────────────┐  ┌──────────────┐            │
│  │      MCP        │  │    Screen    │  │   WebDriver  │            │
│  │    Server       │  │   Capture    │  │     BiDi     │            │
│  │                 │  │  (macOS SCK) │  │  (Marionette)│            │
│  │  • Tools List   │  │              │  │              │            │
│  │  • Tool Calls   │  │  • Window    │  │  • DOM       │            │
│  │  • Resources    │  │    Enum      │  │    Snapshot  │            │
│  └──────┬──────────┘  │  • Frame     │  │  • Click     │            │
│         │             │    Capture   │  │  • Type      │            │
│         │             └──────────────┘  └──────────────┘            │
│         │                                                             │
└─────────│─────────────────────────────────────────────────────────────┘
          │ subprocess (stdin/stdout)
┌─────────▼─────────────────────────────────────────────────────────────┐
│                      ML Backend (Python)                               │
│  ┌─────────────────────────────────────────────────────────────┐     │
│  │                  NLP Processor                               │     │
│  │                                                              │     │
│  │  Phase 1-2: Rule-based heuristics                           │     │
│  │  • Sentiment analysis (keyword matching)                    │     │
│  │  • Entity extraction (regex patterns)                       │     │
│  │  • Summarization (extractive)                               │     │
│  │  • Keyword extraction (frequency-based)                     │     │
│  │                                                              │     │
│  │  Phase 4: Transformer models                                 │     │
│  │  • spaCy for NER                                            │     │
│  │  • DistilBERT for sentiment                                 │     │
│  │  • MiniLM for embeddings                                    │     │
│  │  • ChromaDB for vector storage                              │     │
│  └─────────────────────────────────────────────────────────────┘     │
└───────────────────────────────────────────────────────────────────────┘
```

## Component Details

### 1. WebExtension (Browser Layer)

**Technology**: Firefox WebExtensions API (Manifest V2)

**Key Files**:
- `manifest.json` - Extension configuration
- `background.js` - Persistent background script
- `sidebar.html` / `sidebar.js` - User interface
- `src/` - Source files (copied to root for simplicity)

**Responsibilities**:
- Render sidebar UI in Zen Browser's vertical layout
- Maintain persistent connection to native host via `browser.runtime.connectNative()`
- Observe DOM mutations using `MutationObserver`
- Monitor network requests via `webRequest` API
- Route messages between UI and native host

**Advantages over Manifest V3**:
- Persistent background scripts (not ephemeral service workers)
- Full blocking web request capabilities
- No arbitrary timeouts or execution limits

### 2. Native Host (Systems Layer)

**Technology**: Rust with Tokio async runtime

**Key Files**:
- `Cargo.toml` - Dependencies and build config
- `src/main.rs` - Entry point and message loop
- `src/ipc.rs` - 32-bit length-prefixed messaging
- `src/mcp/mod.rs` - Model Context Protocol server
- `src/screencapture/mod.rs` - Platform-specific screen capture
- `src/webdriver/mod.rs` - WebDriver BiDi integration

**Message Protocol**:
```
┌─────────────────┬─────────────────────────────────┐
│  4-byte length  │     UTF-8 JSON payload          │
│  (little-endian)│     (max 1 MB)                  │
└─────────────────┴─────────────────────────────────┘
```

**Responsibilities**:
- Spawn as subprocess from browser extension
- Handle stdio communication with precise framing
- Route messages to appropriate subsystems
- Manage platform-specific APIs (ScreenCaptureKit, etc.)
- Expose MCP tools to external AI clients

### 3. MCP Server (AI Integration Layer)

**Technology**: Model Context Protocol specification

**Default Tools**:
| Tool | Description | Requires Confirmation |
|------|-------------|----------------------|
| `take_snapshot` | Capture DOM state with UIDs | No |
| `click_by_uid` | Click element by UID | Yes |
| `evaluate_script` | Execute JavaScript | Yes |
| `list_network_requests` | Get network activity | No |
| `get_screen_capture` | Capture browser window | No |
| `extract_page_content` | NLP content analysis | No |

**Integration Points**:
- Claude Desktop (via .mcpb extensions)
- Cursor IDE
- Windsurf
- Zed editor

### 4. ML Backend (Intelligence Layer)

**Technology**: Python with optional ML libraries

**Key Files**:
- `nlp_processor.py` - Text processing pipeline
- `requirements.txt` - Dependencies (minimal for Phase 1)

**Analysis Capabilities**:
- **Sentiment Analysis**: Positive/negative/neutral scores
- **Entity Extraction**: Emails, URLs, money, dates
- **Summarization**: Extractive (first N sentences)
- **Keyword Extraction**: Frequency-based with stopword removal

**Phase 4 Upgrades**:
- Local LLM via Ollama/Llama.cpp
- Embedding models for RAG
- Vector database for browsing history

## Communication Flow

### Example: User Requests Page Analysis

```
1. User clicks "Analyze Page" in sidebar
        ↓
2. sidebar.js sends message to background.js
        ↓
3. background.js forwards to native host via connectNative()
        ↓
4. Native host parses 32-bit prefixed JSON
        ↓
5. MCP server routes to extract_page_content tool
        ↓
6. Native host spawns Python subprocess
        ↓
7. NLP processor analyzes text, returns results
        ↓
8. Results flow back through same path
        ↓
9. Sidebar displays analysis to user
```

## Security Considerations

### Sandboxing
- WebExtension runs in browser sandbox
- Native host has full user-space access
- ML backend isolated in subprocess

### Permission Gates
- Screen capture requires macOS TCC approval
- Destructive operations (click, eval) require HITL confirmation
- WebDriver automation uses isolated profiles

### Fingerprinting Mitigation
- Native messaging bypasses `navigator.webdriver` flag
- Interactions indistinguishable from human input
- Marionette should only be enabled on isolated profiles

## Platform Support Matrix

| Feature | macOS | Windows | Linux |
|---------|-------|---------|-------|
| Native Messaging | ✅ | ✅ | ✅ |
| Screen Capture | ✅ (SCK) | 🔄 (DXGI) | ❌ |
| WebDriver BiDi | ✅ | ✅ | ✅ |
| MCP Server | ✅ | ✅ | ✅ |
| ML Backend | ✅ | ✅ | ✅ |

## Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| IPC Latency | <10ms | Local stdio communication |
| Screen Capture | 60 FPS | Zero-copy via Metal/DXGI |
| CPU Overhead | <2% | Single core during capture |
| Memory Usage | <200MB | Native host + ML backend |
| DOM Snapshot | <100ms | For typical pages |

## Future Enhancements

### Phase 2: Visual Context
- Complete ScreenCaptureKit implementation
- Hardware-accelerated encoding (VideoToolbox)
- Frame streaming to sidebar UI

### Phase 3: Automation
- Full Marionette protocol support
- UID mapping for all interactive elements
- Multi-step workflow automation

### Phase 4: Advanced ML
- Local LLM integration (Ollama)
- Agentic RAG with browsing history
- Real-time sentiment filtering
- Cross-tab context awareness
