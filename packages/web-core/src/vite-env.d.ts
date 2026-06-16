/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_VK_SHARED_API_BASE?: string;
  readonly VITE_RELAY_API_BASE_URL?: string;
  readonly VITE_RELAY_PORT?: string;
  readonly VITE_TAILSCALE_RELAY_HTTPS_PORT?: string;
}

declare const __APP_VERSION__: string;
