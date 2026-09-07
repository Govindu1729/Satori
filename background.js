/**
 * Background script for Zen Agentic AI Extension
 *
 * Handles:
 * - Native messaging connection to Rust host
 * - DOM observation and mutation tracking
 * - Message routing between sidebar and native host
 * - Network request monitoring
 */

// Native messaging port
let nativePort = null;
let reconnectAttempts = 0;
const MAX_RECONNECT_ATTEMPTS = 5;

// Message queue for when native host is not connected
const messageQueue = [];

// Connected state
let isConnected = false;

/**
 * Connect to the native messaging host
 */
function connectToNativeHost() {
    try {
        console.log('[Zen Agentic] Connecting to native host...');

        nativePort = browser.runtime.connectNative('zen_agentic_native');

        nativePort.onMessage.addListener((response) => {
            console.log('[Zen Agentic] Received from native:', response);
            handleNativeMessage(response);
        });

        nativePort.onDisconnect.addListener(() => {
            console.log('[Zen Agentic] Disconnected from native host');
            isConnected = false;
            nativePort = null;

            // Attempt reconnection
            if (reconnectAttempts < MAX_RECONNECT_ATTEMPTS) {
                reconnectAttempts++;
                console.log(`[Zen Agentic] Reconnecting... attempt ${reconnectAttempts}`);
                setTimeout(connectToNativeHost, 2000 * reconnectAttempts);
            } else {
                console.error('[Zen Agentic] Max reconnection attempts reached');
                notifySidebar({
                    type: 'error',
                    message: 'Native host connection failed. Please ensure the native binary is installed.'
                });
            }
        });

        isConnected = true;
        reconnectAttempts = 0;
        console.log('[Zen Agentic] Connected to native host');

        // Flush message queue
        while (messageQueue.length > 0 && isConnected) {
            const msg = messageQueue.shift();
            sendToNative(msg);
        }

        // Notify sidebar of connection status
        notifySidebar({ type: 'connection', status: 'connected' });

    } catch (error) {
        console.error('[Zen Agentic] Failed to connect to native host:', error);
        isConnected = false;
        notifySidebar({
            type: 'error',
            message: `Connection failed: ${error.message}`
        });
    }
}

/**
 * Send message to native host
 */
function sendToNative(message) {
    if (!isConnected || !nativePort) {
        console.warn('[Zen Agentic] Not connected, queuing message');
        messageQueue.push(message);
        return Promise.reject(new Error('Not connected to native host'));
    }

    try {
        nativePort.postMessage(message);
        return Promise.resolve();
    } catch (error) {
        console.error('[Zen Agentic] Failed to send to native host:', error);
        isConnected = false;
        return Promise.reject(error);
    }
}

/**
 * Handle messages from native host
 */
function handleNativeMessage(message) {
    // Route to appropriate handler based on message type
    switch (message.type) {
        case 'screen_capture_frame':
            // Forward frame data to sidebar
            notifySidebar(message);
            break;
        case 'mcp_tool_result':
            // Forward tool result to sidebar
            notifySidebar(message);
            break;
        case 'permission_status':
            // Update permission UI
            notifySidebar(message);
            break;
        default:
            console.log('[Zen Agentic] Unhandled native message:', message);
            notifySidebar(message);
    }
}

/**
 * Send message to sidebar panel
 */
function notifySidebar(message) {
    browser.runtime.sendMessage({
        from: 'background',
        ...message
    }).catch((error) => {
        // Sidebar might not be open, which is fine
        if (error.message.includes('Could not establish connection')) {
            // Silently ignore - sidebar not open
        } else {
            console.warn('[Zen Agentic] Failed to notify sidebar:', error);
        }
    });
}

/**
 * DOM Observer for tracking page changes
 */
class DOMObserver {
    constructor() {
        this.observer = null;
        this.pendingMutations = [];
        this.debounceTimer = null;
    }

    start() {
        if (this.observer) return;

        this.observer = new MutationObserver((mutations) => {
            this.handleMutations(mutations);
        });

        this.observer.observe(document.documentElement, {
            childList: true,
            subtree: true,
            attributes: true,
            characterData: true,
            attributeOldValue: true,
            characterDataOldValue: true
        });

        console.log('[Zen Agentic] DOM Observer started');
    }

    stop() {
        if (this.observer) {
            this.observer.disconnect();
            this.observer = null;
            console.log('[Zen Agentic] DOM Observer stopped');
        }
    }

