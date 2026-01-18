const STORAGE_KEY = "photo-ai-api-key";

function toBase64(bytes) {
  let binary = "";
  const len = bytes.byteLength;
  const view = new Uint8Array(bytes);
  for (let i = 0; i < len; i += 1) {
    binary += String.fromCharCode(view[i]);
  }
  return btoa(binary);
}

function fromBase64(base64) {
  const binary = atob(base64);
  const len = binary.length;
  const bytes = new Uint8Array(len);
  for (let i = 0; i < len; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

async function deriveKey(passphrase, salt) {
  const enc = new TextEncoder();
  const keyMaterial = await crypto.subtle.importKey(
    "raw",
    enc.encode(passphrase),
    "PBKDF2",
    false,
    ["deriveKey"]
  );
  return crypto.subtle.deriveKey(
    {
      name: "PBKDF2",
      salt,
      iterations: 100000,
      hash: "SHA-256",
    },
    keyMaterial,
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt", "decrypt"]
  );
}

export async function encryptApiKey(apiKey, passphrase) {
  if (!apiKey || !passphrase) {
    throw new Error("apiKey/passphrase missing");
  }

  const enc = new TextEncoder();
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const key = await deriveKey(passphrase, salt);
  const ciphertext = await crypto.subtle.encrypt(
    { name: "AES-GCM", iv },
    key,
    enc.encode(apiKey)
  );

  const payload = {
    salt: toBase64(salt),
    iv: toBase64(iv),
    data: toBase64(ciphertext),
  };
  localStorage.setItem(STORAGE_KEY, JSON.stringify(payload));
  return true;
}

export async function decryptApiKey(passphrase) {
  if (!passphrase) {
    throw new Error("passphrase missing");
  }

  const raw = localStorage.getItem(STORAGE_KEY);
  if (!raw) {
    throw new Error("stored key not found");
  }

  const payload = JSON.parse(raw);
  const salt = fromBase64(payload.salt);
  const iv = fromBase64(payload.iv);
  const data = fromBase64(payload.data);

  const key = await deriveKey(passphrase, salt);
  const plaintext = await crypto.subtle.decrypt(
    { name: "AES-GCM", iv },
    key,
    data
  );

  const dec = new TextDecoder();
  return dec.decode(plaintext);
}

export function clearApiKey() {
  localStorage.removeItem(STORAGE_KEY);
}
