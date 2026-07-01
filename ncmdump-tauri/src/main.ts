import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type Provider = "netease" | "bilibili";
type DownloadProgressPhase =
  | "queued"
  | "preparing"
  | "resolving_meta"
  | "downloading"
  | "post_processing"
  | "embedding_cover"
  | "saving_lyrics"
  | "refreshing_library"
  | "completed"
  | "failed";

type ProviderStatus = {
  provider: Provider;
  is_logged_in: boolean;
  session_path: string;
  summary: string;
};

type DownloadResult = {
  provider: Provider;
  saved_path: string;
};

type DownloadProgressSnapshot = {
  job_id: string;
  source: string;
  state: string;
  phase: DownloadProgressPhase;
  percent: number | null;
  message: string;
  detail: string | null;
  filename: string | null;
  warning: string | null;
  error: string | null;
};

type NeteaseSearchItem = {
  id: number;
  name: string;
  artists: string;
  album: string;
};

type BilibiliSearchItem = {
  bvid: string;
  title: string;
  author: string;
  duration: string;
};

const providerCopy: Record<
  Provider,
  {
    title: string;
    openButton: string;
    captureButton: string;
    loginUrl: string;
    intro: string;
    openSuccess: string;
    captureSuccess: string;
    clearSuccess: string;
  }
> = {
  netease: {
    title: "NetEase Cloud Music",
    openButton: "Open NetEase Login",
    captureButton: "Capture MUSIC_U",
    loginUrl: "music.163.com",
    intro:
      "Open the NetEase page in this window, log in, then capture the MUSIC_U cookie.",
    openSuccess:
      "Main webview navigated to music.163.com. Complete the login there, then capture cookies here.",
    captureSuccess:
      "Captured MUSIC_U from the Tauri webview and saved the NetEase session.",
    clearSuccess: "Cleared the saved NetEase session.",
  },
  bilibili: {
    title: "Bilibili",
    openButton: "Open Bilibili Login",
    captureButton: "Capture Bilibili Cookies",
    loginUrl: "www.bilibili.com",
    intro:
      "Open Bilibili in this window, log in there, then capture SESSDATA and the related session cookies.",
    openSuccess:
      "Main webview navigated to bilibili.com. Complete the login there, then capture cookies here.",
    captureSuccess:
      "Captured SESSDATA and related Bilibili cookies from the Tauri webview and saved the session.",
    clearSuccess: "Cleared the saved Bilibili session.",
  },
};

let currentProvider: Provider = "netease";
let titleEl: HTMLElement | null;
let providerHintEl: HTMLElement | null;
let statusValueEl: HTMLElement | null;
let sessionPathEl: HTMLElement | null;
let summaryEl: HTMLElement | null;
let messageEl: HTMLElement | null;
let openButtonEl: HTMLButtonElement | null;
let captureButtonEl: HTMLButtonElement | null;
let bilibiliInputEl: HTMLInputElement | null;
let neteaseTrackIdEl: HTMLInputElement | null;
let downloadPathEl: HTMLElement | null;
let downloadSourceEl: HTMLElement | null;
let downloadStateEl: HTMLElement | null;
let downloadPhaseEl: HTMLElement | null;
let downloadPercentEl: HTMLElement | null;
let downloadMessageEl: HTMLElement | null;
let downloadDetailEl: HTMLElement | null;
let neteaseSearchEl: HTMLInputElement | null;
let bilibiliSearchEl: HTMLInputElement | null;
let neteaseResultsEl: HTMLElement | null;
let bilibiliResultsEl: HTMLElement | null;

function setMessage(message: string, kind: "info" | "error" = "info") {
  if (!messageEl) return;
  messageEl.textContent = message;
  messageEl.dataset.kind = kind;
}

function applyProviderCopy(provider: Provider) {
  const copy = providerCopy[provider];
  if (titleEl)
    titleEl.textContent = `Login to ${copy.title} in the embedded webview`;
  if (providerHintEl) providerHintEl.textContent = copy.intro;
  if (openButtonEl) openButtonEl.textContent = copy.openButton;
  if (captureButtonEl) captureButtonEl.textContent = copy.captureButton;

  document
    .querySelectorAll<HTMLButtonElement>("[data-provider]")
    .forEach((button) => {
      button.dataset.active =
        button.dataset.provider === provider ? "true" : "false";
    });

  document
    .querySelectorAll<HTMLElement>("[data-provider-copy]")
    .forEach((node) => {
      const key = node.dataset.providerCopy;
      if (key === "login-url") {
        node.textContent = copy.loginUrl;
      }
    });
}

