declare global {
  namespace App {
    interface PageState {
      /** Headset view in fullscreen (shallow route, so Back leaves fullscreen). */
      fullscreen?: boolean;
    }
  }

  interface Window {
    /** Native bridge from MainActivity.kt (Android only). */
    FrameMateAndroid?: {
      setFullscreen(enabled: boolean): void;
      openUrl(url: string): void;
      /** False on Android 17+ until "Nearby devices" (local network) is granted. */
      localNetworkAllowed?(): boolean;
      openAppSettings?(): void;
    };
  }
}

export {};
