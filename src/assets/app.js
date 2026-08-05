// VS Code Web IDE Application Engine

(function () {
  'use strict';

  // State
  const state = {
    config: {
      projects_folder: 'projects',
      lsp: { rs: 2087, js: 2088, ts: 2088, py: 2089, c: 2090, cpp: 2090 }
    },
    workspaces: [],
    activeWorkspace: null,
    treeData: [],
    openTabs: [], // { path, name, content, originalContent, isDirty }
    activeTabPath: null,
    terminalWs: null,
    searchOptions: { caseSensitive: false, wholeWord: false, regex: false },
    settings: {
      editorFontSize: 14,
      terminalFontSize: 13,
      tabSize: 4
    },
    glass: {
      blur: 20,
      opacity: 70
    }
  };

  const $ = (id) => document.getElementById(id);
  const $$ = (sel) => document.querySelectorAll(sel);

  document.addEventListener('DOMContentLoaded', () => {
    initConfig();
    initTitlebar();
    initActivityBar();
    initWorkspaceSelector();
    initExplorerActions();
    initEditorEngine();
    initTerminalEngine();
    initSearchEngine();
    initSettingsEngine();
    initPaneResizers();
    initQuickOpen();
    initContextMenu();

    loadWorkspaces();
    loadSettings();
  });

  // --- 0. Config Loader ---
  async function initConfig() {
    try {
      const res = await fetch('/api/config');
      if (res.ok) {
        state.config = await res.json();
      }
    } catch (err) {
      console.warn('Using fallback config');
    }
  }

  // --- 1. Titlebar & Activity Bar ---
  function initTitlebar() {
    const tbTerm = $('tb-terminal-btn');
    if (tbTerm) tbTerm.addEventListener('click', toggleBottomPanel);

    const tbSet = $('tb-settings-btn');
    if (tbSet) tbSet.addEventListener('click', () => switchSidebarView('settings'));

    const tbSrch = $('tb-search-btn');
    if (tbSrch) tbSrch.addEventListener('click', () => switchSidebarView('search'));
  }

  function initActivityBar() {
    $('act-explorer').addEventListener('click', () => switchSidebarView('explorer'));
    $('act-search').addEventListener('click', () => switchSidebarView('search'));
    $('act-settings').addEventListener('click', () => switchSidebarView('settings'));

    const monitorBtn = $('act-monitor');
    if (monitorBtn) monitorBtn.addEventListener('click', () => switchSidebarView('explorer'));

    const termBottomBtn = $('act-terminal-bottom');
    if (termBottomBtn) termBottomBtn.addEventListener('click', toggleBottomPanel);

    const glassBtn = $('status-glass-btn');
    if (glassBtn) glassBtn.addEventListener('click', () => switchSidebarView('settings'));

    const termPill = $('status-terminal-badge');
    if (termPill) termPill.addEventListener('click', toggleBottomPanel);
  }

  function switchSidebarView(viewName) {
    const btnMap = {
      explorer: 'act-explorer',
      search: 'act-search',
      settings: 'act-settings'
    };

    $$('.activity-btn').forEach((b) => b.classList.remove('active'));
    const actBtn = $(btnMap[viewName] || 'act-explorer');
    if (actBtn) actBtn.classList.add('active');

    $$('.sidebar-view').forEach((v) => v.classList.remove('active'));
    const targetView = $('view-' + viewName);
    if (targetView) targetView.classList.add('active');

    const titleMap = {
      explorer: 'EXPLORER',
      search: 'SEARCH WORKSPACE',
      settings: 'SETTINGS'
    };
    $('sidebar-title').innerText = titleMap[viewName] || 'PANEL';
  }

  // --- 2. Workspace Management ---
  async function loadWorkspaces() {
    try {
      const res = await fetch('/api/workspaces');
      if (!res.ok) return;
      const data = await res.json();
      state.workspaces = data.workspaces || [];

      const select = $('workspace-select');
      select.innerHTML = '<option value="">-- Select Workspace --</option>';

      state.workspaces.forEach((ws) => {
        const opt = document.createElement('option');
        opt.value = ws.path;
        opt.textContent = `${ws.name} (${ws.path})`;
        select.appendChild(opt);
      });

      if (state.workspaces.length > 0) {
        select.value = state.workspaces[0].path;
        selectWorkspace(state.workspaces[0]);
      }
    } catch (err) {
      console.warn('Failed loading workspaces:', err);
    }
  }

  function initWorkspaceSelector() {
    $('workspace-select').addEventListener('change', (e) => {
      const selectedPath = e.target.value;
      const ws = state.workspaces.find((w) => w.path === selectedPath);
      if (ws) selectWorkspace(ws);
    });
  }

  async function selectWorkspace(ws) {
    state.activeWorkspace = ws;
    $('bc-workspace').innerText = ws.name;
    loadDirectoryTree(ws.path);
  }

  async function loadDirectoryTree(path) {
    try {
      const res = await fetch(`/api/workspace/tree?path=${encodeURIComponent(path)}`);
      if (!res.ok) return;
      const nodes = await res.json();
      renderTree(nodes, $('tree-container'));
    } catch (err) {
      console.error('Failed loading tree:', err);
    }
  }

  function renderTree(nodes, container) {
    container.innerHTML = '';
    if (!nodes || nodes.length === 0) {
      container.innerHTML = '<div class="empty-tree-message">Folder is empty.</div>';
      return;
    }

    nodes.forEach((node) => {
      const nodeEl = document.createElement('div');
      nodeEl.className = 'tree-node-wrapper';

      const row = document.createElement('div');
      row.className = 'tree-node';
      row.dataset.path = node.path;
      row.dataset.isDir = node.is_dir;

      const arrow = document.createElement('span');
      arrow.className = 'tree-arrow';
      arrow.textContent = node.is_dir ? '▶' : '';

      const icon = document.createElement('span');
      icon.className = 'tree-icon';
      icon.textContent = node.is_dir ? '📁' : getFileIcon(node.name);

      const label = document.createElement('span');
      label.className = 'tree-label';
      label.textContent = node.name;

      row.appendChild(arrow);
      row.appendChild(icon);
      row.appendChild(label);

      const childrenContainer = document.createElement('div');
      childrenContainer.className = 'tree-children hidden';

      row.addEventListener('click', (e) => {
        e.stopPropagation();
        $$('.tree-node').forEach((n) => n.classList.remove('selected'));
        row.classList.add('selected');

        if (node.is_dir) {
          const isExpanded = !childrenContainer.classList.contains('hidden');
          if (isExpanded) {
            childrenContainer.classList.add('hidden');
            arrow.textContent = '▶';
          } else {
            childrenContainer.classList.remove('hidden');
            arrow.textContent = '▼';
            if (childrenContainer.children.length === 0) {
              loadDirectoryTreeLazy(node.path, childrenContainer);
            }
          }
        } else {
          openFile(node.path, node.name);
        }
      });

      row.addEventListener('contextmenu', (e) => {
        e.preventDefault();
        e.stopPropagation();
        showContextMenu(e.clientX, e.clientY, node.path, node.is_dir);
      });

      nodeEl.appendChild(row);
      if (node.is_dir) nodeEl.appendChild(childrenContainer);
      container.appendChild(nodeEl);
    });
  }

  async function loadDirectoryTreeLazy(path, container) {
    try {
      const res = await fetch(`/api/workspace/tree?path=${encodeURIComponent(path)}`);
      if (!res.ok) return;
      const nodes = await res.json();
      renderTree(nodes, container);
    } catch (err) {
      console.error(err);
    }
  }

  function getFileIcon(name) {
    if (name.endsWith('.rs')) return '🦀';
    if (name.endsWith('.js') || name.endsWith('.ts')) return '📜';
    if (name.endsWith('.html')) return '🌐';
    if (name.endsWith('.css')) return '🎨';
    if (name.endsWith('.json') || name.endsWith('.toml')) return '⚙️';
    if (name.endsWith('.md')) return '📝';
    return '📄';
  }

  // --- 3. Explorer Actions ---
  function initExplorerActions() {
    $('btn-new-file').addEventListener('click', () => createPrompt('file'));
    $('btn-new-folder').addEventListener('click', () => createPrompt('folder'));
    $('btn-refresh-explorer').addEventListener('click', () => {
      if (state.activeWorkspace) loadDirectoryTree(state.activeWorkspace.path);
    });
  }

  async function createPrompt(type) {
    if (!state.activeWorkspace) return alert('Select a workspace first.');
    const name = prompt(`Enter name for new ${type}:`);
    if (!name) return;

    const selectedNode = document.querySelector('.tree-node.selected');
    const parentPath = selectedNode ? selectedNode.dataset.path : state.activeWorkspace.path;

    try {
      const res = await fetch('/api/fs/create', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ parent_path: parentPath, name, is_dir: type === 'folder' })
      });
      if (res.ok) {
        if (state.activeWorkspace) loadDirectoryTree(state.activeWorkspace.path);
      } else {
        alert('Failed to create item.');
      }
    } catch (err) {
      alert('Error creating item: ' + err);
    }
  }

  // --- 4. Synchronized Editor Engine ---
  function initEditorEngine() {
    const textarea = $('code-textarea');
    const overlay = $('code-highlight-overlay');
    const lineNumbers = $('line-numbers');

    textarea.addEventListener('scroll', () => {
      overlay.scrollTop = textarea.scrollTop;
      overlay.scrollLeft = textarea.scrollLeft;
      lineNumbers.scrollTop = textarea.scrollTop;
    });

    textarea.addEventListener('input', () => {
      const tab = state.openTabs.find((t) => t.path === state.activeTabPath);
      if (!tab) return;

      tab.content = textarea.value;
      tab.isDirty = tab.content !== tab.originalContent;
      updateTabUI();
      updateLineNumbers();
      updateSyntaxHighlighting();
      updateStatusCursor();
    });

    textarea.addEventListener('keyup', updateStatusCursor);
    textarea.addEventListener('click', updateStatusCursor);

    // Global Keybindings
    window.addEventListener('keydown', (e) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
        e.preventDefault();
        saveActiveFile();
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'p') {
        e.preventDefault();
        openQuickOpenModal();
      } else if ((e.ctrlKey || e.metaKey) && e.key === '`') {
        e.preventDefault();
        toggleBottomPanel();
      }
    });
  }

  async function openFile(path, name) {
    let tab = state.openTabs.find((t) => t.path === path);
    if (!tab) {
      try {
        const res = await fetch(`/api/fs/file?path=${encodeURIComponent(path)}`);
        if (!res.ok) return alert('Unable to read file.');
        const data = await res.json();
        tab = {
          path,
          name,
          content: data.content,
          originalContent: data.content,
          isDirty: false
        };
        state.openTabs.push(tab);
      } catch (err) {
        return alert('Error reading file: ' + err);
      }
    }

    state.activeTabPath = path;
    renderTabs();
    activateTab(tab);
  }

  function renderTabs() {
    const container = $('tab-list');
    container.innerHTML = '';

    state.openTabs.forEach((tab) => {
      const tabEl = document.createElement('div');
      tabEl.className = 'editor-tab' + (tab.path === state.activeTabPath ? ' active' : '');

      const title = document.createElement('span');
      title.textContent = tab.name;

      if (tab.isDirty) {
        const dirtyDot = document.createElement('span');
        dirtyDot.className = 'tab-dirty-dot';
        tabEl.appendChild(dirtyDot);
      }

      const closeBtn = document.createElement('button');
      closeBtn.className = 'tab-close-btn';
      closeBtn.innerHTML = '✕';
      closeBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        closeTab(tab.path);
      });

      tabEl.appendChild(title);
      tabEl.appendChild(closeBtn);

      tabEl.addEventListener('click', () => {
        state.activeTabPath = tab.path;
        renderTabs();
        activateTab(tab);
      });

      container.appendChild(tabEl);
    });
  }

  function updateTabUI() {
    renderTabs();
  }

  function activateTab(tab) {
    $('welcome-view').classList.add('hidden');
    $('code-surface').classList.remove('hidden');

    const textarea = $('code-textarea');
    textarea.value = tab.content;
    $('bc-filename').innerText = tab.name;

    // Auto-detect LSP Port based on active file extension
    const ext = tab.name.split('.').pop().toLowerCase();
    const lspPort = state.config.lsp[ext] || 2087;
    $('status-lsp-port').innerText = `LSP: ${ext.toUpperCase()} (Port ${lspPort})`;
    $('setting-lsp-target').value = `Port ${lspPort} (${ext.toUpperCase()})`;

    updateLineNumbers();
    updateSyntaxHighlighting();
    updateStatusCursor();
  }

  function closeTab(path) {
    state.openTabs = state.openTabs.filter((t) => t.path !== path);
    if (state.activeTabPath === path) {
      state.activeTabPath = state.openTabs.length > 0 ? state.openTabs[state.openTabs.length - 1].path : null;
    }
    renderTabs();
    if (state.activeTabPath) {
      const tab = state.openTabs.find((t) => t.path === state.activeTabPath);
      if (tab) activateTab(tab);
    } else {
      $('welcome-view').classList.remove('hidden');
      $('code-surface').classList.add('hidden');
      $('bc-filename').innerText = 'select a file';
    }
  }

  async function saveActiveFile() {
    const tab = state.openTabs.find((t) => t.path === state.activeTabPath);
    if (!tab || !tab.isDirty) return;

    try {
      const res = await fetch('/api/fs/file', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ path: tab.path, content: tab.content })
      });
      if (res.ok) {
        tab.originalContent = tab.content;
        tab.isDirty = false;
        updateTabUI();
      } else {
        alert('Failed to save file.');
      }
    } catch (err) {
      alert('Save error: ' + err);
    }
  }

  function updateLineNumbers() {
    const textarea = $('code-textarea');
    const lines = textarea.value.split('\n').length;
    const col = $('line-numbers');
    col.innerHTML = Array.from({ length: lines }, (_, i) => i + 1).join('<br>');
  }

  function updateSyntaxHighlighting() {
    const textarea = $('code-textarea');
    const overlay = $('code-highlight-overlay');
    const minimap = $('minimap-content');

    let text = escapeHtml(textarea.value);
    
    text = text
      .replace(/\b(fn|let|mut|const|struct|enum|pub|use|mod|impl|trait|return|if|else|match|for|while|in|async|await)\b/g, '<span class="token-keyword">$1</span>')
      .replace(/(["'])(?:(?=(\\?))\2[\s\S])*?\1/g, '<span class="token-string">$&</span>')
      .replace(/\/\/.*/g, '<span class="token-comment">$&</span>')
      .replace(/\b([a-zA-Z_]\w*)\s*(?=\()/g, '<span class="token-function">$1</span>')
      .replace(/\b\d+\b/g, '<span class="token-number">$1</span>');

    overlay.innerHTML = text + '\n';
    minimap.innerText = textarea.value;
  }

  function updateStatusCursor() {
    const textarea = $('code-textarea');
    const pos = textarea.selectionStart;
    const textBefore = textarea.value.substring(0, pos);
    const lines = textBefore.split('\n');
    const ln = lines.length;
    const col = lines[lines.length - 1].length + 1;
    $('status-cursor').innerText = `Ln ${ln}, Col ${col}`;
  }

  function escapeHtml(str) {
    return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }

  // --- 5. ANSI Terminal Renderer & Input ---
  function parseAnsiToHtml(str) {
    return str
      .replace(/\x1b\[\?[0-9]+[hL]/g, '')
      .replace(/\x1b\[[0-9;]*[a-zA-Z]/g, '')
      .replace(/\x1b\][^\x07]*\x07/g, '');
  }

  function initTerminalEngine() {
    const screen = $('terminal-screen');
    const wsProtocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${wsProtocol}//${window.location.host}/ws/terminal`;

    try {
      state.terminalWs = new WebSocket(wsUrl);

      state.terminalWs.onopen = () => {
        screen.innerText = '... >\n';
      };

      state.terminalWs.onmessage = (e) => {
        const clean = parseAnsiToHtml(e.data);
        screen.innerText += clean;
        screen.scrollTop = screen.scrollHeight;
      };

      state.terminalWs.onclose = () => {
        screen.innerText += '\n[PTY Shell Closed]\n';
      };
    } catch (err) {
      screen.innerText += '[PTY Error: ' + err + ']\n';
    }

    // Clear terminal button
    const clearBtn = $('btn-clear-terminal');
    if (clearBtn) {
      clearBtn.addEventListener('click', () => {
        screen.innerText = '... >\n';
      });
    }

    // Direct Input Processing
    screen.addEventListener('keydown', (e) => {
      if (!state.terminalWs || state.terminalWs.readyState !== WebSocket.OPEN) return;

      e.preventDefault();
      e.stopPropagation();

      let char = '';
      if (e.key === 'Enter') char = '\r';
      else if (e.key === 'Backspace') char = '\x08';
      else if (e.key === 'Tab') char = '\t';
      else if (e.key === 'Escape') char = '\x1b';
      else if (e.key === 'ArrowUp') char = '\x1b[A';
      else if (e.key === 'ArrowDown') char = '\x1b[B';
      else if (e.key === 'ArrowRight') char = '\x1b[C';
      else if (e.key === 'ArrowLeft') char = '\x1b[D';
      else if (e.ctrlKey) {
        const code = e.key.toUpperCase().charCodeAt(0);
        if (code >= 65 && code <= 90) {
          char = String.fromCharCode(code - 64);
        }
      } else if (e.key.length === 1) {
        char = e.key;
      }

      if (char) {
        state.terminalWs.send(char);
      }
    });

    screen.addEventListener('paste', (e) => {
      if (!state.terminalWs || state.terminalWs.readyState !== WebSocket.OPEN) return;
      e.preventDefault();
      const pasteText = (e.clipboardData || window.clipboardData).getData('text');
      if (pasteText) {
        state.terminalWs.send(pasteText);
      }
    });

    // Panel tabs
    $$('.panel-tab').forEach((tab) => {
      tab.addEventListener('click', () => {
        $$('.panel-tab').forEach((t) => t.classList.remove('active'));
        tab.classList.add('active');

        const contentId = 'pcontent-' + tab.id.replace('ptab-', '');
        $$('.panel-content').forEach((c) => c.classList.remove('active'));
        const contentEl = $(contentId);
        if (contentEl) contentEl.classList.add('active');
      });
    });
  }

  function toggleBottomPanel() {
    $('bottom-panel').classList.toggle('hidden');
  }

  // --- 6. Pane Resizers ---
  function initPaneResizers() {
    const sidebarResizer = $('sidebar-resizer');
    let isResizingSidebar = false;

    sidebarResizer.addEventListener('mousedown', (e) => {
      isResizingSidebar = true;
      document.body.style.cursor = 'col-resize';
    });

    const panelResizer = $('panel-resizer');
    let isResizingPanel = false;

    panelResizer.addEventListener('mousedown', (e) => {
      isResizingPanel = true;
      document.body.style.cursor = 'row-resize';
    });

    window.addEventListener('mousemove', (e) => {
      if (isResizingSidebar) {
        const newWidth = Math.max(180, Math.min(600, e.clientX - 48));
        document.documentElement.style.setProperty('--sidebar-width', newWidth + 'px');
      }
      if (isResizingPanel) {
        const newHeight = Math.max(100, Math.min(600, window.innerHeight - e.clientY - 24));
        document.documentElement.style.setProperty('--panel-height', newHeight + 'px');
      }
    });

    window.addEventListener('mouseup', () => {
      if (isResizingSidebar || isResizingPanel) {
        isResizingSidebar = false;
        isResizingPanel = false;
        document.body.style.cursor = 'default';
      }
    });
  }

  // --- 7. Search Engine ---
  function initSearchEngine() {
    $('search-opt-case').addEventListener('click', (e) => {
      state.searchOptions.caseSensitive = !state.searchOptions.caseSensitive;
      e.target.classList.toggle('active', state.searchOptions.caseSensitive);
    });
    $('search-opt-word').addEventListener('click', (e) => {
      state.searchOptions.wholeWord = !state.searchOptions.wholeWord;
      e.target.classList.toggle('active', state.searchOptions.wholeWord);
    });
    $('search-opt-regex').addEventListener('click', (e) => {
      state.searchOptions.regex = !state.searchOptions.regex;
      e.target.classList.toggle('active', state.searchOptions.regex);
    });

    $('btn-trigger-search').addEventListener('click', runWorkspaceSearch);
  }

  async function runWorkspaceSearch() {
    const query = $('search-query').value;
    if (!query || !state.activeWorkspace) return;

    try {
      const res = await fetch('/api/search', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          workspace: state.activeWorkspace.path,
          query,
          case_sensitive: state.searchOptions.caseSensitive,
          whole_word: state.searchOptions.wholeWord,
          is_regex: state.searchOptions.regex
        })
      });
      const data = await res.json();
      renderSearchResults(data.results || []);
    } catch (err) {
      alert('Search error: ' + err);
    }
  }

  function renderSearchResults(results) {
    const container = $('search-tree');
    container.innerHTML = '';
    $('search-summary').innerText = `${results.length} matches found`;

    results.forEach((item) => {
      const matchEl = document.createElement('div');
      matchEl.className = 'search-match-item';
      matchEl.innerHTML = `<strong>${escapeHtml(item.file)}</strong>:${item.line}: ${escapeHtml(item.content)}`;
      matchEl.addEventListener('click', () => openFile(item.path, item.file));
      container.appendChild(matchEl);
    });
  }

  // --- 8. Settings Engine ---
  function initSettingsEngine() {
    const updateGlass = () => {
      const blur = $('glass-blur').value;
      const opacity = $('glass-opacity').value;

      $('val-blur').innerText = blur;
      $('val-opacity').innerText = opacity;

      const root = document.documentElement;
      root.style.setProperty('--glass-blur', blur + 'px');
      root.style.setProperty('--glass-opacity', opacity / 100);
    };

    ['glass-blur', 'glass-opacity'].forEach((id) => {
      const el = $(id);
      if (el) el.addEventListener('input', updateGlass);
    });

    const syncSettings = () => {
      state.settings.editorFontSize = parseInt($('setting-editor-font-size').value, 10);
      state.settings.terminalFontSize = parseInt($('setting-terminal-font-size').value, 10);
      state.settings.tabSize = parseInt($('setting-tab-size').value, 10);

      document.documentElement.style.setProperty('--editor-font-size', state.settings.editorFontSize + 'px');
      $('terminal-screen').style.fontSize = state.settings.terminalFontSize + 'px';
    };

    ['setting-editor-font-size', 'setting-terminal-font-size', 'setting-tab-size'].forEach((id) => {
      const el = $(id);
      if (el) el.addEventListener('change', syncSettings);
    });
  }

  async function loadSettings() {
    try {
      const res = await fetch('/api/settings');
      if (!res.ok) return;
      const data = await res.json();
      state.settings = { ...state.settings, ...data };
    } catch (err) {
      console.warn('Using default settings');
    }
  }

  // --- 9. Quick Open Modal ---
  function initQuickOpen() {
    const modal = $('quick-open-modal');
    const input = $('quick-open-input');

    modal.addEventListener('click', (e) => {
      if (e.target === modal) modal.classList.add('hidden');
    });

    input.addEventListener('input', async () => {
      const query = input.value;
      if (!query || !state.activeWorkspace) return;

      try {
        const res = await fetch(`/api/search/quickopen?workspace=${encodeURIComponent(state.activeWorkspace.path)}&q=${encodeURIComponent(query)}`);
        const data = await res.json();
        renderQuickOpenResults(data.files || []);
      } catch (err) {
        console.error(err);
      }
    });
  }

  function openQuickOpenModal() {
    const modal = $('quick-open-modal');
    modal.classList.remove('hidden');
    $('quick-open-input').value = '';
    $('quick-open-input').focus();
    $('quick-open-results').innerHTML = '';
  }

  function renderQuickOpenResults(files) {
    const container = $('quick-open-results');
    container.innerHTML = '';

    files.forEach((f) => {
      const row = document.createElement('div');
      row.className = 'search-match-item';
      row.innerHTML = `📄 <strong>${escapeHtml(f.name)}</strong> <small style="color:#888888;">(${escapeHtml(f.path)})</small>`;
      row.addEventListener('click', () => {
        openFile(f.path, f.name);
        $('quick-open-modal').classList.add('hidden');
      });
      container.appendChild(row);
    });
  }

  // --- 10. Context Menu ---
  function initContextMenu() {
    const cm = $('context-menu');
    window.addEventListener('click', () => cm.classList.add('hidden'));

    $('cm-new-file').addEventListener('click', () => createPrompt('file'));
    $('cm-new-folder').addEventListener('click', () => createPrompt('folder'));
  }

  function showContextMenu(x, y, path, isDir) {
    const cm = $('context-menu');
    cm.style.left = x + 'px';
    cm.style.top = y + 'px';
    cm.classList.remove('hidden');
  }

})();
