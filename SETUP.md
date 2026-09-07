# Setup Guide - Zen Agentic Extension

This guide walks you through setting up and running the Zen Agentic Extension on macOS (Apple Silicon).

## Prerequisites

### System Requirements
- **OS**: macOS 12.3+ (for ScreenCaptureKit)
- **Hardware**: Apple Silicon (M1/M2/M3) recommended for optimal performance
- **Browser**: Zen Browser (latest version)

### Development Tools

1. **Install Xcode Command Line Tools**
   ```bash
   xcode-select --install
   ```

2. **Install Rust** (for native host)
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source $HOME/.cargo/env
   rustup default stable
   ```

3. **Install Node.js** (for web extension tooling)
   ```bash
   brew install node
   # Or use nvm: curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash
   ```

4. **Install Python** (for ML backend)
   ```bash
   brew install python@3.10
   # Or use pyenv: brew install pyenv
   ```

## Phase 1: Build Core Infrastructure

### Step 1: Build Native Host

```bash
cd zen-agentic-extension/native-host

# Build debug version
cargo build

# Build release version (optimized)
cargo build --release

# The binary will be at:
# Debug: target/debug/zen-agentic-native
# Release: target/release/zen-agentic-native
```

### Step 2: Create Native Messaging Manifest

Create a manifest file to register the native host with Firefox/Zen:

**macOS:** `~/Library/Application Support/Zen/NativeMessagingHosts/zen_agentic_native.json`

```json
{
  "name": "zen_agentic_native",
  "description": "Zen Agentic AI Native Host",
  "path": "/absolute/path/to/zen-agentic-extension/native-host/target/release/zen-agentic-native",
  "type": "stdio",
  "allowed_extensions": ["zen-agentic-ai@localhost"]
}
```

**Important**: Replace `/absolute/path/to/` with the actual path to your binary.

### Step 3: Grant Screen Capture Permission (macOS)

The native host needs screen recording permission:

1. Open **System Settings** → **Privacy & Security** → **Screen & System Audio Recording**
2. Find and enable `zen-agentic-native` (you may need to run it once first)
3. Restart Zen Browser after granting permission

### Step 4: Load Web Extension in Zen Browser

1. Open Zen Browser
2. Navigate to `about:debugging#/runtime/this-firefox` (or similar for Zen)
3. Click "Load Temporary Add-on"
4. Select the `manifest.json` file in `zen-agentic-extension/webextension/`

**Note**: Temporary add-ons are unloaded when browser closes. For permanent installation, use `web-ext`:

```bash
cd zen-agentic-extension/webextension
npm install
npx web-ext sign --api-key YOUR_API_KEY --api-secret YOUR_SECRET
```

### Step 5: Test Connection

1. Open Zen Browser sidebar (click extension icon)
2. Click "Connect" button
3. You should see "Connected to native host" message

## Testing Individual Components

### Test Native Host Directly

```bash
cd zen-agentic-extension/native-host

# Run with test input
echo '{"id":"test1","method":"ping","params":{}}' | cargo run --release
```

Expected output: Binary response with 4-byte length prefix + JSON

### Test NLP Processor

```bash
cd zen-agentic-extension/ml-backend

# Test with sample text
echo '{"id":"test1","method":"process_text","params":{"text":"This is a great product!","analysis_types":["sentiment","summary"]}}' | python3 nlp_processor.py
```

Expected output:
```json
{"id":"test1","result":{"text":"This is a great product!","language":"en","sentiment":{"positive":1.0,"negative":0.0,"neutral":0.5},"summary":"This is a great product!"}}
```

## Troubleshooting

### Native Host Won't Connect

1. Check that manifest file exists in correct location
2. Verify binary path in manifest is absolute and correct
3. Ensure binary is executable: `chmod +x path/to/zen-agentic-native`
4. Check stderr logs from native host

### Screen Capture Permission Issues

1. Remove existing permission: `tccutil reset ScreenCapture com.your.bundle.id`
2. Re-run native host to trigger permission request
3. Manually grant in System Settings

### Extension Not Loading

1. Check browser console for errors (Ctrl+Shift+J / Cmd+Opt+J)
2. Verify manifest.json syntax
3. Ensure all referenced files exist (sidebar.html, background.js, etc.)

## Next Steps (Future Phases)

### Phase 2: Visual Context Engine
- Implement ScreenCaptureKit integration in `src/screencapture/mod.rs`
- Add frame streaming to sidebar UI
- Implement hardware-accelerated encoding

### Phase 3: MCP & WebDriver Integration
- Complete Marionette protocol implementation
- Add DOM element UID mapping
- Implement click/type automation

### Phase 4: Full ML Integration
- Install spaCy: `pip install spacy && python -m spacy download en_core_web_sm`
- Install transformers: `pip install transformers sentence-transformers`
- Set up local LLM with Ollama: `brew install ollama`

## Project Structure Reference

```
zen-agentic-extension/
├── native-host/           # Rust binary
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs        # Entry point
│       ├── ipc.rs         # Message protocol
│       ├── mcp/           # MCP server
│       ├── screencapture/ # Screen capture
│       └── webdriver/     # DOM automation
├── webextension/          # Browser extension
│   ├── manifest.json
│   ├── background.js
│   ├── sidebar.js
│   └── sidebar.html
├── ml-backend/            # Python ML
│   ├── nlp_processor.py
│   └── requirements.txt
└── docs/                  # Documentation
```

## Useful Commands

```bash
# Build everything
cd native-host && cargo build --release
cd ../webextension && npm install

# Run tests
cd native-host && cargo test

# Format code
cd native-host && cargo fmt
cd ../webextension && npx eslint --fix .

# Check for issues
cd native-host && cargo clippy
```

## Resources

- [WebExtensions API (Mozilla)](https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions)
- [Native Messaging](https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Native_messaging)
- [ScreenCaptureKit Documentation](https://developer.apple.com/documentation/screencapturekit)
- [Model Context Protocol](https://www.anthropic.com/research/model-context-protocol)
- [Zen Browser Docs](https://docs.zen-browser.app/)
