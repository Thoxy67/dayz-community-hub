/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** "1" in the screenshot build: the pretend backend is installed (lib/ipc/mock.ts). */
  readonly VITE_MOCK?: string;
}