function renderStatus(status: ProviderStatus) {
  if (statusValueEl) {
    statusValueEl.textContent = status.is_logged_in
      ? "Logged in"
      : "Not logged in";
  }
  if (sessionPathEl) {
    sessionPathEl.textContent = status.session_path;
  }
  if (summaryEl) {
    summaryEl.textContent = status.summary;
  }
}

function renderDownload(result: DownloadResult) {
  if (downloadPathEl) {
    downloadPathEl.textContent = result.saved_path;
  }
}

function renderProgress(snapshot: DownloadProgressSnapshot) {
  if (downloadSourceEl) downloadSourceEl.textContent = snapshot.source;
  if (downloadStateEl) downloadStateEl.textContent = snapshot.state;
  if (downloadPhaseEl) downloadPhaseEl.textContent = snapshot.phase;
  if (downloadPercentEl)
    downloadPercentEl.textContent =
      snapshot.percent === null ? "-" : `${snapshot.percent}%`;
  if (downloadMessageEl) downloadMessageEl.textContent = snapshot.message;
  if (downloadDetailEl)
    downloadDetailEl.textContent = snapshot.detail ?? snapshot.error ?? "-";
  if (downloadPathEl && snapshot.filename) {
    downloadPathEl.textContent = snapshot.filename;
  }
}

function renderNeteaseResults(items: NeteaseSearchItem[]) {
  if (!neteaseResultsEl) return;
  if (items.length === 0) {
    neteaseResultsEl.innerHTML = `<p class="result-empty">No NetEase results.</p>`;
    return;
  }
  neteaseResultsEl.innerHTML = items
    .map(
      (item) => `
        <article class="result-card">
          <div class="result-copy">
            <strong>${item.name}</strong>
            <span>${item.artists}</span>
            <span>${item.album}</span>
            <span>ID: ${item.id}</span>
          </div>
          <button type="button" class="secondary" data-download-netease-id="${item.id}">Download</button>
        </article>
      `,
    )
    .join("");

  neteaseResultsEl
    .querySelectorAll<HTMLButtonElement>("[data-download-netease-id]")
    .forEach((button) => {
      button.addEventListener("click", async () => {
        const trackId = Number(button.dataset.downloadNeteaseId);
        const result = await runAction(
          () => invoke<DownloadResult>("download_netease_track", { trackId }),
          "Downloaded NetEase track.",
        );
        renderDownload(result);
      });
    });
}

function renderBilibiliResults(items: BilibiliSearchItem[]) {
  if (!bilibiliResultsEl) return;
  if (items.length === 0) {
    bilibiliResultsEl.innerHTML = `<p class="result-empty">No Bilibili results.</p>`;
    return;
  }
  bilibiliResultsEl.innerHTML = items
    .map(
      (item) => `
        <article class="result-card">
          <div class="result-copy">
            <strong>${item.title}</strong>
            <span>${item.author}</span>
            <span>${item.duration}</span>
            <span>${item.bvid}</span>
          </div>
          <button type="button" class="secondary" data-download-bilibili-id="${item.bvid}">Download</button>
        </article>
      `,
    )
    .join("");

  bilibiliResultsEl
    .querySelectorAll<HTMLButtonElement>("[data-download-bilibili-id]")
    .forEach((button) => {
      button.addEventListener("click", async () => {
        const input = button.dataset.downloadBilibiliId ?? "";
        const result = await runAction(
          () => invoke<DownloadResult>("download_bilibili_audio", { input }),
          "Downloaded Bilibili audio stream.",
        );
        renderDownload(result);
      });
    });
}

async function refreshStatus(provider = currentProvider) {
  const status = await invoke<ProviderStatus>("login_status", { provider });
  if (provider === currentProvider) {
    renderStatus(status);
  }
  return status;
}

async function runAction<T>(action: () => Promise<T>, successMessage?: string) {
  try {
    const result = await action();
    if (successMessage) {
      setMessage(successMessage, "info");
    }
    return result;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    setMessage(message, "error");
    throw error;
  }
}

async function switchProvider(provider: Provider) {
  currentProvider = provider;
  applyProviderCopy(provider);
  try {
    const status = await refreshStatus(provider);
    renderStatus(status);
    setMessage(providerCopy[provider].intro, "info");
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    setMessage(message, "error");
  }
}

