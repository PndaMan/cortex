// Ambient declarations for Cortex.

declare global {
  interface Window {
    // Set by modal/session components (diff review, flashcards, quiz) to claim
    // the keyboard so the global Helix engine stays out of their way.
    __cortexModalOpen?: boolean;
  }
}

export {};
