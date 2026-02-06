<script lang="ts">
import { onMount } from "svelte";
import { invoke } from "@tauri-apps/api/tauri";
import { open } from '@tauri-apps/api/dialog';
import { appWindow } from "@tauri-apps/api/window";

let mode = "extract";
let path = "";
let destination = "";
let extractFormat = "auto";
let compressFormat = "tar.zst";
let level = 3;
let busy = false;
let message = "";
let detectedFormat = "";
let showSuccess = false;
let dropModalOpen = false;
let dropPaths: string[] = [];
let dropAction: "extract" | "compress" | null = null;
let dropFormat = "";
let dropWarning = "";
let compressSourceKind: "file" | "folder" = "folder";

type ExtractReport = {
  entries: number;
  bytes_written: number;
  warnings: string[];
};

type CompressReport = {
  files: number;
  bytes_read: number;
  bytes_written: number;
  compression_ratio: number;
};

let extractReport: ExtractReport | null = null;
let compressReport: CompressReport | null = null;
let lastMode = mode;

// iOS-style icon components
const icons = {
  extract: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>`,
  compress: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 16 12 12 8 16"/><line x1="12" y1="12" x2="12" y2="21"/><path d="M20.24 12.24a6 6 0 0 0-8.49-8.49L5 10.5V19h8.5z"/></svg>`,
  folder: `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>`,
  check: `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="#34C759" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>`,
  file: `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/><polyline points="13 2 13 9 20 9"/></svg>`
};

// File picker functions
async function selectInputSource(kind?: "file" | "folder") {
  try {
    let selectFolder = false;
    if (mode === "compress") {
      if (kind) {
        compressSourceKind = kind;
      }
      selectFolder = (kind ?? compressSourceKind) === "folder";
    }
    const selected = await open({
      multiple: false,
      directory: selectFolder,
    });
    if (selected && typeof selected === 'string') {
      path = selected;
      detectFileFormat();
    }
  } catch (err) {
    console.error('Selection cancelled');
  }
}

async function selectOutputDirectory() {
  try {
    const selected = await open({
      directory: true,
    });
    if (selected && typeof selected === 'string') {
      destination = selected;
    }
  } catch (err) {
    console.error('Directory selection cancelled');
  }
}

// Auto-detect format when path changes (in extract mode)
async function detectFileFormat() {
  if (mode === "extract" && path && extractFormat === "auto") {
    try {
      const detected = await invoke<string>("detect_format", { path });
      detectedFormat = detected !== "unknown" ? detected : "";
    } catch (err) {
      detectedFormat = "";
    }
  } else {
    detectedFormat = "";
  }
}

// Watch for path changes
$: if (path) {
  detectFileFormat();
  showSuccess = false;
}

$: if (destination) {
  showSuccess = false;
}

$: if (mode) {
  message = "";
  showSuccess = false;
  if (mode === "compress") {
    detectedFormat = "";
  }
  if (mode !== lastMode) {
    path = "";
    destination = "";
    extractReport = null;
    compressReport = null;
    lastMode = mode;
  }
}

async function runExtract() {
  if (!path || !destination) {
    message = "Please select input and output";
    return;
  }
  busy = true;
  message = "";
  showSuccess = false;
  try {
    const started = performance.now();
    const result = await invoke<ExtractReport>("extract_archive", { path, destination, format: extractFormat });
    const elapsed = performance.now() - started;
    extractReport = result;
    compressReport = null;
    message = `${result.entries} files`;
    showSuccess = true;
  } catch (err) {
    message = `Error: ${err}`;
    showSuccess = false;
  } finally {
    busy = false;
  }
}

async function runCompress() {
  if (!path || !destination) {
    message = "Please select input and output";
    return;
  }
  if (compressFormat === "rar") {
    message = "RAR compression is not supported";
    return;
  }
  busy = true;
  message = "";
  showSuccess = false;
  try {
    const started = performance.now();
    const outputPath = buildCompressDestination();
    const result = await invoke<CompressReport>("compress_archive", { source: path, destination: outputPath, format: compressFormat, level });
    const elapsed = performance.now() - started;
    compressReport = result;
    extractReport = null;
    message = `${result.files} files`;
    showSuccess = true;
  } catch (err) {
    message = `Error: ${err}`;
    showSuccess = false;
  } finally {
    busy = false;
  }
}

function runOperation() {
  if (mode === "extract") {
    runExtract();
  } else {
    runCompress();
  }
}

