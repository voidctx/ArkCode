// SPDX-License-Identifier: AGPL-3.0-only
//
// Copyright (C) 2026 xpert0
//
// This file is part of ArkCode.

(function () {
  'use strict';

  const state = {
    openFiles: [],
    activeFilePath: null,
    selectedDirPath: '',
    selectedNodePath: '',
    expandedFolderPaths: new Set(),
    treeData: null,
    stderrCount: 0,
    termInputBuffer: '',
    currentProjectConfig: null,
    currentWorkspaceName: '',
    lspCompletions: []
  };

  const els = {
    fileTreeRoot: document.getElementById('file-tree-root'),
    tabsContainer: document.getElementById('tabs-container'),
    codeTextarea: document.getElementById('code-textarea'),
    syntaxOverlay: document.getElementById('syntax-highlight-overlay'),
    gutter: document.getElementById('gutter'),
    emptyState: document.getElementById('editor-empty-state'),
    breadcrumbPath: document.getElementById('breadcrumb-path'),
    windowTitle: document.getElementById('window-title'),
    statusPos: document.getElementById('status-pos'),
    statusLspInfo: document.getElementById('status-lsp-info'),
    sidebarPanels: document.querySelectorAll('.sidebar-panel'),
    activityBtns: document.querySelectorAll('.activity-btn'),
    panelTabs: document.querySelectorAll('.panel-tab-btn'),
    panelViews: document.querySelectorAll('.panel-view'),
    searchQuery: document.getElementById('search-query'),
    btnDoSearch: document.getElementById('btn-do-search'),
    searchResults: document.getElementById('search-results'),
    btnNewFile: document.getElementById('btn-new-file'),
    btnNewFolder: document.getElementById('btn-new-folder'),
    btnRefreshTree: document.getElementById('btn-refresh-tree'),
    workspaceItemsList: document.getElementById('workspace-items-list'),
    btnCreateWorkspacePanel: document.getElementById('btn-create-workspace-panel'),
    workspaceContextMenu: document.getElementById('workspace-context-menu'),
    sidebar: document.getElementById('sidebar'),
    resizerSidebar: document.getElementById('resizer-sidebar'),
    bottomPanel: document.getElementById('bottom-panel'),
    resizerPanel: document.getElementById('resizer-panel'),
    vscodeTerminal: document.getElementById('vscode-terminal'),
    termLog: document.getElementById('term-log'),
    termPromptStr: document.getElementById('term-prompt-str'),
    termInputText: document.getElementById('term-input-text'),
    stderrBuffer: document.getElementById('stderr-buffer'),
    stderrBadge: document.getElementById('stderr-badge'),
    buildBuffer: document.getElementById('build-stream-buffer'),
    btnRunBuildActivity: document.getElementById('btn-run-build-activity'),
    btnClearOutput: document.getElementById('btn-clear-output'),
    lspPopup: document.getElementById('lsp-autocomplete-popup'),
    createProjectBackdrop: document.getElementById('create-project-backdrop'),
    btnCloseCreateProject: document.getElementById('btn-close-create-project'),
    cfgProjName: document.getElementById('cfg-proj-name'),
    cfgProjBuild: document.getElementById('cfg-proj-build'),
    cfgProjLsType: document.getElementById('cfg-proj-ls-type'),
    remoteUrlContainer: document.getElementById('remote-url-container'),
    cfgProjLsUrl: document.getElementById('cfg-proj-ls-url'),
    btnSubmitCreateProject: document.getElementById('btn-submit-create-project'),
    contextMenu: document.getElementById('glass-context-menu'),
    menubarDropdown: document.getElementById('glass-menubar-dropdown'),
    settingsBackdrop: document.getElementById('settings-backdrop'),
    btnCloseSettings: document.getElementById('btn-close-settings'),
    cfgGlassOpacity: document.getElementById('cfg-glass-opacity'),
    cfgGlassBlur: document.getElementById('cfg-glass-blur'),
    cfgFontSize: document.getElementById('cfg-font-size'),
    valGlassOpacity: document.getElementById('val-glass-opacity'),
    valGlassBlur: document.getElementById('val-glass-blur'),
    valFontSize: document.getElementById('val-font-size')
  };

  let ws = null;

  window.addEventListener('DOMContentLoaded', () => {
    loadPersistedSettings();
    initWebSocket();
    initResizablePanes();
    initVSCodeTerminal();
    initExplorerInlineCreation();
    initCreateProjectModal();
    initLspAutoComplete();
    initGlobalEvents();
    initMenuActions();
    initWorkspacesManager();
    initSearchEngine();
    initLiveSettings();
  });

  function applyInitialSettings() {
    try {
      const saved = localStorage.getItem('webide_settings');
      if (saved) {
        const cfg = JSON.parse(saved);
        if (cfg.glassOpacity !== undefined) {
          document.documentElement.style.setProperty('--glass-opacity', (cfg.glassOpacity / 100).toFixed(2));
        }
        if (cfg.glassBlur !== undefined) {
          document.documentElement.style.setProperty('--glass-blur', `${cfg.glassBlur}px`);
        }
        if (cfg.fontSize !== undefined) {
          document.documentElement.style.setProperty('--font-size-code', `${cfg.fontSize}px`);
        }
      }
    } catch (e) { }
  }
  applyInitialSettings();

  function loadPersistedSettings() {
    try {
      const saved = localStorage.getItem('webide_settings');
      if (saved) {
        const cfg = JSON.parse(saved);
        if (cfg.glassOpacity !== undefined) {
          if (els.cfgGlassOpacity) els.cfgGlassOpacity.value = cfg.glassOpacity;
          document.documentElement.style.setProperty('--glass-opacity', (cfg.glassOpacity / 100).toFixed(2));
          if (els.valGlassOpacity) els.valGlassOpacity.textContent = `${cfg.glassOpacity}%`;
        }
        if (cfg.glassBlur !== undefined) {
          if (els.cfgGlassBlur) els.cfgGlassBlur.value = cfg.glassBlur;
          document.documentElement.style.setProperty('--glass-blur', `${cfg.glassBlur}px`);
          if (els.valGlassBlur) els.valGlassBlur.textContent = `${cfg.glassBlur}px`;
        }
        if (cfg.fontSize !== undefined) {
          if (els.cfgFontSize) els.cfgFontSize.value = cfg.fontSize;
          document.documentElement.style.setProperty('--font-size-code', `${cfg.fontSize}px`);
          if (els.valFontSize) els.valFontSize.textContent = `${cfg.fontSize}px`;
        }
      }
    } catch (e) {
      console.warn('Could not load saved settings from localStorage', e);
    }
  }

  function savePersistedSettings() {
    try {
      const cfg = {
        glassOpacity: parseInt(els.cfgGlassOpacity.value, 10),
        glassBlur: parseInt(els.cfgGlassBlur.value, 10),
        fontSize: parseInt(els.cfgFontSize.value, 10)
      };
      localStorage.setItem('webide_settings', JSON.stringify(cfg));
    } catch (e) {
      console.warn('Could not save settings to localStorage', e);
    }
  }

  function initWebSocket() {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.host}`;

    ws = new WebSocket(wsUrl);

    ws.onopen = () => {
      console.log('WebSocket connected to Modular Rust Web IDE backend');
      wsSend({ type: 'workspace:list' });
      wsSend({ type: 'fs:list', payload: { path: '' } });
      wsSend({ type: 'terminal:init' });
    };

    ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data);
        handleServerMessage(msg);
      } catch (e) {
        console.error('Failed to parse WebSocket message:', event.data);
      }
    };

    ws.onclose = () => {
      setTimeout(initWebSocket, 2000);
    };
  }

  function wsSend(msg) {
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify(msg));
    }
  }

  function handleServerMessage(msg) {
    const { type, tree, content, path, workspaces, current, results, cmd, stdout, stderr, banner, prompt, config, suggestions } = msg;

    switch (type) {
      case 'fs:list:response':
        if (tree) {
          state.treeData = tree;
          renderTreeRoot(tree);
        }
        break;

      case 'fs:read:response':
        if (path && content !== undefined) {
          openFileTab(path, content);
        }
        break;

      case 'fs:write:response':
        wsSend({ type: 'fs:list', payload: { path: '' } });
        break;

      case 'fs:search:response':
        renderSearchResults(results || []);
        break;

      case 'workspace:list:response':
        if (workspaces) {
          state.currentWorkspaceName = current;
          renderWorkspacesList(workspaces, current);
        }
        break;

      case 'workspace:create:response':
        if (config) {
          state.currentProjectConfig = config;
        }
        state.expandedFolderPaths.clear();
        updateLspStatusForActiveFile();
        state.selectedDirPath = '';
        state.selectedNodePath = '';
        wsSend({ type: 'workspace:list' });
        wsSend({ type: 'fs:list', payload: { path: '' } });
        wsSend({ type: 'terminal:init' });
        activateSidebarPanel('panel-explorer');
        break;

      case 'workspace:switch:response':
        if (config) {
          state.currentProjectConfig = config;
        }
        state.openFiles = [];
        state.activeFilePath = null;
        renderTabs();
        updateEditorContent();
        state.expandedFolderPaths.clear();
        updateLspStatusForActiveFile();
        state.selectedDirPath = '';
        state.selectedNodePath = '';
        wsSend({ type: 'workspace:list' });
        wsSend({ type: 'fs:list', payload: { path: '' } });
        wsSend({ type: 'terminal:init' });
        activateSidebarPanel('panel-explorer');
        break;

      case 'workspace:delete:response':
        wsSend({ type: 'workspace:list' });
        break;

      case 'terminal:init:response':
        if (banner && els.termLog.textContent === '') {
          els.termLog.textContent = banner;
        }
        if (prompt) {
          els.termPromptStr.textContent = prompt;
        }
        break;

      case 'terminal:data':
        const curPrompt = els.termPromptStr.textContent;
        els.termLog.textContent += `${curPrompt} ${cmd}\n`;
        if (stdout) {
          els.termLog.textContent += stdout;
        }
        if (stderr) {
          appendStderr(stderr);
        }
        if (prompt) {
          els.termPromptStr.textContent = prompt;
        }
        state.termInputBuffer = '';
        els.termInputText.textContent = '';
        els.vscodeTerminal.scrollTop = els.vscodeTerminal.scrollHeight;
        wsSend({ type: 'fs:list', payload: { path: '' } });
        break;

      case 'lsp:completion:response':
        if (suggestions && suggestions.length > 0) {
          renderLspCompletions(suggestions);
        } else {
          els.lspPopup.classList.add('hidden');
        }
        break;

      case 'build:response':
      case 'build:stream':
        if (els.buildBuffer) {
          if (els.buildBuffer.textContent.includes('[Building workspace...]') || els.buildBuffer.textContent.includes('[Build Console ready]')) {
            els.buildBuffer.textContent = '';
          }
          if (msg.cmd) {
            els.buildBuffer.textContent += `[Build Command: ${msg.cmd}]\n`;
          }
          if (msg.stdout) els.buildBuffer.textContent += msg.stdout;
          if (msg.stderr) els.buildBuffer.textContent += msg.stderr;
          els.buildBuffer.scrollTop = els.buildBuffer.scrollHeight;
        }
        break;
    }
  }

  function initWorkspacesManager() {
    els.btnCreateWorkspacePanel.addEventListener('click', () => {
      els.createProjectBackdrop.classList.remove('hidden');
    });

    els.workspaceContextMenu.querySelectorAll('.menu-option').forEach(opt => {
      opt.addEventListener('click', (e) => {
        e.stopPropagation();
        els.workspaceContextMenu.classList.add('hidden');
        const action = opt.dataset.action;
        const targetWorkspace = els.workspaceContextMenu.dataset.targetWorkspace;

        if (action === 'delete-workspace' && targetWorkspace) {
          const confirm1 = confirm(`Are you sure you want to delete workspace '${targetWorkspace}'?`);
          if (confirm1) {
            const confirm2 = confirm(`CRITICAL WARNING: All files inside '${targetWorkspace}' will be permanently lost! Proceed with deletion?`);
            if (confirm2) {
              wsSend({ type: 'workspace:delete', payload: { name: targetWorkspace } });
            }
          }
        }
      });
    });
  }

  function renderWorkspacesList(workspaces, current) {
    els.workspaceItemsList.innerHTML = '';
    workspaces.forEach(wsName => {
      const item = document.createElement('div');
      item.className = `workspace-item ${wsName === current ? 'active' : ''}`;
      item.innerHTML = `
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z"/></svg>
        <span>${wsName}</span>
      `;

      item.addEventListener('click', () => {
        if (wsName !== state.currentWorkspaceName) {
          wsSend({ type: 'workspace:switch', payload: { name: wsName } });
        }
      });

      item.addEventListener('contextmenu', (e) => {
        e.preventDefault();
        e.stopPropagation();
        showWorkspaceContextMenu(e.clientX, e.clientY, wsName);
      });

      els.workspaceItemsList.appendChild(item);
    });
  }

  function showWorkspaceContextMenu(x, y, targetWorkspace) {
    els.workspaceContextMenu.dataset.targetWorkspace = targetWorkspace;
    els.workspaceContextMenu.style.left = `${Math.min(x, window.innerWidth - 240)}px`;
    els.workspaceContextMenu.style.top = `${Math.min(y, window.innerHeight - 200)}px`;
    els.workspaceContextMenu.classList.remove('hidden');
  }

  function updateLspStatusForActiveFile() {
    const cfg = state.currentProjectConfig;

    if (cfg && (cfg.ls_type === false || cfg.ls_type === 'false' || cfg.ls_type === 'NONE')) {
      els.statusLspInfo.textContent = 'LSP: Disabled';
      return;
    }

    if (!state.activeFilePath) {
      els.statusLspInfo.textContent = 'LSP: Ready (No file open)';
      return;
    }

    const ext = state.activeFilePath.split('.').pop().toLowerCase();
    const lspMap = {
      'rs': { name: 'Rust', port: 2087 },
      'js': { name: 'JavaScript', port: 2088 },
      'jsx': { name: 'JavaScript', port: 2088 },
      'ts': { name: 'TypeScript', port: 2088 },
      'tsx': { name: 'TypeScript', port: 2088 },
      'py': { name: 'Python', port: 2089 },
      'c': { name: 'C', port: 2090 },
      'h': { name: 'C', port: 2090 },
      'cpp': { name: 'C++', port: 2090 },
      'hpp': { name: 'C++', port: 2090 },
      'cc': { name: 'C++', port: 2090 },
      'cxx': { name: 'C++', port: 2090 },
      'html': { name: 'HTML', port: 2091 },
      'htm': { name: 'HTML', port: 2091 },
      'css': { name: 'CSS', port: 2091 },
      'scss': { name: 'CSS', port: 2091 }
    };

    const lspInfo = lspMap[ext];
    if (lspInfo) {
      els.statusLspInfo.textContent = `LSP: Auto (${lspInfo.name} - Port ${lspInfo.port})`;
    } else {
      els.statusLspInfo.textContent = `LSP: Auto (.${ext})`;
    }
  }

  function appendStderr(text) {
    if (els.stderrBuffer.textContent.includes('No process errors recorded')) {
      els.stderrBuffer.textContent = '';
    }
    els.stderrBuffer.textContent += text + '\n';
    state.stderrCount++;
    els.stderrBadge.textContent = state.stderrCount;
    els.stderrBadge.style.display = 'inline-block';
  }

  function renderTreeRoot(rootNode) {
    els.fileTreeRoot.innerHTML = '';
    if (!rootNode.children || rootNode.children.length === 0) {
      els.fileTreeRoot.innerHTML = '<div style="padding:12px;color:#858585;font-size:12px;">Workspace empty. Use Explorer buttons to create files/folders.</div>';
      return;
    }

    renderTreeNodes(rootNode.children, els.fileTreeRoot, 0);
  }

  function renderTreeNodes(children, containerElement, level) {
    children.forEach(item => {
      const itemWrapper = document.createElement('div');
      itemWrapper.className = 'tree-item-wrapper';

      const isExpanded = state.expandedFolderPaths.has(item.path);

      const nodeEl = document.createElement('div');
      nodeEl.className = `tree-node ${item.isDir ? 'folder' : 'file'} ${item.path === state.selectedNodePath ? 'selected' : ''}`;
      nodeEl.dataset.path = item.path;
      nodeEl.dataset.isDir = item.isDir;
      nodeEl.style.paddingLeft = `${level * 14 + 8}px`;

      const arrowHtml = item.isDir
        ? `<span class="tree-arrow ${isExpanded ? 'expanded' : ''}">›</span>`
        : `<span class="tree-arrow-spacer"></span>`;

      const iconSvg = item.isDir
        ? `<svg class="node-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z"/></svg>`
        : `<svg class="node-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>`;

      nodeEl.innerHTML = `${arrowHtml}${iconSvg}<span>${item.name}</span>`;

      itemWrapper.appendChild(nodeEl);

      let childrenContainer = null;
      if (item.isDir && item.children && item.children.length > 0) {
        childrenContainer = document.createElement('div');
        childrenContainer.className = `tree-children ${isExpanded ? '' : 'collapsed'}`;
        renderTreeNodes(item.children, childrenContainer, level + 1);
        itemWrapper.appendChild(childrenContainer);
      }

      nodeEl.addEventListener('click', (e) => {
        e.stopPropagation();
        document.querySelectorAll('.tree-node').forEach(el => el.classList.remove('selected'));
        nodeEl.classList.add('selected');
        state.selectedNodePath = item.path;

        if (item.isDir) {
          state.selectedDirPath = item.path;
          const arrow = nodeEl.querySelector('.tree-arrow');
          if (state.expandedFolderPaths.has(item.path)) {
            state.expandedFolderPaths.delete(item.path);
            if (childrenContainer) childrenContainer.classList.add('collapsed');
            if (arrow) arrow.classList.remove('expanded');
          } else {
            state.expandedFolderPaths.add(item.path);
            if (childrenContainer) childrenContainer.classList.remove('collapsed');
            if (arrow) arrow.classList.add('expanded');
          }
        } else {
          const parts = item.path.split(/[/\\]/);
          parts.pop();
          state.selectedDirPath = parts.join('/');
          wsSend({ type: 'fs:read', payload: { path: item.path } });
        }
      });

      nodeEl.addEventListener('contextmenu', (e) => {
        e.preventDefault();
        e.stopPropagation();
        document.querySelectorAll('.tree-node').forEach(el => el.classList.remove('selected'));
        nodeEl.classList.add('selected');
        state.selectedNodePath = item.path;
        if (item.isDir) {
          state.selectedDirPath = item.path;
        } else {
          const parts = item.path.split(/[/\\]/);
          parts.pop();
          state.selectedDirPath = parts.join('/');
        }
        showContextMenu(e.clientX, e.clientY, item.path, item.isDir);
      });

      containerElement.appendChild(itemWrapper);
    });
  }

  function initExplorerInlineCreation() {
    els.btnNewFile.addEventListener('click', () => createInlineItem(false));
    els.btnNewFolder.addEventListener('click', () => createInlineItem(true));
    els.btnRefreshTree.addEventListener('click', () => wsSend({ type: 'fs:list', payload: { path: '' } }));
  }

  function createInlineItem(isDir) {
    const existing = document.querySelector('.tree-inline-row');
    if (existing) existing.remove();

    const parentDir = state.selectedDirPath;
    const targetContainer = els.fileTreeRoot;

    const row = document.createElement('div');
    row.className = 'tree-inline-row';

    const iconSvg = isDir
      ? `<svg class="node-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="color:#e8a838;width:16px;height:16px;"><path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z"/></svg>`
      : `<svg class="node-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="color:var(--fg-blue);width:16px;height:16px;"><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>`;

    const placeholderText = parentDir ? `${parentDir}/${isDir ? 'folder' : 'file.ext'}` : (isDir ? 'directory_name' : 'filename.ext');

    row.innerHTML = `
      ${iconSvg}
      <input type="text" class="tree-inline-input" placeholder="${placeholderText}" spellcheck="false" autocomplete="off">
    `;

    targetContainer.prepend(row);
    const input = row.querySelector('.tree-inline-input');
    input.focus();

    const commitCreation = () => {
      const val = input.value.trim();
      if (val) {
        const fullPath = parentDir ? `${parentDir}/${val}` : val;
        if (isDir) {
          wsSend({ type: 'fs:createDir', payload: { path: fullPath } });
        } else {
          wsSend({ type: 'fs:write', payload: { path: fullPath, content: '' } });
          setTimeout(() => wsSend({ type: 'fs:read', payload: { path: fullPath } }), 100);
        }
      }
      row.remove();
    };

    input.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        commitCreation();
      } else if (e.key === 'Escape') {
        row.remove();
      }
    });

    input.addEventListener('blur', () => {
      setTimeout(() => { if (document.body.contains(row)) row.remove(); }, 150);
    });
  }

  function initCreateProjectModal() {
    const closeModal = () => els.createProjectBackdrop.classList.add('hidden');
    els.btnCloseCreateProject.addEventListener('click', closeModal);

    els.cfgProjLsType.addEventListener('change', () => {
      if (els.cfgProjLsType.value === 'REMOTE') {
        els.remoteUrlContainer.classList.remove('hidden');
      } else {
        els.remoteUrlContainer.classList.add('hidden');
      }
    });

    els.btnSubmitCreateProject.addEventListener('click', () => {
      const name = els.cfgProjName.value.trim();
      const build = els.cfgProjBuild.value.trim();
      const lsType = els.cfgProjLsType.value === 'true';

      wsSend({
        type: 'workspace:create',
        payload: { name, build, lsType }
      });

      closeModal();
      els.cfgProjName.value = '';
      els.cfgProjBuild.value = '';
    });
  }

  function initLspAutoComplete() {
    els.codeTextarea.addEventListener('input', () => {
      const text = els.codeTextarea.value;
      const pos = els.codeTextarea.selectionStart;
      const left = text.substring(0, pos);
      const match = left.match(/([a-zA-Z_]\w*)$/);

      if (match && match[1].length >= 3) {
        const word = match[1];
        const ext = state.activeFilePath ? state.activeFilePath.split('.').pop().toLowerCase() : 'rust';
        wsSend({ type: 'lsp:completion', payload: { language: ext, word } });
      } else {
        els.lspPopup.classList.add('hidden');
      }
    });

    els.codeTextarea.addEventListener('keydown', (e) => {
      if (!els.lspPopup.classList.contains('hidden')) {
        const items = Array.from(els.lspPopup.querySelectorAll('.lsp-item'));
        if (items.length > 0) {
          let activeIdx = items.findIndex(item => item.classList.contains('active'));
          if (e.key === 'ArrowDown') {
            e.preventDefault();
            if (activeIdx >= 0) items[activeIdx].classList.remove('active');
            activeIdx = (activeIdx + 1) % items.length;
            items[activeIdx].classList.add('active');
            items[activeIdx].scrollIntoView({ block: 'nearest' });
            return;
          } else if (e.key === 'ArrowUp') {
            e.preventDefault();
            if (activeIdx >= 0) items[activeIdx].classList.remove('active');
            activeIdx = (activeIdx - 1 + items.length) % items.length;
            items[activeIdx].classList.add('active');
            items[activeIdx].scrollIntoView({ block: 'nearest' });
            return;
          } else if (e.key === 'Tab' || e.key === 'Enter') {
            const activeItem = items[activeIdx >= 0 ? activeIdx : 0];
            if (activeItem) {
              e.preventDefault();
              insertLspSuggestion(activeItem.dataset.label);
              return;
            }
          } else if (e.key === 'Escape') {
            els.lspPopup.classList.add('hidden');
            return;
          }
        }
      }

      const text = els.codeTextarea.value;
      const pos = els.codeTextarea.selectionStart;

      if (e.key === 'Tab') {
        e.preventDefault();
        els.codeTextarea.value = text.substring(0, pos) + '  ' + text.substring(els.codeTextarea.selectionEnd);
        els.codeTextarea.selectionStart = els.codeTextarea.selectionEnd = pos + 2;
        renderGutterAndSyntax();
        updateCursorPos();
        return;
      }

      if (e.key === 'Enter') {
        e.preventDefault();
        const lines = text.substring(0, pos).split('\n');
        const curLine = lines[lines.length - 1];
        const indentMatch = curLine.match(/^(\s*)/);
        const curIndent = indentMatch ? indentMatch[1] : '';

        const trimmedCurLine = curLine.trim();
        const charAfter = text.substring(pos, pos + 1);

        if (trimmedCurLine.endsWith('{') || trimmedCurLine.endsWith('(') || trimmedCurLine.endsWith('[')) {
          const newIndent = curIndent + '  ';
          const isClosingNext = (trimmedCurLine.endsWith('{') && charAfter === '}') ||
            (trimmedCurLine.endsWith('(') && charAfter === ')') ||
            (trimmedCurLine.endsWith('[') && charAfter === ']');

          if (isClosingNext) {
            const insertion = '\n' + newIndent + '\n' + curIndent;
            els.codeTextarea.value = text.substring(0, pos) + insertion + text.substring(pos);
            const targetPos = pos + 1 + newIndent.length;
            els.codeTextarea.selectionStart = els.codeTextarea.selectionEnd = targetPos;
          } else {
            const insertion = '\n' + newIndent;
            els.codeTextarea.value = text.substring(0, pos) + insertion + text.substring(pos);
            const targetPos = pos + insertion.length;
            els.codeTextarea.selectionStart = els.codeTextarea.selectionEnd = targetPos;
          }
        } else {
          const insertion = '\n' + curIndent;
          els.codeTextarea.value = text.substring(0, pos) + insertion + text.substring(pos);
          const targetPos = pos + insertion.length;
          els.codeTextarea.selectionStart = els.codeTextarea.selectionEnd = targetPos;
        }

        renderGutterAndSyntax();
        updateCursorPos();
        return;
      }

      const pairs = {
        '(': ')',
        '{': '}',
        '[': ']',
        '<': '>',
        '"': '"',
        "'": "'",
        '`': '`'
      };

      const closingChars = new Set([')', '}', ']', '>', '"', "'", '`']);

      if (e.key === 'Backspace' && pos > 0 && pos === els.codeTextarea.selectionEnd) {
        const leftChar = text.substring(pos - 1, pos);
        const rightChar = text.substring(pos, pos + 1);
        if (pairs[leftChar] && pairs[leftChar] === rightChar) {
          e.preventDefault();
          els.codeTextarea.value = text.substring(0, pos - 1) + text.substring(pos + 1);
          els.codeTextarea.selectionStart = els.codeTextarea.selectionEnd = pos - 1;
          renderGutterAndSyntax();
          updateCursorPos();
          return;
        }
      }

      if (closingChars.has(e.key) && text.substring(pos, pos + 1) === e.key) {
        e.preventDefault();
        els.codeTextarea.selectionStart = els.codeTextarea.selectionEnd = pos + 1;
        updateCursorPos();
        return;
      }

      if (pairs[e.key]) {
        e.preventDefault();
        const closePair = pairs[e.key];
        const selectionEnd = els.codeTextarea.selectionEnd;
        const selectedText = text.substring(pos, selectionEnd);
        els.codeTextarea.value = text.substring(0, pos) + e.key + selectedText + closePair + text.substring(selectionEnd);
        els.codeTextarea.selectionStart = els.codeTextarea.selectionEnd = pos + 1;
        renderGutterAndSyntax();
        updateCursorPos();
        return;
      }
    });
  }

  function renderLspCompletions(suggestions) {
    state.lspCompletions = suggestions;
    els.lspPopup.innerHTML = '';

    suggestions.forEach((item, idx) => {
      const div = document.createElement('div');
      div.className = `lsp-item ${idx === 0 ? 'active' : ''}`;
      div.dataset.label = item.label;
      div.innerHTML = `<span>${item.label}</span><span class="lsp-kind">${item.kind}</span>`;

      div.addEventListener('click', () => {
        insertLspSuggestion(item.label);
      });

      els.lspPopup.appendChild(div);
    });

    els.lspPopup.style.left = '60px';
    els.lspPopup.style.top = '40px';
    els.lspPopup.classList.remove('hidden');
  }

  function insertLspSuggestion(suggestion) {
    const text = els.codeTextarea.value;
    const pos = els.codeTextarea.selectionStart;
    const left = text.substring(0, pos);
    const right = text.substring(pos);
    const match = left.match(/([a-zA-Z_]\w*)$/);

    let insertText = suggestion;
    let cursorOffset = suggestion.length;

    if (suggestion.endsWith('()')) {
      cursorOffset = suggestion.length - 1;
    } else if (suggestion.endsWith('!()')) {
      cursorOffset = suggestion.length - 1;
    } else if (suggestion.endsWith('{}')) {
      cursorOffset = suggestion.length - 1;
    } else if (suggestion.endsWith('[]')) {
      cursorOffset = suggestion.length - 1;
    } else if (suggestion.endsWith('""') || suggestion.endsWith("''") || suggestion.endsWith('``')) {
      cursorOffset = suggestion.length - 1;
    }

    if (match) {
      const start = pos - match[1].length;
      els.codeTextarea.value = text.substring(0, start) + insertText + right;
      const finalPos = start + cursorOffset;
      els.codeTextarea.selectionStart = els.codeTextarea.selectionEnd = finalPos;
    } else {
      els.codeTextarea.value = left + insertText + right;
      const finalPos = pos + cursorOffset;
      els.codeTextarea.selectionStart = els.codeTextarea.selectionEnd = finalPos;
    }

    els.lspPopup.classList.add('hidden');
    renderGutterAndSyntax();
    updateCursorPos();
  }

  function initResizablePanes() {
    let isDraggingSidebar = false;
    els.resizerSidebar.addEventListener('mousedown', () => {
      isDraggingSidebar = true;
      els.resizerSidebar.classList.add('dragging');
      document.body.style.cursor = 'col-resize';
      document.body.style.userSelect = 'none';
    });

    let isDraggingPanel = false;
    els.resizerPanel.addEventListener('mousedown', () => {
      isDraggingPanel = true;
      els.resizerPanel.classList.add('dragging');
      document.body.style.cursor = 'row-resize';
      document.body.style.userSelect = 'none';
    });

    document.addEventListener('mousemove', (e) => {
      if (isDraggingSidebar) {
        const newWidth = e.clientX - 48;
        if (newWidth >= 140 && newWidth <= 600) {
          els.sidebar.style.width = `${newWidth}px`;
        }
      } else if (isDraggingPanel) {
        const newHeight = window.innerHeight - e.clientY - 24;
        if (newHeight >= 80 && newHeight <= window.innerHeight - 150) {
          els.bottomPanel.style.height = `${newHeight}px`;
        }
      }
    });

    document.addEventListener('mouseup', () => {
      if (isDraggingSidebar || isDraggingPanel) {
        isDraggingSidebar = false;
        isDraggingPanel = false;
        els.resizerSidebar.classList.remove('dragging');
        els.resizerPanel.classList.remove('dragging');
        document.body.style.cursor = '';
        document.body.style.userSelect = '';
      }
    });
  }

  function initVSCodeTerminal() {
    els.vscodeTerminal.addEventListener('keydown', (e) => {
      e.stopPropagation();

      if (e.key === 'Enter') {
        const cmd = state.termInputBuffer.trim();
        if (cmd) {
          if (cmd.toLowerCase() === 'cls' || cmd.toLowerCase() === 'clear') {
            els.termLog.textContent = '';
            state.termInputBuffer = '';
            els.termInputText.textContent = '';
            return;
          }
          wsSend({ type: 'terminal:input', payload: { command: cmd } });
        } else {
          els.termLog.textContent += `${els.termPromptStr.textContent}\n`;
          els.vscodeTerminal.scrollTop = els.vscodeTerminal.scrollHeight;
        }
      } else if (e.key === 'Backspace') {
        state.termInputBuffer = state.termInputBuffer.slice(0, -1);
        els.termInputText.textContent = state.termInputBuffer;
      } else if (e.key.length === 1 && !e.ctrlKey && !e.altKey && !e.metaKey) {
        state.termInputBuffer += e.key;
        els.termInputText.textContent = state.termInputBuffer;
      }
    });

    els.vscodeTerminal.addEventListener('click', () => {
      els.vscodeTerminal.focus();
    });
  }

  function initLiveSettings() {
    const openSettings = () => els.settingsBackdrop.classList.remove('hidden');
    const closeSettings = () => els.settingsBackdrop.classList.add('hidden');

    document.getElementById('btn-settings-activity').addEventListener('click', openSettings);
    els.btnCloseSettings.addEventListener('click', closeSettings);

    els.settingsBackdrop.addEventListener('click', (e) => {
      if (e.target === els.settingsBackdrop) closeSettings();
    });

    els.cfgGlassOpacity.addEventListener('input', () => {
      const val = els.cfgGlassOpacity.value;
      const opacityDecimal = (val / 100).toFixed(2);
      document.documentElement.style.setProperty('--glass-opacity', opacityDecimal);
      els.valGlassOpacity.textContent = `${val}%`;
      savePersistedSettings();
    });

    els.cfgGlassBlur.addEventListener('input', () => {
      const val = els.cfgGlassBlur.value;
      document.documentElement.style.setProperty('--glass-blur', `${val}px`);
      els.valGlassBlur.textContent = `${val}px`;
      savePersistedSettings();
    });

    els.cfgFontSize.addEventListener('input', () => {
      const val = els.cfgFontSize.value;
      document.documentElement.style.setProperty('--font-size-code', `${val}px`);
      els.valFontSize.textContent = `${val}px`;
      renderGutterAndSyntax();
      savePersistedSettings();
    });
  }

  function openFileTab(filePath, content) {
    let file = state.openFiles.find(f => f.path === filePath);
    if (!file) {
      file = { path: filePath, name: filePath.split(/[/\\]/).pop(), content, dirty: false };
      state.openFiles.push(file);
    } else {
      file.content = content;
    }

    state.activeFilePath = filePath;
    renderTabs();
    updateEditorContent();
    updateLspStatusForActiveFile();
  }

  function renderTabs() {
    els.tabsContainer.innerHTML = '';
    state.openFiles.forEach(file => {
      const tab = document.createElement('div');
      tab.className = `tab-item ${file.path === state.activeFilePath ? 'active' : ''} ${file.dirty ? 'dirty' : ''}`;
      tab.innerHTML = `
        <span class="tab-title">${file.name}</span>
        ${file.dirty ? '<span class="tab-dirty-icon"></span>' : ''}
        <span class="tab-close">✕</span>
      `;

      tab.addEventListener('click', () => {
        state.activeFilePath = file.path;
        renderTabs();
        updateEditorContent();
        updateLspStatusForActiveFile();
      });

      tab.querySelector('.tab-close').addEventListener('click', (e) => {
        e.stopPropagation();
        closeFile(file.path);
      });

      els.tabsContainer.appendChild(tab);
    });

    if (state.openFiles.length === 0) {
      els.emptyState.classList.remove('hidden');
      els.breadcrumbPath.textContent = 'workspace';
    } else {
      els.emptyState.classList.add('hidden');
      const active = state.openFiles.find(f => f.path === state.activeFilePath);
      if (active) {
        els.breadcrumbPath.textContent = `workspace > ${active.path}`;
      }
    }
  }

  function closeFile(path) {
    const idx = state.openFiles.findIndex(f => f.path === path);
    if (idx !== -1) {
      state.openFiles.splice(idx, 1);
      if (state.activeFilePath === path) {
        state.activeFilePath = state.openFiles.length > 0 ? state.openFiles[Math.max(0, idx - 1)].path : null;
      }
      renderTabs();
      updateEditorContent();
      updateLspStatusForActiveFile();
    }
  }

  function updateEditorContent() {
    const active = state.openFiles.find(f => f.path === state.activeFilePath);
    if (!active) {
      els.codeTextarea.value = '';
      els.syntaxOverlay.innerHTML = '';
      els.gutter.innerHTML = '';
      return;
    }

    els.codeTextarea.value = active.content;
    renderGutterAndSyntax();
  }

  function renderGutterAndSyntax() {
    const val = els.codeTextarea.value;
    const lines = val.split('\n');

    let gutterHtml = '';
    for (let i = 1; i <= lines.length; i++) {
      gutterHtml += `<div>${i}</div>`;
    }
    els.gutter.innerHTML = gutterHtml;

    const ext = state.activeFilePath ? state.activeFilePath.split('.').pop().toLowerCase() : '';
    els.syntaxOverlay.innerHTML = highlightSyntax(val, ext);

    els.syntaxOverlay.scrollTop = els.codeTextarea.scrollTop;
    els.syntaxOverlay.scrollLeft = els.codeTextarea.scrollLeft;
    els.gutter.scrollTop = els.codeTextarea.scrollTop;
  }

  function highlightSyntax(code, ext) {
    if (!code) return '';
    const escaped = code.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    if (!['js', 'json', 'java', 'cpp', 'c', 'html', 'css', 'sh', 'ps1', 'rs', 'md'].includes(ext)) {
      return escaped;
    }

    const tokenRegex = /(\/\/.+$|\/\*[\s\S]*?\*\/|#.+)|("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|`(?:[^`\\]|\\.)*`)|(\b(?:const|let|var|function|return|if|else|for|while|import|from|export|class|extends|public|private|static|new|async|await|def|fn|pub|struct|enum|impl|mut|print|echo|try|catch)\b)|(\b[a-zA-Z_]\w*(?=\())|(\b\d+\b)/g;

    return escaped.replace(tokenRegex, (match, comment, str, kw, fn, num) => {
      if (comment) return `<span class="token-comment">${comment}</span>`;
      if (str) return `<span class="token-string">${str}</span>`;
      if (kw) return `<span class="token-keyword">${kw}</span>`;
      if (fn) return `<span class="token-function">${fn}</span>`;
      if (num) return `<span class="token-number">${num}</span>`;
      return match;
    });
  }

  function saveActiveFile() {
    const active = state.openFiles.find(f => f.path === state.activeFilePath);
    if (active) {
      wsSend({ type: 'fs:write', payload: { path: active.path, content: active.content } });
      active.dirty = false;
      renderTabs();
    }
  }

  function jumpToLine(lineNumber) {
    const lines = els.codeTextarea.value.split('\n');
    let pos = 0;
    for (let i = 0; i < Math.min(lineNumber - 1, lines.length); i++) {
      pos += lines[i].length + 1;
    }
    els.codeTextarea.focus();
    els.codeTextarea.setSelectionRange(pos, pos + (lines[lineNumber - 1] || '').length);
    updateCursorPos();
  }

  function updateCursorPos() {
    const text = els.codeTextarea.value;
    const pos = els.codeTextarea.selectionStart;
    const lines = text.substring(0, pos).split('\n');
    els.statusPos.textContent = `Ln ${lines.length}, Col ${lines[lines.length - 1].length + 1}`;
  }

  function initMenuActions() {
    document.addEventListener('click', () => {
      els.contextMenu.classList.add('hidden');
      els.workspaceContextMenu.classList.add('hidden');
      els.menubarDropdown.classList.add('hidden');
    });

    document.querySelectorAll('.menu-item').forEach(item => {
      item.addEventListener('click', (e) => {
        e.stopPropagation();
        const menuType = item.dataset.menu;
        const rect = item.getBoundingClientRect();
        showMenubarDropdown(menuType, rect.left, rect.bottom);
      });
    });

    els.contextMenu.querySelectorAll('.menu-option').forEach(opt => {
      opt.addEventListener('click', (e) => {
        e.stopPropagation();
        els.contextMenu.classList.add('hidden');
        const action = opt.dataset.action;
        const targetPath = els.contextMenu.dataset.targetPath;

        if (action === 'new-file') {
          createInlineItem(false);
        } else if (action === 'new-folder') {
          createInlineItem(true);
        } else if (action === 'rename') {
          const newName = prompt('New name:', targetPath);
          if (newName && newName !== targetPath) {
            wsSend({ type: 'fs:rename', payload: { oldPath: targetPath, newPath: newName } });
          }
        } else if (action === 'delete') {
          if (confirm(`Delete ${targetPath}?`)) {
            wsSend({ type: 'fs:delete', payload: { path: targetPath } });
            closeFile(targetPath);
          }
        } else if (action === 'copy-path') {
          navigator.clipboard.writeText(targetPath);
        }
      });
    });
  }

  function showContextMenu(x, y, targetPath, isDir) {
    els.contextMenu.dataset.targetPath = targetPath || '';
    els.contextMenu.dataset.isDir = isDir ? 'true' : 'false';
    els.contextMenu.style.left = `${Math.min(x, window.innerWidth - 240)}px`;
    els.contextMenu.style.top = `${Math.min(y, window.innerHeight - 200)}px`;
    els.contextMenu.classList.remove('hidden');
  }

  function showMenubarDropdown(type, x, y) {
    els.menubarDropdown.style.left = `${x}px`;
    els.menubarDropdown.style.top = `${y + 4}px`;

    let html = '';
    if (type === 'file') {
      html = `
        <div class="menu-option" id="m-new-file"><span>New File</span></div>
        <div class="menu-option" id="m-new-folder"><span>New Folder</span></div>
        <div class="menu-divider"></div>
        <div class="menu-option" id="m-save-file"><span>Save File</span><span class="shortcut">Ctrl+S</span></div>
        <div class="menu-divider"></div>
        <div class="menu-option" id="m-refresh"><span>Refresh Explorer</span></div>
      `;
    } else if (type === 'edit') {
      html = `
        <div class="menu-option" id="m-undo"><span>Undo</span><span class="shortcut">Ctrl+Z</span></div>
        <div class="menu-option" id="m-redo"><span>Redo</span><span class="shortcut">Ctrl+Y</span></div>
        <div class="menu-divider"></div>
        <div class="menu-option" id="m-cut"><span>Cut</span><span class="shortcut">Ctrl+X</span></div>
        <div class="menu-option" id="m-copy"><span>Copy</span><span class="shortcut">Ctrl+C</span></div>
        <div class="menu-option" id="m-paste"><span>Paste</span><span class="shortcut">Ctrl+V</span></div>
        <div class="menu-divider"></div>
        <div class="menu-option" id="m-selectall"><span>Select All</span><span class="shortcut">Ctrl+A</span></div>
      `;
    }

    els.menubarDropdown.innerHTML = html;
    els.menubarDropdown.classList.remove('hidden');

    bindMenuHandler('m-new-file', () => createInlineItem(false));
    bindMenuHandler('m-new-folder', () => createInlineItem(true));
    bindMenuHandler('m-save-file', saveActiveFile);
    bindMenuHandler('m-refresh', () => wsSend({ type: 'fs:list', payload: { path: '' } }));
    bindMenuHandler('m-undo', () => document.execCommand('undo'));
    bindMenuHandler('m-redo', () => document.execCommand('redo'));
    bindMenuHandler('m-selectall', () => { els.codeTextarea.select(); });
  }

  function bindMenuHandler(id, fn) {
    const el = document.getElementById(id);
    if (el) el.addEventListener('click', fn);
  }

  let searchDebounceTimer = null;

  function initSearchEngine() {
    els.btnDoSearch.addEventListener('click', performSearch);
    els.searchQuery.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') performSearch();
    });
    els.searchQuery.addEventListener('input', () => {
      clearTimeout(searchDebounceTimer);
      searchDebounceTimer = setTimeout(() => {
        if (els.searchQuery.value.trim().length >= 2) {
          performSearch();
        }
      }, 350);
    });
  }

  function performSearch() {
    const query = els.searchQuery.value.trim();
    const metaEl = document.getElementById('search-meta');
    if (query.length < 2) {
      if (metaEl) metaEl.textContent = '';
      els.searchResults.innerHTML = `
        <div class="search-empty-state">
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" class="search-empty-icon"><circle cx="11" cy="11" r="8"/><path d="M21 21l-4.35-4.35"/></svg>
          <span>Type at least 2 characters to search across files</span>
        </div>
      `;
      return;
    }

    if (metaEl) metaEl.textContent = 'Searching workspace...';
    els.searchResults.innerHTML = `
      <div class="search-empty-state">
        <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" class="search-empty-icon"><circle cx="11" cy="11" r="8"/><path d="M21 21l-4.35-4.35"/></svg>
        <span>Searching workspace for "${escapeHtml(query)}"...</span>
      </div>
    `;
    wsSend({ type: 'fs:search', payload: { query } });
  }

  function renderSearchResults(results) {
    const metaEl = document.getElementById('search-meta');
    const query = els.searchQuery ? els.searchQuery.value.trim() : '';

    els.searchResults.innerHTML = '';
    if (!results || results.length === 0) {
      if (metaEl) metaEl.textContent = '0 matches found';
      els.searchResults.innerHTML = `
        <div class="search-empty-state">
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" class="search-empty-icon"><circle cx="11" cy="11" r="8"/><line x1="8" y1="11" x2="14" y2="11"/></svg>
          <span>No matching occurrences found for "${escapeHtml(query)}"</span>
        </div>
      `;
      return;
    }

    const uniqueFiles = new Set(results.map(r => r.file)).size;
    if (metaEl) {
      metaEl.textContent = `${results.length} match${results.length > 1 ? 'es' : ''} in ${uniqueFiles} file${uniqueFiles > 1 ? 's' : ''}`;
    }

    results.forEach(res => {
      const card = document.createElement('div');
      card.className = 'search-result-card';

      const fileNameOnly = res.file.split(/[/\\]/).pop();
      const dirPath = res.file.substring(0, res.file.length - fileNameOnly.length);

      const highlightedText = highlightQueryInText(res.text, query);

      card.innerHTML = `
        <div class="search-res-header">
          <span class="search-res-file" title="${escapeHtml(res.file)}">
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>
            <span>${escapeHtml(fileNameOnly)}</span>
            <span style="color:var(--fg-muted);font-weight:400;font-size:10.5px;">${escapeHtml(dirPath)}</span>
          </span>
          <span class="search-res-line-badge">Ln ${res.line}</span>
        </div>
        <div class="search-res-text">${highlightedText}</div>
      `;

      card.addEventListener('click', () => {
        wsSend({ type: 'fs:read', payload: { path: res.file } });
        setTimeout(() => jumpToLine(res.line), 150);
      });

      els.searchResults.appendChild(card);
    });
  }

  function highlightQueryInText(text, query) {
    if (!query) return escapeHtml(text);
    const escapedText = escapeHtml(text);
    const escapedQuery = escapeHtml(query);
    const regex = new RegExp(`(${escapedQuery.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')})`, 'gi');
    return escapedText.replace(regex, '<span class="search-highlight">$1</span>');
  }

  function escapeHtml(str) {
    return (str || '').replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  }

  function toggleTerminalPanel() {
    const panel = document.getElementById('bottom-panel');
    panel.style.display = panel.style.display === 'none' ? 'flex' : 'none';
    if (panel.style.display !== 'none') {
      els.vscodeTerminal.focus();
    }
  }

  function activateSidebarPanel(panelId) {
    els.sidebarPanels.forEach(p => p.classList.remove('active'));
    els.activityBtns.forEach(b => {
      b.classList.toggle('active', b.dataset.target === panelId);
    });
    const p = document.getElementById(panelId);
    if (p) p.classList.add('active');
  }

  function initGlobalEvents() {
    els.activityBtns.forEach(btn => {
      btn.addEventListener('click', () => {
        const target = btn.dataset.target;
        if (!target) return;
        activateSidebarPanel(target);
      });
    });

    els.panelTabs.forEach(tab => {
      tab.addEventListener('click', () => {
        const panelId = tab.dataset.panel;
        els.panelTabs.forEach(t => t.classList.toggle('active', t.dataset.panel === panelId));
        els.panelViews.forEach(v => v.classList.toggle('active', v.id === panelId));
        if (panelId === 'panel-terminal') {
          els.vscodeTerminal.focus();
        }
      });
    });

    els.btnClearOutput.addEventListener('click', () => {
      els.termLog.textContent = '';
      els.stderrBuffer.textContent = '[STDERR Stream] Log cleared.';
      if (els.buildBuffer) els.buildBuffer.textContent = '[Build Console reset]';
      state.stderrCount = 0;
      els.stderrBadge.style.display = 'none';
    });

    if (els.btnRunBuildActivity) {
      els.btnRunBuildActivity.addEventListener('click', () => {
        const panel = document.getElementById('bottom-panel');
        panel.style.display = 'flex';
        els.panelTabs.forEach(t => t.classList.toggle('active', t.dataset.panel === 'panel-build'));
        els.panelViews.forEach(v => v.classList.toggle('active', v.id === 'panel-build'));
        if (els.buildBuffer) els.buildBuffer.textContent = '[Building workspace...]';
        wsSend({ type: 'build:run' });
      });
    }

    document.getElementById('btn-toggle-terminal-activity').addEventListener('click', toggleTerminalPanel);

    els.codeTextarea.addEventListener('input', () => {
      const active = state.openFiles.find(f => f.path === state.activeFilePath);
      if (active) {
        active.content = els.codeTextarea.value;
        if (!active.dirty) {
          active.dirty = true;
          renderTabs();
        }
      }
      renderGutterAndSyntax();
      updateCursorPos();
    });

    els.codeTextarea.addEventListener('keyup', updateCursorPos);
    els.codeTextarea.addEventListener('click', updateCursorPos);
    els.codeTextarea.addEventListener('scroll', () => {
      els.syntaxOverlay.scrollTop = els.codeTextarea.scrollTop;
      els.syntaxOverlay.scrollLeft = els.codeTextarea.scrollLeft;
      els.gutter.scrollTop = els.codeTextarea.scrollTop;
    });

    window.addEventListener('keydown', (e) => {
      if ((e.ctrlKey || e.metaKey) && e.key === '`') {
        e.preventDefault();
        toggleTerminalPanel();
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
        e.preventDefault();
        saveActiveFile();
      }
    });
  }

})();