window.addEventListener("DOMContentLoaded", async () => {
  titleEl = document.querySelector("#provider-title");
  providerHintEl = document.querySelector("#provider-hint");
  statusValueEl = document.querySelector("#status-value");
  sessionPathEl = document.querySelector("#session-path");
  summaryEl = document.querySelector("#summary");
  messageEl = document.querySelector("#message");
  openButtonEl = document.querySelector("#open-login");
  captureButtonEl = document.querySelector("#capture-login");
  bilibiliInputEl = document.querySelector("#bilibili-input");
  neteaseTrackIdEl = document.querySelector("#netease-track-id");
  downloadPathEl = document.querySelector("#download-path");
  downloadSourceEl = document.querySelector("#download-source");
  downloadStateEl = document.querySelector("#download-state");
  downloadPhaseEl = document.querySelector("#download-phase");
  downloadPercentEl = document.querySelector("#download-percent");
  downloadMessageEl = document.querySelector("#download-message");
  downloadDetailEl = document.querySelector("#download-detail");
  neteaseSearchEl = document.querySelector("#netease-search");
  bilibiliSearchEl = document.querySelector("#bilibili-search");
  neteaseResultsEl = document.querySelector("#netease-results");
  bilibiliResultsEl = document.querySelector("#bilibili-results");

  await listen<DownloadProgressSnapshot>("download-progress", (event) => {
    renderProgress(event.payload);
  });

  document
    .querySelectorAll<HTMLButtonElement>("[data-provider]")
    .forEach((button) => {
      button.addEventListener("click", async () => {
        await switchProvider(button.dataset.provider as Provider);
      });
    });

  openButtonEl?.addEventListener("click", async () => {
    const provider = currentProvider;
    await runAction(
      () => invoke("open_login", { provider }),
      providerCopy[provider].openSuccess,
    );
  });

  captureButtonEl?.addEventListener("click", async () => {
    const provider = currentProvider;
    const status = await runAction(
      () => invoke<ProviderStatus>("capture_login_cookie", { provider }),
      providerCopy[provider].captureSuccess,
    );
    renderStatus(status);
  });

  document
    .querySelector<HTMLButtonElement>("#refresh-status")
    ?.addEventListener("click", async () => {
      const status = await runAction(
        () => refreshStatus(),
        "Session status refreshed.",
      );
      renderStatus(status);
    });

  document
    .querySelector<HTMLButtonElement>("#logout")
    ?.addEventListener("click", async () => {
      const provider = currentProvider;
      const status = await runAction(
        () => invoke<ProviderStatus>("logout", { provider }),
        providerCopy[provider].clearSuccess,
      );
      renderStatus(status);
    });

  document
    .querySelector<HTMLButtonElement>("#download-bilibili")
    ?.addEventListener("click", async () => {
      const input = bilibiliInputEl?.value.trim() ?? "";
      if (!input) {
        setMessage("Enter a Bilibili URL or BV ID.", "error");
        return;
      }
      const result = await runAction(
        () => invoke<DownloadResult>("download_bilibili_audio", { input }),
        "Downloaded Bilibili audio stream.",
      );
      renderDownload(result);
    });

  document
    .querySelector<HTMLButtonElement>("#download-netease")
    ?.addEventListener("click", async () => {
      const trackIdRaw = neteaseTrackIdEl?.value.trim() ?? "";
      const trackId = Number(trackIdRaw);
      if (!Number.isInteger(trackId) || trackId <= 0) {
        setMessage("Enter a valid NetEase numeric track ID.", "error");
        return;
      }
      const result = await runAction(
        () => invoke<DownloadResult>("download_netease_track", { trackId }),
        "Downloaded NetEase track.",
      );
      renderDownload(result);
    });

  document
    .querySelector<HTMLButtonElement>("#search-netease")
    ?.addEventListener("click", async () => {
      const keyword = neteaseSearchEl?.value.trim() ?? "";
      const results = await runAction(
        () => invoke<NeteaseSearchItem[]>("search_netease_tracks", { keyword }),
        "NetEase search complete.",
      );
      renderNeteaseResults(results);
    });

  document
    .querySelector<HTMLButtonElement>("#search-bilibili")
    ?.addEventListener("click", async () => {
      const keyword = bilibiliSearchEl?.value.trim() ?? "";
      const results = await runAction(
        () =>
          invoke<BilibiliSearchItem[]>("search_bilibili_videos", { keyword }),
        "Bilibili search complete.",
      );
      renderBilibiliResults(results);
    });

  await switchProvider("netease");
});
