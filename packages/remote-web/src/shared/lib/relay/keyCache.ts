import type { PairedRelayHost } from "@/shared/lib/relayPairingStorage";
import { subscribeRelayPairingChanges } from "@/shared/lib/relayPairingStorage";
import {
  base64ToBytes,
  parseEd25519PrivateKeyFromJwk,
} from "@/shared/lib/relayCrypto";

const signingKeyCache = new Map<string, Uint8Array>();
const serverVerifyKeyCache = new Map<string, Uint8Array>();

subscribeRelayPairingChanges(({ hostId }) => {
  clearRelayHostCryptoCaches(hostId);
});

export async function getSigningKey(
  pairedHost: PairedRelayHost,
): Promise<Uint8Array> {
  const signingSessionId = pairedHost.signing_session_id;
  if (!signingSessionId) {
    throw new Error("Missing signing session for paired host.");
  }

  const cacheKey = pairedHost.host_id;
  const cachedKey = signingKeyCache.get(cacheKey);
  if (cachedKey) {
    return cachedKey;
  }

  const privateKey = parseEd25519PrivateKeyFromJwk(pairedHost.private_key_jwk);
  signingKeyCache.set(cacheKey, privateKey);
  return privateKey;
}

export async function getServerVerifyKey(
  pairedHost: PairedRelayHost,
): Promise<Uint8Array> {
  const signingSessionId = pairedHost.signing_session_id;
  if (!signingSessionId) {
    throw new Error("Missing signing session for paired host.");
  }

  const cacheKey = pairedHost.host_id;
  const cachedKey = serverVerifyKeyCache.get(cacheKey);
  if (cachedKey) {
    return cachedKey;
  }

  const serverPublicKeyB64 = pairedHost.server_public_key_b64;
  if (!serverPublicKeyB64) {
    throw new Error("Missing server signing key for paired host.");
  }

  const publicKey = base64ToBytes(serverPublicKeyB64);
  serverVerifyKeyCache.set(cacheKey, publicKey);
  return publicKey;
}

export function clearRelayHostCryptoCaches(hostId: string): void {
  signingKeyCache.delete(hostId);
  serverVerifyKeyCache.delete(hostId);
}
