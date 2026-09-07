/**
 * Sidebar UI script for Zen Agentic AI Extension
 *
 * Handles:
 * - Chat interface for AI interaction
 * - Tool button actions
 * - Connection status display
 * - Message routing to background script
 */

// DOM Elements
const connectBtn = document.getElementById('connectBtn');
const connectionStatus = document.getElementById('connectionStatus');
const chatMessages = document.getElementById('chatMessages');
const chatInput = document.getElementById('chatInput');
const sendBtn = document.getElementById('sendBtn');
const permissionBanner = document.getElementById('permissionBanner');
const btnSnapshot = document.getElementById('btnSnapshot');
const btnScreenCapture = document.getElementById('btnScreenCapture');
const btnAnalyze = document.getElementById('btnAnalyze');
const btnNetwork = document.getElementById('btnNetwork');
const domStats = document.getElementById('domStats');
const networkStats = document.getElementById('networkStats');

// State
let isConnected = false;
let messageId = 0;

/**
 * Generate unique message ID
 */
function generateMessageId() {
    return `msg_${Date.now()}_${messageId++}`;
}

/**
 * Add message to chat
 */
function addMessage(content, type = 'system') {
    const messageDiv = document.createElement('div');
    messageDiv.className = `message ${type}`;

    if (type === 'loading') {
        messageDiv.innerHTML = '<span class="loading"></span> Processing...';
    } else {
        // Simple text content - in production, sanitize HTML
        messageDiv.textContent = typeof content === 'string' ? content : JSON.stringify(content, null, 2);
    }

    chatMessages.appendChild(messageDiv);
    chatMessages.scrollTop = chatMessages.scrollHeight;

    return messageDiv;
}

/**
 * Update connection status UI
 */
function updateConnectionStatus(connected) {
    isConnected = connected;

    if (connected) {
        connectionStatus.classList.add('connected');
        connectBtn.textContent = 'Disconnect';
        connectBtn.classList.remove('connect');
        connectBtn.classList.add('disconnect');

        // Enable tool buttons
        btnSnapshot.disabled = false;
        btnScreenCapture.disabled = false;
        btnAnalyze.disabled = false;
        btnNetwork.disabled = false;

        addMessage('Connected to native host. Ready to assist!', 'system');
    } else {
        connectionStatus.classList.remove('connected');
        connectBtn.textContent = 'Connect';
        connectBtn.classList.add('connect');
        connectBtn.classList.remove('disconnect');

        // Disable tool buttons
        btnSnapshot.disabled = true;
        btnScreenCapture.disabled = true;
        btnAnalyze.disabled = true;
        btnNetwork.disabled = true;
    }
}

/**
 * Send message to background script
 */
function sendToBackground(action, data = {}) {
    return browser.runtime.sendMessage({ action, ...data })
        .then(response => {
            if (response.error) {
                throw new Error(response.error);
            }
            return response;
        });
}

/**
 * Send message to native host via background
 */
function sendToNative(method, params = {}) {
    const message = {
        id: generateMessageId(),
        method,
        params
    };

    return sendToBackground('send_to_native', { data: message });
}

/**
 * Handle chat input
 */
async function handleChatSubmit() {
    const text = chatInput.value.trim();
    if (!text || !isConnected) return;

    // Add user message
    addMessage(text, 'user');
    chatInput.value = '';
    sendBtn.disabled = true;

    // Show loading indicator
    const loadingMsg = addMessage('', 'loading');

    try {
        // Send query to native host (which routes to MCP/LLM)
        await sendToNative('call_mcp_tool', {
            tool_name: 'extract_page_content',
            arguments: {
                query: text,
                analysis_types: ['summary', 'entities']
            }
        });

        // In Phase 1, just acknowledge - actual LLM integration comes later
        loadingMsg.remove();
        addMessage('Query received. LLM integration coming in Phase 4.', 'assistant');

    } catch (error) {
        loadingMsg.remove();
        addMessage(`Error: ${error.message}`, 'error');
    } finally {
        sendBtn.disabled = false;
        chatInput.focus();
    }
}

/**
 * Take DOM snapshot
 */