function formatFileSize(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

function getFileName(filepath: string): string {
  const parts = filepath.split(/[/\\]/);
  return parts[parts.length - 1] || filepath;
}

function detectActionFromPath(filePath: string): "extract" | "compress" {
  const lower = filePath.toLowerCase();
  if (lower.endsWith(".zip") || lower.endsWith(".7z") || lower.endsWith(".rar") || lower.endsWith(".tar.gz") || lower.endsWith(".tgz") || lower.endsWith(".tar.zst") || lower.endsWith(".tar.lz4") || lower.endsWith(".tar.br") || lower.endsWith(".tar")) {
    return "extract";
  }
  return "compress";
}

async function setupDragDrop() {
  await appWindow.onFileDropEvent(async (event) => {
    if (event.payload.type !== "drop") return;
    const paths = event.payload.paths ?? [];
    if (paths.length === 0) return;
    dropPaths = paths;
    const primary = paths[0];
    dropAction = detectActionFromPath(primary);
    dropWarning = paths.length > 1 ? "Multiple items dropped. Only the first will be processed." : "";
    if (dropAction === "extract") {
      const detected = await invoke<string>("detect_format", { path: primary }).catch(() => "");
      dropFormat = detected && detected !== "unknown" ? detected : "";
    } else {
      dropFormat = "";
    }
    dropModalOpen = true;
  });
}

onMount(() => {
  setupDragDrop();
});

async function selectDropDestination() {
  const selected = await open({ directory: true });
  if (selected && typeof selected === "string") {
    destination = selected;
  }
}

async function confirmDropAction() {
  if (!dropPaths.length || !dropAction) return;
  if (!destination) {
    message = "Please select output destination";
    return;
  }
  path = dropPaths[0];
  if (dropAction === "extract") {
    mode = "extract";
    extractFormat = dropFormat || "auto";
    await runExtract();
  } else {
    mode = "compress";
    compressFormat = "tar.zst";
    await runCompress();
  }
  dropModalOpen = false;
}

function closeDropModal() {
  dropModalOpen = false;
}

function getBaseName(filepath: string): string {
  const name = getFileName(filepath);
  const lastDot = name.lastIndexOf(".");
  return lastDot > 0 ? name.slice(0, lastDot) : name || "archive";
}

function buildCompressDestination(): string {
  const separator = destination.includes("\\") ? "\\" : "/";
  const baseName = getBaseName(path);
  const extension = compressFormat === "zip" ? "zip" : compressFormat;
  const fileName = `${baseName}.${extension}`;
  if (!destination) return fileName;
  if (destination.endsWith("/") || destination.endsWith("\\")) {
    return `${destination}${fileName}`;
  }
  return `${destination}${separator}${fileName}`;
}
</script>

<main>
  <div class="ios-container">
    <!-- Header -->
    <header class="ios-header">
      <div class="header-title">淬 Quench</div>
      <div class="header-subtitle">Compression Tool</div>
    </header>

    <!-- Mode Switcher - iOS Segmented Control -->
    <div class="mode-switcher">
      <button
        class="mode-btn"
        class:active={mode === "extract"}
        on:click={() => mode = "extract"}
      >
        {@html icons.extract}
        <span>Extract</span>
      </button>
      <button
        class="mode-btn"
        class:active={mode === "compress"}
        on:click={() => mode = "compress"}
      >
        {@html icons.compress}
        <span>Compress</span>
      </button>
    </div>

    <!-- Main Content Card -->
    <div class="content-card">
      <!-- Input Section -->
      <div class="input-section">
        <div class="section-label">
          {mode === "extract" ? "Archive" : "Source"}
        </div>
        {#if mode === "compress"}
          <div class="source-kind">
            <span class="source-kind-label">Quick Pick</span>
            <div class="source-kind-actions">
              <button type="button" class="source-kind-btn primary" on:click={() => selectInputSource("folder")}>Choose folder</button>
              <button type="button" class="source-kind-btn ghost" on:click={() => selectInputSource("file")}>Choose file</button>
            </div>
          </div>
        {/if}
        <button type="button" class="file-selector" on:click={() => selectInputSource()}>
          <div class="file-icon">
            {@html mode === "compress" && compressSourceKind === "folder" ? icons.folder : icons.file}
          </div>
          <div class="file-info">
            <div class="file-name">
              {path ? getFileName(path) : (mode === "compress" && compressSourceKind === "folder" ? "Choose folder" : "Choose file")}
            </div>
            <div class="file-path">
              {path || "Click to browse"}
            </div>
          </div>
          <div class="chevron">›</div>
        </button>
      </div>

      <!-- Output Section -->
      <div class="input-section">
        <div class="section-label">Destination</div>
        <button type="button" class="file-selector" on:click={selectOutputDirectory}>
          <div class="file-icon">
            {@html icons.folder}
          </div>
          <div class="file-info">
            <div class="file-name">
              {destination ? getFileName(destination) : "Choose folder"}
            </div>
            <div class="file-path">
              {destination || "Click to browse"}
            </div>
          </div>
          <div class="chevron">›</div>
        </button>
      </div>

      <!-- Format Selection -->
      {#if mode === "extract"}
        <div class="input-section">
          <div class="section-label">Format</div>
          <div class="format-selector">
            <select bind:value={extractFormat} class="ios-select">
              <option value="auto">Auto-Detect</option>
              <option value="tar.zst">tar.zst</option>
              <option value="tar.lz4">tar.lz4</option>
              <option value="tar.br">tar.br</option>
              <option value="zip">zip</option>
              <option value="7z">7z</option>
              <option value="rar">rar</option>
            </select>
            {#if detectedFormat}
              <div class="detected-badge">{detectedFormat}</div>
            {/if}
          </div>
        </div>
      {:else}
        <div class="input-section">
          <div class="section-label">Format</div>
          <div class="format-selector">
            <select bind:value={compressFormat} class="ios-select">
              <option value="tar.zst">tar.zst</option>
              <option value="tar.lz4">tar.lz4</option>
              <option value="tar.br">tar.br</option>
              <option value="zip">zip</option>
              <option value="7z">7z</option>
            </select>
          </div>
        </div>
      {/if}

      <!-- Compression Level (compress mode only) -->
      {#if mode === "compress"}
        <div class="input-section">
          <div class="section-label">Compression Level</div>
          <div class="level-container">
            <input
              type="range"
              min="1"
              max="20"
              bind:value={level}
              class="ios-slider"
            />
            <div class="level-value">{level}</div>
          </div>
          <div class="level-labels">
            <span>Fast</span>
            <span>Small</span>
          </div>
        </div>
      {/if}

      <!-- Action Button -->
      <button
        class="ios-button"
        class:busy={busy}
        class:success={showSuccess}
        on:click={runOperation}
        disabled={busy || !path || !destination}
      >
        {#if busy}
          <span class="spinner"></span>
          <span>{mode === "extract" ? "Extracting..." : "Compressing..."}</span>
        {:else if showSuccess}
          {@html icons.check}
          <span>{message}</span>
        {:else}
          <span>{mode === "extract" ? "Extract Archive" : "Compress"}</span>
        {/if}
      </button>

      <!-- Status Message -->
      {#if message && !showSuccess}
        <div class="status-message" class:error={message.startsWith('Error')}>
          {message}
        </div>
      {/if}

      <!-- Results Card -->
      {#if showSuccess && (extractReport || compressReport)}
        <div class="results-card">
          {#if extractReport}
            <div class="result-item">
              <span class="result-label">Files</span>
              <span class="result-value">{extractReport.entries}</span>
            </div>
            <div class="result-item">
              <span class="result-label">Size</span>
              <span class="result-value">{formatFileSize(extractReport.bytes_written)}</span>
            </div>
            {#if extractReport.warnings.length > 0}
              <div class="warnings-section">
                <div class="warnings-title">Warnings ({extractReport.warnings.length})</div>
                {#each extractReport.warnings as warning}
                  <div class="warning-item">{warning}</div>
                {/each}
              </div>
            {/if}
          {/if}

          {#if compressReport}
            <div class="result-item">
              <span class="result-label">Files</span>
              <span class="result-value">{compressReport.files}</span>
            </div>
            <div class="result-item">
              <span class="result-label">Input</span>
              <span class="result-value">{formatFileSize(compressReport.bytes_read)}</span>
            </div>
            <div class="result-item">
              <span class="result-label">Output</span>
              <span class="result-value">{formatFileSize(compressReport.bytes_written)}</span>
            </div>
            <div class="result-item">
              <span class="result-label">Ratio</span>
              <span class="result-value">{(compressReport.compression_ratio * 100).toFixed(1)}%</span>
            </div>
          {/if}
        </div>
      {/if}
    </div>

    {#if dropModalOpen}
      <button type="button" class="modal-backdrop" on:click={closeDropModal} aria-label="Close"></button>
      <div class="ios-modal" role="dialog" aria-modal="true">
        <div class="ios-modal-header">Dropped item</div>
        <div class="ios-modal-body">
          <div class="modal-row">
            <span class="modal-label">File</span>
            <span class="modal-value">{dropPaths[0] ? getFileName(dropPaths[0]) : ""}</span>
          </div>
          <div class="modal-row">
            <span class="modal-label">Action</span>
            <span class="modal-value">{dropAction === "extract" ? "Extract" : "Compress"}</span>
          </div>
          {#if dropAction === "extract"}
            <div class="modal-row">
              <span class="modal-label">Format</span>
              <span class="modal-value">{dropFormat || "auto"}</span>
            </div>
          {/if}
          {#if dropWarning}
            <div class="modal-warning">{dropWarning}</div>
          {/if}
          <div class="modal-destination">
            <button type="button" class="file-selector" on:click={selectDropDestination}>
              <div class="file-icon">
                {@html icons.folder}
              </div>
              <div class="file-info">
                <div class="file-name">
                  {destination ? getFileName(destination) : "Choose destination"}
                </div>
                <div class="file-path">
                  {destination || "Click to browse"}
                </div>
              </div>
              <div class="chevron">›</div>
            </button>
          </div>
        </div>
        <div class="ios-modal-actions">
          <button type="button" class="ios-modal-btn ghost" on:click={closeDropModal}>Cancel</button>
          <button type="button" class="ios-modal-btn primary" on:click={confirmDropAction}>Run</button>
        </div>
      </div>
    {/if}

    <!-- Footer -->
    <footer class="ios-footer">
      <span class="footer-text">Made with ❤️ in Rust + Svelte</span>
    </footer>
  </div>
</main>

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    font-family: -apple-system, BlinkMacSystemFont, "SF Pro Display", "Segoe UI", Roboto, sans-serif;
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
  }

  main {
    min-height: 100vh;
    background: linear-gradient(135deg, #f5f7fa 0%, #e8ecf1 100%);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 20px;
  }

  .ios-container {
    width: 100%;
    max-width: 480px;
    animation: fadeIn 0.4s ease-out;
  }

  @keyframes fadeIn {
    from {
      opacity: 0;
      transform: translateY(20px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  /* iOS Header */
  .ios-header {
    text-align: center;
    margin-bottom: 32px;
    animation: slideDown 0.5s ease-out;
  }

  .header-title {
    font-size: 42px;
    font-weight: 700;
    letter-spacing: -0.5px;
    color: #1c1c1e;
    margin-bottom: 4px;
  }

  @keyframes slideDown {
    from {
      opacity: 0;
      transform: translateY(-30px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }


  .header-subtitle {
    font-size: 15px;
    color: #8e8e93;
    font-weight: 400;
  }

  /* iOS Segmented Control */
  .mode-switcher {
    display: flex;
    background: #e5e5ea;
    border-radius: 14px;
    padding: 3px;
    margin-bottom: 32px;
    box-shadow: inset 0 1px 3px rgba(0, 0, 0, 0.08);
  }

  .mode-btn {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 10px 16px;
    border: none;
    background: transparent;
    border-radius: 11px;
    font-size: 15px;
    font-weight: 500;
    color: #8e8e93;
    cursor: pointer;
    transition: all 0.2s cubic-bezier(0.25, 0.1, 0.25, 1);
  }

  .mode-btn:hover {
    color: #1c1c1e;
  }

  .mode-btn.active {
    background: #ffffff;
    color: #007aff;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.12);
  }

  .mode-btn :global(svg) {
    width: 18px;
    height: 18px;
  }

  /* Content Card */
  .content-card {
    background: rgba(255, 255, 255, 0.8);
    backdrop-filter: blur(20px) saturate(180%);
    -webkit-backdrop-filter: blur(20px) saturate(180%);
    border-radius: 24px;
    padding: 28px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.5);
    animation: scaleIn 0.3s ease-out;
  }

  @keyframes scaleIn {
    from {
      opacity: 0;
      transform: scale(0.95);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }

  /* Input Sections */
  .input-section {
    margin-bottom: 24px;
    animation: slideIn 0.4s ease-out;
    animation-fill-mode: both;
  }

  .input-section:nth-child(1) {
    animation-delay: 0.1s;
  }

  .input-section:nth-child(2) {
    animation-delay: 0.15s;
  }

  .input-section:nth-child(3) {
    animation-delay: 0.2s;
  }

  @keyframes slideIn {
    from {
      opacity: 0;
      transform: translateX(-20px);
    }
    to {
      opacity: 1;
      transform: translateX(0);
    }
  }

  .section-label {
    font-size: 13px;
    font-weight: 600;
    color: #8e8e93;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 10px;
    margin-left: 4px;
  }

  /* File Selector */
  .file-selector {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 16px;
    background: #f5f5f7;
    border: none;
    border-radius: 16px;
    cursor: pointer;
    transition: all 0.2s ease;
    border: 2px solid transparent;
    width: 100%;
    text-align: left;
    font: inherit;
  }

  .file-selector:hover {
    background: #ebebf0;
    transform: translateY(-1px);
  }

  .file-selector:active {
    transform: scale(0.98);
    background: #e0e0e5;
  }

  .file-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 44px;
    height: 44px;
    background: linear-gradient(135deg, #007aff, #5856d6);
    border-radius: 12px;
    color: white;
  }

  .file-icon :global(svg) {
    width: 22px;
    height: 22px;
  }

  .file-info {
    flex: 1;
    min-width: 0;
  }

  .file-name {
    font-size: 17px;
    font-weight: 600;
    color: #1c1c1e;
    margin-bottom: 2px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .file-path {
    font-size: 14px;
    color: #8e8e93;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .chevron {
    font-size: 24px;
    color: #c7c7cc;
    font-weight: 300;
  }

  /* Format Selector */
  .format-selector {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .source-kind {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 12px;
  }

  .source-kind-label {
    font-size: 12px;
    font-weight: 600;
    color: #8e8e93;
    text-transform: uppercase;
    letter-spacing: 0.4px;
  }

  .source-kind-actions {
    display: flex;
    gap: 8px;
  }

  .source-kind-btn {
    border: none;
    border-radius: 10px;
    padding: 8px 12px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    background: #f2f2f7;
    color: #1c1c1e;
  }

  .source-kind-btn.primary {
    background: linear-gradient(135deg, #007aff, #5856d6);
    color: #ffffff;
  }

  .source-kind-btn.ghost {
    background: #f2f2f7;
    color: #1c1c1e;
  }

  .ios-select {
    flex: 1;
    padding: 14px 18px;
    background: #f5f5f7;
    border: none;
    border-radius: 14px;
    font-size: 17px;
    font-weight: 500;
    color: #1c1c1e;
    appearance: none;
    -webkit-appearance: none;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .ios-select:focus {
    outline: none;
    background: #ebebf0;
  }


  .detected-badge {
    padding: 6px 14px;
    background: linear-gradient(135deg, #34c759, #30d158);
    color: white;
    font-size: 13px;
    font-weight: 600;
    border-radius: 20px;
    box-shadow: 0 2px 8px rgba(52, 199, 89, 0.3);
  }

  /* Level Slider */
  .level-container {
    display: flex;
    align-items: center;
    gap: 16px;
    margin-bottom: 8px;
  }

  .ios-slider {
    flex: 1;
    height: 6px;
    border-radius: 3px;
    appearance: none;
    background: #e5e5ea;
    outline: none;
  }

  .ios-slider::-webkit-slider-thumb {
    appearance: none;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: #ffffff;
    cursor: pointer;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
    border: 1px solid rgba(0, 0, 0, 0.1);
    transition: transform 0.2s ease;
  }

  .ios-slider::-webkit-slider-thumb:hover {
    transform: scale(1.1);
  }

  .ios-slider::-webkit-slider-thumb:active {
    transform: scale(0.95);
  }

  .level-value {
    min-width: 32px;
    text-align: center;
    font-size: 20px;
    font-weight: 700;
    color: #007aff;
  }

  .level-labels {
    display: flex;
    justify-content: space-between;
    padding: 0 4px;
  }

  .level-labels span {
    font-size: 12px;
    color: #8e8e93;
    font-weight: 500;
  }

  /* iOS Button */
  .ios-button {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 18px 32px;
    background: linear-gradient(135deg, #007aff, #5856d6);
    color: white;
    border: none;
    border-radius: 16px;
    font-size: 17px;
    font-weight: 600;
    letter-spacing: 0.3px;
    cursor: pointer;
    transition: all 0.3s cubic-bezier(0.25, 0.1, 0.25, 1);
    box-shadow: 0 4px 16px rgba(0, 122, 255, 0.3);
  }

  .ios-button:hover:not(:disabled) {
    transform: translateY(-2px);
    box-shadow: 0 6px 24px rgba(0, 122, 255, 0.4);
  }

  .ios-button:active:not(:disabled) {
    transform: translateY(0);
    box-shadow: 0 2px 12px rgba(0, 122, 255, 0.3);
  }

  .ios-button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    transform: none !important;
  }

  .ios-button.busy {
    background: linear-gradient(135deg, #8e8e93, #636366);
    box-shadow: none;
  }

  .ios-button.success {
    background: linear-gradient(135deg, #34c759, #30d158);
    box-shadow: 0 4px 16px rgba(52, 199, 89, 0.3);
  }

  .ios-button :global(svg) {
    width: 22px;
    height: 22px;
  }

  /* Spinner */
  .spinner {
    width: 18px;
    height: 18px;
    border: 2px solid rgba(255, 255, 255, 0.3);
    border-top-color: white;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* Status Message */
  .status-message {
    margin-top: 20px;
    padding: 14px 20px;
    background: rgba(255, 255, 255, 0.9);
    border-radius: 14px;
    font-size: 15px;
    font-weight: 500;
    text-align: center;
    animation: shake 0.4s ease;
  }

  .status-message.error {
    background: #ff3b30;
    color: white;
  }

  @keyframes shake {
    0%, 100% { transform: translateX(0); }
    25% { transform: translateX(-5px); }
    75% { transform: translateX(5px); }
  }

  /* Results Card */
  .results-card {
    margin-top: 24px;
    padding: 20px;
    background: rgba(255, 255, 255, 0.6);
    backdrop-filter: blur(10px);
    border-radius: 20px;
    border: 1px solid rgba(255, 255, 255, 0.5);
    animation: expand 0.3s ease-out;
  }

  @keyframes expand {
    from {
      opacity: 0;
      transform: scale(0.95);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }

  .result-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 14px 0;
    border-bottom: 1px solid rgba(0, 0, 0, 0.05);
  }

  .result-item:last-child {
    border-bottom: none;
  }

  .result-label {
    font-size: 15px;
    color: #8e8e93;
    font-weight: 500;
  }

  .result-value {
    font-size: 17px;
    color: #1c1c1e;
    font-weight: 600;
  }

  /* Warnings */
  .warnings-section {
    margin-top: 16px;
    padding-top: 16px;
    border-top: 1px solid rgba(0, 0, 0, 0.08);
  }

  .warnings-title {
    font-size: 14px;
    font-weight: 600;
    color: #ff9500;
    margin-bottom: 10px;
  }

  .warning-item {
    font-size: 14px;
    color: #8e8e93;
    padding: 8px 0;
    line-height: 1.4;
  }

  /* Footer */
  .ios-footer {
    text-align: center;
    margin-top: 32px;
    animation: fadeIn 0.6s ease-out;
    animation-delay: 0.3s;
    animation-fill-mode: both;
  }

  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.3);
    backdrop-filter: blur(2px);
    z-index: 50;
  }

  .ios-modal {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(420px, 90vw);
    background: rgba(255, 255, 255, 0.95);
    border-radius: 20px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.18);
    border: 1px solid rgba(255, 255, 255, 0.6);
    z-index: 60;
    padding: 20px;
  }

  .ios-modal-header {
    font-size: 18px;
    font-weight: 700;
    color: #1c1c1e;
    margin-bottom: 12px;
  }

  .ios-modal-body {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .modal-row {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    font-size: 14px;
  }

  .modal-label {
    color: #8e8e93;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.4px;
  }

  .modal-value {
    color: #1c1c1e;
    font-weight: 600;
  }

  .modal-warning {
    background: #fff3cd;
    color: #8a6d3b;
    padding: 10px 12px;
    border-radius: 12px;
    font-size: 13px;
  }

  .modal-destination .file-selector {
    margin-top: 4px;
  }

  .ios-modal-actions {
    margin-top: 16px;
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }

  .ios-modal-btn {
    border: none;
    border-radius: 12px;
    padding: 10px 16px;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
  }

  .ios-modal-btn.ghost {
    background: #f2f2f7;
    color: #1c1c1e;
  }

  .ios-modal-btn.primary {
    background: linear-gradient(135deg, #007aff, #5856d6);
    color: white;
  }

  .footer-text {
    font-size: 13px;
    color: #8e8e93;
    font-weight: 400;
  }

  /* Responsive */
  @media (max-width: 500px) {
    main {
      padding: 12px;
    }

    .header-title {
      font-size: 36px;
    }

    .content-card {
      padding: 20px;
    }
  }
</style>
