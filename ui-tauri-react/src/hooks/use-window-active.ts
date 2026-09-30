import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

// Native focus handles tray hiding/minimizing even when WebKit visibility stays visible.
export function useWindowActive() {
  const [active, setActive] = useState(false);
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    let focus = false;
    let revision = 0;
    const publish = () => {
      if (!cancelled) setActive(focus && document.visibilityState !== "hidden");
    };
    const visibilityChanged = () => publish();
    document.addEventListener("visibilitychange", visibilityChanged);
    const nativeWindow = getCurrentWindow();
    void nativeWindow.onFocusChanged(({ payload }) => {
      revision += 1;
      focus = payload;
      publish();
    }).then(async stop => {
      if (cancelled) { stop(); return; }
      unlisten = stop;
      const beforeRead = revision;
      const focused = await nativeWindow.isFocused();
      if (revision === beforeRead) { focus = focused; publish(); }
    }).catch(error => {
      console.error("Could not track dashboard window focus", error);
      focus = document.hasFocus();
      publish();
    });
    return () => {
      cancelled = true;
      unlisten?.();
      document.removeEventListener("visibilitychange", visibilityChanged);
    };
  }, []);
  return active;
}