async function handleSnapshot() {
    addMessage('Taking page snapshot...', 'system');

    try {
        const snapshot = await sendToBackground('get_dom_snapshot');
        addMessage(
            `Page: ${snapshot.title}\nURL: ${snapshot.url}\nElements: ${snapshot.elementCount}\nInteractive: ${snapshot.interactiveElements}`,
            'assistant'
        );
        domStats.textContent = `DOM: ${snapshot.elementCount}`;
    } catch (error) {
        addMessage(`Snapshot failed: ${error.message}`, 'error');
    }
}

/**
 * Request screen capture
 */
async function handleScreenCapture() {
    addMessage('Requesting screen capture...', 'system');

    try {
        // First check permission
        const permResponse = await sendToNative('check_screen_capture_permission');

        if (!permResponse.has_permission) {
            permissionBanner.classList.add('visible');
            addMessage('Screen capture permission not granted. Please follow the instructions above.', 'error');
            return;
        }

        // Request frame capture
        await sendToNative('capture_frame');
        addMessage('Screen capture initiated. Frame will appear when ready.', 'assistant');

    } catch (error) {
        addMessage(`Screen capture failed: ${error.message}`, 'error');
    }
}

/**
 * Analyze page content
 */
async function handleAnalyze() {
    addMessage('Analyzing page content...', 'system');

    try {
        await sendToNative('call_mcp_tool', {
            tool_name: 'extract_page_content',
            arguments: {
                analysis_types: ['sentiment', 'entities', 'summary', 'keywords']
            }
        });

        addMessage('Analysis started. Results will appear when processing is complete.', 'assistant');

    } catch (error) {
        addMessage(`Analysis failed: ${error.message}`, 'error');
    }
}

/**
 * Get network information
 */
async function handleNetwork() {
    addMessage('Fetching network requests...', 'system');

    try {
        const requests = await sendToBackground('get_network_requests', { limit: 20 });

        if (requests.length === 0) {
            addMessage('No recent network requests found.', 'system');
            return;
        }

        let summary = `Found ${requests.length} recent requests:\n\n`;
        requests.slice(-5).forEach(req => {
            const status = req.status || req.error || '?';
            summary += `${req.method} ${status} - ${req.url.substring(0, 50)}...\n`;
        });

        addMessage(summary, 'assistant');
        networkStats.textContent = `Network: ${requests.length}`;

    } catch (error) {
        addMessage(`Network info failed: ${error.message}`, 'error');
    }
}

/**
 * Initialize sidebar
 */
function init() {
    console.log('[Zen Agentic Sidebar] Initializing...');

    // Connection button handler
    connectBtn.addEventListener('click', () => {
        if (isConnected) {
            sendToBackground('disconnect_native');
            updateConnectionStatus(false);
        } else {
            addMessage('Connecting to native host...', 'system');
            sendToBackground('connect_native');
        }
    });

    // Chat handlers
    sendBtn.addEventListener('click', handleChatSubmit);
    chatInput.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            handleChatSubmit();
        }
    });

    // Tool button handlers
    btnSnapshot.addEventListener('click', handleSnapshot);
    btnScreenCapture.addEventListener('click', handleScreenCapture);
    btnAnalyze.addEventListener('click', handleAnalyze);
    btnNetwork.addEventListener('click', handleNetwork);

    // Listen for messages from background
    browser.runtime.onMessage.addListener((message) => {
        console.log('[Zen Agentic Sidebar] Received:', message);

        if (message.from !== 'background') return;

        switch (message.type) {
            case 'connection':
                updateConnectionStatus(message.status === 'connected');
                break;

            case 'error':
                addMessage(message.message, 'error');
                break;

            case 'screen_capture_frame':
                // Handle incoming frame data
                addMessage('New screen capture frame received', 'assistant');
                break;

            case 'mcp_tool_result':
                // Handle MCP tool results
                if (message.result) {
                    addMessage(JSON.stringify(message.result, null, 2), 'assistant');
                }
                break;

            case 'permission_status':
                if (!message.has_permission) {
                    permissionBanner.classList.add('visible');
                }
                break;
        }
    });

    // Check initial connection state
    sendToBackground('ping')
        .then(response => {
            updateConnectionStatus(response.connected);
        })
        .catch(() => {
            updateConnectionStatus(false);
        });

    // Initial DOM stats
    sendToBackground('get_dom_snapshot')
        .then(snapshot => {
            domStats.textContent = `DOM: ${snapshot.elementCount}`;
        })
        .catch(() => {});
}

// Start when DOM is ready
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
} else {
    init();
}