    handleMutations(mutations) {
        this.pendingMutations.push(...mutations);

        // Debounce mutation reporting
        clearTimeout(this.debounceTimer);
        this.debounceTimer = setTimeout(() => {
            this.reportMutations();
        }, 500);
    }

    reportMutations() {
        if (this.pendingMutations.length === 0) return;

        const summary = {
            type: 'dom_mutations',
            timestamp: Date.now(),
            url: window.location.href,
            title: document.title,
            mutationCount: this.pendingMutations.length,
            mutations: this.pendingMutations.map(m => ({
                type: m.type,
                tagName: m.target.tagName,
                addedNodes: m.addedNodes?.length || 0,
                removedNodes: m.removedNodes?.length || 0,
                attributeName: m.attributeName
            }))
        };

        // Send to background script
        browser.runtime.sendMessage(summary).catch(() => {});

        this.pendingMutations = [];
    }

    getSnapshot() {
        // Generate a lightweight DOM snapshot
        return {
            url: window.location.href,
            title: document.title,
            elementCount: document.getElementsByTagName('*').length,
            interactiveElements: document.querySelectorAll('a, button, input, select, textarea, [role="button"], [onclick]').length
        };
    }
}

// Global DOM observer instance
const domObserver = new DOMObserver();

/**
 * Network request monitoring
 */
class NetworkMonitor {
    constructor() {
        this.requests = new Map();
        this.maxRequests = 100;
    }

    start() {
        // Listen to webRequest events (requires permission)
        browser.webRequest?.onCompleted?.addListener((details) => {
            this.recordRequest(details);
        }, { urls: ['<all_urls>'] });

        browser.webRequest?.onErrorOccurred?.addListener((details) => {
            this.recordError(details);
        }, { urls: ['<all_urls>'] });
    }

    recordRequest(details) {
        this.requests.set(details.requestId, {
            url: details.url,
            method: details.method,
            status: details.statusCode,
            timestamp: details.timeStamp,
            type: details.type
        });

        // Trim old requests
        if (this.requests.size > this.maxRequests) {
            const firstKey = this.requests.keys().next().value;
            this.requests.delete(firstKey);
        }
    }

    recordError(details) {
        this.requests.set(details.requestId, {
            url: details.url,
            method: details.method,
            error: details.error,
            timestamp: details.timeStamp,
            type: details.type
        });
    }

    getRecentRequests(limit = 50, urlFilter = null) {
        let requests = Array.from(this.requests.values());

        if (urlFilter) {
            requests = requests.filter(r => r.url.includes(urlFilter));
        }

        return requests.slice(-limit);
    }
}

const networkMonitor = new NetworkMonitor();

/**
 * Message handler from sidebar/content scripts
 */
browser.runtime.onMessage.addListener((message, sender, sendResponse) => {
    console.log('[Zen Agentic] Received message:', message);

    switch (message.action) {
        case 'ping':
            sendResponse({ pong: true, connected: isConnected });
            break;

        case 'connect_native':
            connectToNativeHost();
            sendResponse({ status: 'connecting' });
            break;

        case 'disconnect_native':
            if (nativePort) {
                nativePort.disconnect();
            }
            sendResponse({ status: 'disconnected' });
            break;

        case 'send_to_native':
            sendToNative(message.data)
                .then(() => sendResponse({ success: true }))
                .catch((error) => sendResponse({ success: false, error: error.message }));
            return true; // Keep channel open for async response

        case 'get_dom_snapshot':
            const snapshot = domObserver.getSnapshot();
            sendResponse(snapshot);
            break;

        case 'get_network_requests':
            const requests = networkMonitor.getRecentRequests(
                message.limit || 50,
                message.urlFilter
            );
            sendResponse(requests);
            break;

        case 'start_observing':
            domObserver.start();
            sendResponse({ status: 'started' });
            break;

        case 'stop_observing':
            domObserver.stop();
            sendResponse({ status: 'stopped' });
            break;

        default:
            console.warn('[Zen Agentic] Unknown action:', message.action);
            sendResponse({ error: 'Unknown action' });
    }

    return true; // Will respond asynchronously if needed
});

/**
 * Initialize extension
 */
console.log('[Zen Agentic] Background script initialized');

// Auto-connect to native host on startup
setTimeout(connectToNativeHost, 1000);

// Start network monitoring
networkMonitor.start();

// Clean up on extension unload
browser.runtime.onSuspend.addListener(() => {
    domObserver.stop();
    if (nativePort) {
        nativePort.disconnect();
    }
    console.log('[Zen Agentic] Extension suspended');
});
