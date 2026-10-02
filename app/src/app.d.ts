// See https://svelte.dev/docs/kit/types#app.d.ts
declare global {
  namespace App {
    interface PageState {
      /** Headset view in fullscreen (shallow route, so Back leaves fullscreen). */
      fullscreen?: boolean;
    }
  }

  interface Window {
    /** Native bridge from MainActivity.kt (Android only). */
    FrameMateAndroid?: { setFullscreen(enabled: boolean): void };
  }
}

export {};
