import { ed25519 } from '@noble/curves/ed25519';
import { hkdf } from '@noble/hashes/hkdf';
import { hmac } from '@noble/hashes/hmac';
import { sha256 } from '@noble/hashes/sha2';

/** Crypto helpers that work over plain HTTP (no secure-context crypto.subtle). */

export function sha256Bytes(data: Uint8Array): Uint8Array {
  return sha256(data);
}

export function hmacSha256Bytes(key: Uint8Array, data: Uint8Array): Uint8Array {
  return hmac(sha256, key, data);
}

export function hkdfSha256Bytes(
  ikm: Uint8Array,
  salt: Uint8Array,
  info: Uint8Array,
  length: number
): Uint8Array {
  return hkdf(sha256, ikm, salt, info, length);
}

export function bytesToBase64(bytes: Uint8Array): string {
  let binary = '';
  for (const value of bytes) {
    binary += String.fromCharCode(value);
  }
  return btoa(binary);
}

export function base64ToBytes(value: string): Uint8Array {
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

function base64UrlEncode(bytes: Uint8Array): string {
  return bytesToBase64(bytes)
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=+$/, '');
}

function base64UrlToBytes(value: string): Uint8Array {
  const base64 = value.replace(/-/g, '+').replace(/_/g, '/');
  const padded = base64 + '='.repeat((4 - (base64.length % 4)) % 4);
  return base64ToBytes(padded);
}

export function parseEd25519PrivateKeyFromJwk(jwk: JsonWebKey): Uint8Array {
  if (!jwk.d) {
    throw new Error('Missing Ed25519 private key.');
  }
  return base64UrlToBytes(jwk.d);
}

export function generateEd25519RelayKeyPair(): {
  privateKeyJwk: JsonWebKey;
  publicKeyBytes: Uint8Array;
  publicKeyB64: string;
} {
  const privateKey = ed25519.utils.randomPrivateKey();
  const publicKeyBytes = ed25519.getPublicKey(privateKey);
  const privateKeyJwk: JsonWebKey = {
    kty: 'OKP',
    crv: 'Ed25519',
    d: base64UrlEncode(privateKey),
    x: base64UrlEncode(publicKeyBytes),
  };

  return {
    privateKeyJwk,
    publicKeyBytes,
    publicKeyB64: bytesToBase64(publicKeyBytes),
  };
}

export function ed25519Sign(
  message: Uint8Array,
  privateKey: Uint8Array
): Uint8Array {
  return ed25519.sign(message, privateKey);
}

export function ed25519Verify(
  signature: Uint8Array,
  message: Uint8Array,
  publicKey: Uint8Array
): boolean {
  return ed25519.verify(signature, message, publicKey);
}

export function secureRandomUuid(): string {
  if (
    typeof crypto !== 'undefined' &&
    typeof crypto.randomUUID === 'function'
  ) {
    return crypto.randomUUID();
  }

  const bytes = new Uint8Array(16);
  if (
    typeof crypto !== 'undefined' &&
    typeof crypto.getRandomValues === 'function'
  ) {
    crypto.getRandomValues(bytes);
  } else {
    for (let index = 0; index < 16; index += 1) {
      bytes[index] = Math.floor(Math.random() * 256);
    }
  }

  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = [...bytes].map((byte) => byte.toString(16).padStart(2, '0'));
  return (
    hex.slice(0, 4).join('') +
    '-' +
    hex.slice(4, 6).join('') +
    '-' +
    hex.slice(6, 8).join('') +
    '-' +
    hex.slice(8, 10).join('') +
    '-' +
    hex.slice(10, 16).join('')
  );
}
