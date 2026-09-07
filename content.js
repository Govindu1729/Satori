/**
 * Content script for Zen Agentic AI Extension
 *
 * Injected into web pages to:
 * - Extract page content for NLP processing
 * - Generate DOM snapshots with UIDs
 * - Listen for commands from background script
 */

console.log('[Zen Agentic Content] Loaded on:', window.location.href);

// Generate unique IDs for DOM elements
function generateElementUID(element) {
    const path = [];
    let current = element;

    while (current && current.nodeType === Node.ELEMENT_NODE) {
        let selector = current.nodeName.toLowerCase();

        if (current.id) {
            selector += `#${current.id}`;
            path.unshift(selector);
            break;
        } else {
            let sibling = current;
            let nth = 1;

            while (sibling.previousElementSibling) {
                sibling = sibling.previousElementSibling;
                if (sibling.nodeName === current.nodeName) nth++;
            }

            if (nth > 1 || current.nextElementSibling) {
                selector += `:nth-of-type(${nth})`;
            }
        }

        path.unshift(selector);
        current = current.parentNode;
    }

    return path.join(' > ');
}

// Extract visible text content from page
function extractPageText() {
    // Remove script, style, and hidden elements
    const clone = document.body.cloneNode(true);
    const removeSelectors = ['script', 'style', 'noscript', '[hidden]', '[aria-hidden="true"]'];

    removeSelectors.forEach(selector => {
        clone.querySelectorAll(selector).forEach(el => el.remove());
    });

    return clone.textContent.replace(/\s+/g, ' ').trim();
}

// Get interactive elements with UIDs
function getInteractiveElements() {
    const selectors = 'a, button, input, select, textarea, [role="button"], [onclick], [tabindex]:not([tabindex="-1"])';
    const elements = document.querySelectorAll(selectors);

    return Array.from(elements).map(el => ({
        uid: generateElementUID(el),
        tagName: el.tagName.toLowerCase(),
        id: el.id || null,
        className: el.className || null,
        text: el.textContent?.trim().substring(0, 100) || null,
        type: el.type || null,
        visible: isElementVisible(el)
    }));
}

// Check if element is visible
function isElementVisible(el) {
    const style = window.getComputedStyle(el);
    return style.display !== 'none' &&
           style.visibility !== 'hidden' &&
           style.opacity !== '0' &&
           el.offsetWidth > 0 &&
           el.offsetHeight > 0;
}

// Create DOM snapshot
function createDOMSnapshot(maxDepth = 10) {
    return {
        url: window.location.href,
        title: document.title,
        timestamp: Date.now(),
        text: extractPageText(),
        interactiveElements: getInteractiveElements(),
        elementCount: document.getElementsByTagName('*').length
    };
}

// Listen for messages from background script
browser.runtime.onMessage.addListener((message, sender, sendResponse) => {
    console.log('[Zen Agentic Content] Received:', message);

    switch (message.action) {
        case 'get_content':
            sendResponse({
                text: extractPageText(),
                url: window.location.href,
                title: document.title
            });
            break;

        case 'get_snapshot':
            sendResponse(createDOMSnapshot(message.maxDepth));
            break;

        case 'get_interactive_elements':
            sendResponse(getInteractiveElements());
            break;

        default:
            sendResponse({ error: 'Unknown action' });
    }

    return true;
});

// Notify background script that content script is ready
browser.runtime.sendMessage({
    action: 'content_script_loaded',
    url: window.location.href
}).catch(() => {
    // Background script might not be ready yet
    console.log('[Zen Agentic Content] Will retry connection');
});
