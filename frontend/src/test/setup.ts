import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

class ResizeObserverMock {
  observe() {}

  unobserve() {}

  disconnect() {}
}

globalThis.ResizeObserver = ResizeObserverMock;
HTMLElement.prototype.hasPointerCapture = () => false;
HTMLElement.prototype.setPointerCapture = () => undefined;
HTMLElement.prototype.releasePointerCapture = () => undefined;
HTMLElement.prototype.scrollIntoView = () => undefined;

const storageEntries = new Map<string, string>();
const localStorageMock: Storage = {
  get length() {
    return storageEntries.size;
  },
  clear() {
    storageEntries.clear();
  },
  getItem(key) {
    return storageEntries.get(key) ?? null;
  },
  key(index) {
    return Array.from(storageEntries.keys())[index] ?? null;
  },
  removeItem(key) {
    storageEntries.delete(key);
  },
  setItem(key, value) {
    storageEntries.set(key, String(value));
  },
};

if (
  typeof window !== "undefined" &&
  (typeof window.localStorage?.getItem !== "function" ||
    typeof window.localStorage?.setItem !== "function" ||
    typeof window.localStorage?.removeItem !== "function" ||
    typeof window.localStorage?.clear !== "function")
) {
  Object.defineProperty(window, "localStorage", {
    configurable: true,
    value: localStorageMock,
  });
}

if (typeof window !== "undefined" && typeof window.matchMedia !== "function") {
  window.matchMedia = ((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener: () => undefined,
    addListener: () => undefined,
    dispatchEvent: () => false,
    removeEventListener: () => undefined,
    removeListener: () => undefined,
  })) as typeof window.matchMedia;
}

afterEach(() => {
  if (typeof window !== "undefined" && typeof window.localStorage?.clear === "function") {
    window.localStorage.clear();
  }
  cleanup();
});
