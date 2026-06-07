// Ambient declarations for Cortex.

// Image assets imported as URLs by Vite (e.g. the logo).
declare module "*.png" {
  const src: string;
  export default src;
}
declare module "*.svg" {
  const src: string;
  export default src;
}
declare module "*.webp" {
  const src: string;
  export default src;
}

declare global {
  interface Window {
    // Set by modal/session components (diff review, flashcards, quiz) to claim
    // the keyboard so the global Helix engine stays out of their way.
    __cortexModalOpen?: boolean;
  }
}

export {};
