export function isLoopbackHostname(hostname: string): boolean {
  return (
    hostname === 'localhost' ||
    hostname === '127.0.0.1' ||
    hostname === '[::1]' ||
    hostname === '::1'
  );
}

/** Dev/self-host: baked relay URL may use LAN IP while the page is opened via Tailscale IP/DNS. */
export function isSelfHostedDevHostname(hostname: string): boolean {
  if (isLoopbackHostname(hostname)) {
    return true;
  }
  if (/^100\.(6[4-9]|[7-9]\d|1[01]\d|12[0-7])\./.test(hostname)) {
    return true;
  }
  if (hostname.endsWith('.ts.net')) {
    return true;
  }
  if (/^(10\.|192\.168\.|172\.(1[6-9]|2\d|3[01])\.)/.test(hostname)) {
    return true;
  }
  return false;
}

export interface RelayApiBaseForPageInput {
  bakedRelayApiBase: string;
  pageProtocol: string;
  pageHostname: string;
  relayPort: string;
  tailscaleRelayHttpsPort: string;
}

/**
 * Relay origin for the current page.
 *
 * An https `*.ts.net` page (mobile / data front door, e.g. :13444) must call
 * the Tailscale relay HTTPS door (:18443). Port 18082 is plaintext HTTP; using
 * it as `https://…:18082` fails TLS (`Failed to fetch`) and the pairing POST
 * never lands. localhost h2, http pages, and LAN pages keep `relayPort`.
 */
export function resolveRelayApiBaseForPage(
  input: RelayApiBaseForPageInput
): string {
  const {
    bakedRelayApiBase,
    pageProtocol: protocol,
    pageHostname: hostname,
    relayPort,
    tailscaleRelayHttpsPort,
  } = input;

  const tailscaleHttpsPage =
    protocol === 'https:' && hostname.endsWith('.ts.net');
  const pageRelayPort = tailscaleHttpsPage
    ? tailscaleRelayHttpsPort
    : relayPort;

  if (protocol === 'https:' && bakedRelayApiBase.startsWith('https:')) {
    try {
      const baked = new URL(bakedRelayApiBase);
      if (
        baked.hostname !== hostname &&
        (hostname.endsWith('.ts.net') || isSelfHostedDevHostname(hostname))
      ) {
        return `${protocol}//${hostname}:${pageRelayPort}`;
      }
      if (tailscaleHttpsPage && baked.port !== tailscaleRelayHttpsPort) {
        return `${protocol}//${hostname}:${tailscaleRelayHttpsPort}`;
      }
    } catch {
      if (tailscaleHttpsPage) {
        return `${protocol}//${hostname}:${tailscaleRelayHttpsPort}`;
      }
    }
    return bakedRelayApiBase;
  }

  const liveRelayOrigin = `${protocol}//${hostname}:${pageRelayPort}`;

  if (bakedRelayApiBase) {
    try {
      const baked = new URL(bakedRelayApiBase);
      // Worker desktops open on localhost but point at another machine's
      // relay (e.g. Mac Tailscale). Do not remap that remote relay onto
      // localhost — nothing is listening there.
      if (isLoopbackHostname(hostname) && !isLoopbackHostname(baked.hostname)) {
        return bakedRelayApiBase;
      }
      if (
        baked.hostname === hostname ||
        baked.hostname === 'localhost' ||
        isSelfHostedDevHostname(baked.hostname)
      ) {
        return liveRelayOrigin;
      }
      return bakedRelayApiBase;
    } catch {
      return liveRelayOrigin;
    }
  }

  return liveRelayOrigin;
}
