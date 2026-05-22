import { WebAudioBackend } from "vm-rust";

declare global {
  interface Window {
    getAudioContext: () => AudioContext;
  }
}

let globalAudioContext: AudioContext | null = null;
let audioBackend: WebAudioBackend | null = null;
let isAudioInitialized = false;
let areGestureListenersInstalled = false;

const audioGestureEvents = ["pointerdown", "mousedown", "click", "keydown", "touchstart"];

function removeGestureListeners(listener: EventListener): void {
  for (const eventName of audioGestureEvents) {
    document.removeEventListener(eventName, listener, true);
  }
  areGestureListenersInstalled = false;
}

export async function resumeAudioContext(): Promise<boolean> {
  if (!globalAudioContext) {
    return false;
  }

  try {
    audioBackend?.resume_context();

    if (globalAudioContext.state !== "running") {
      console.log(`Resuming AudioContext from state: ${globalAudioContext.state}`);
      await globalAudioContext.resume();
    }

    return globalAudioContext.state === "running";
  } catch (err) {
    console.error("Failed to resume AudioContext:", err);
    return false;
  }
}

/**
 * Initialize the global AudioContext.
 * Browsers keep it suspended until a user gesture resumes it.
 */
export function initAudioContext(): AudioContext {
  if (!globalAudioContext) {
    globalAudioContext = new (window.AudioContext || (window as any).webkitAudioContext)();
    console.log("AudioContext created:", globalAudioContext.state);

    window.getAudioContext = () => {
      if (!globalAudioContext) throw new Error("AudioContext not initialized");
      return globalAudioContext;
    };

    setupAudioOnUserGesture();
  }
  return globalAudioContext;
}

/**
 * Initialize the WebAudioBackend.
 * This requires WASM to be initialized first.
 * Returns true if initialization was successful.
 */
export function initAudioBackend(): boolean {
  if (isAudioInitialized) {
    void resumeAudioContext();
    return true;
  }

  try {
    initAudioContext();
    audioBackend = new WebAudioBackend();
    console.log("WebAudioBackend created");

    isAudioInitialized = true;
    void resumeAudioContext();
    return true;
  } catch (err) {
    console.error("Failed to create WebAudioBackend:", err);
    return false;
  }
}

/**
 * Setup audio initialization on user gestures.
 * This handles browser autoplay policy for contexts created during VM startup.
 */
export function setupAudioOnUserGesture(): void {
  if (areGestureListenersInstalled) {
    return;
  }

  const initAudio: EventListener = () => {
    if (!initAudioBackend()) {
      return;
    }

    void resumeAudioContext().then((isRunning) => {
      if (isRunning) {
        removeGestureListeners(initAudio);
      }
    });
  };

  areGestureListenersInstalled = true;
  for (const eventName of audioGestureEvents) {
    document.addEventListener(eventName, initAudio, {
      capture: true,
      passive: true,
    });
  }
}

/**
 * Get the current audio initialization status.
 */
export function isAudioReady(): boolean {
  return isAudioInitialized;
}

/**
 * Get the audio backend instance (may be null if not initialized).
 */
export function getAudioBackend(): WebAudioBackend | null {
  return audioBackend;
}
