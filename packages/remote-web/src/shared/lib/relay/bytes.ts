import {
  base64ToBytes,
  bytesToBase64,
  sha256Bytes,
} from "@/shared/lib/relayCrypto";

export const TEXT_ENCODER = new TextEncoder();
export const TEXT_DECODER = new TextDecoder();

export { base64ToBytes, bytesToBase64 };

export function toArrayBuffer(bytes: Uint8Array): ArrayBuffer {
  return bytes.buffer.slice(
    bytes.byteOffset,
    bytes.byteOffset + bytes.byteLength,
  ) as ArrayBuffer;
}

export async function sha256Base64(bytes: Uint8Array): Promise<string> {
  return bytesToBase64(sha256Bytes(bytes));
}
