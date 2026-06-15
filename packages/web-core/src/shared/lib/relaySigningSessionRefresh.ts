import type { RelaySigningSessionRefreshPayload } from '@/shared/lib/relayBackendApi';
import {
  bytesToBase64,
  ed25519Sign,
  parseEd25519PrivateKeyFromJwk,
  secureRandomUuid,
} from '@/shared/lib/relayCrypto';

const TEXT_ENCODER = new TextEncoder();

export function buildRelaySigningSessionRefreshMessage(
  timestamp: number,
  nonce: string,
  clientId: string
): string {
  return `v1|refresh|${timestamp}|${nonce}|${clientId}`;
}

export async function buildRelaySigningSessionRefreshPayload(
  clientId: string,
  privateKeyJwk: JsonWebKey
): Promise<RelaySigningSessionRefreshPayload> {
  const timestamp = Math.floor(Date.now() / 1000);
  const nonce = secureRandomUuid();
  const message = buildRelaySigningSessionRefreshMessage(
    timestamp,
    nonce,
    clientId
  );
  const privateKey = parseEd25519PrivateKeyFromJwk(privateKeyJwk);
  const signature = ed25519Sign(TEXT_ENCODER.encode(message), privateKey);

  return {
    client_id: clientId,
    timestamp,
    nonce,
    signature_b64: bytesToBase64(signature),
  };
}
