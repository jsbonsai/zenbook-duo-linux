import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

type DesktopSettings = {
  keepDualOnUsb: boolean; typingGuard: boolean; typingDelayMs: number; hudEnabled: boolean; terminal: string;
  f7Action: string; f8Action: string; f12Action: string;
  upperWallpaper: string; lowerWallpaper: string; syncWallpapers: boolean;
};
const actions = [
  ["none", "Pass through / no app action"], ["show_controls", "Open Duo Control"], ["codex", "Open Codex terminal"],
  ["emoji", "Emoji picker"], ["cycle_layout", "Stacked ↔ side by side"], ["swap_windows", "Swap windows between screens"],
  ["swap_positions", "Swap desktop positions"], ["mic_mute", "Toggle microphone mute"], ["toggle_touchpad", "Toggle touchpad"],
];
function WallpaperPreview({ path }: { path: string }) {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false; let objectUrl: string | null = null;
    setUrl(null);
    if (path) void invoke<number[]>("desktop_wallpaper_preview", { path }).then(bytes => {
      if (cancelled) return;
      const extension = path.split(".").at(-1)?.toLowerCase();
      const mime = extension === "jpg" || extension === "jpeg" ? "image/jpeg" : `image/${extension}`;
      objectUrl = URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: mime })); setUrl(objectUrl);
    }).catch(() => { if (!cancelled) setUrl(null); });
    return () => { cancelled = true; if (objectUrl) URL.revokeObjectURL(objectUrl); };
  }, [path]);
  return <div className="flex aspect-video items-center justify-center overflow-hidden rounded-lg bg-muted/50">
    {url ? <img className="h-full w-full object-cover" src={url} alt="Wallpaper preview" /> : <span className="text-xs text-muted-foreground">{path ? "Preview unavailable" : "Keep current wallpaper"}</span>}
  </div>;
}
export default function Desktop() {
  const [settings, setSettings] = useState<DesktopSettings | null>(null);
  const [busy, setBusy] = useState(false); const [error, setError] = useState<string | null>(null);
  useEffect(() => { void invoke<DesktopSettings>("load_desktop_settings").then(setSettings).catch(e => setError(String(e))); }, []);
  const update = (patch: Partial<DesktopSettings>) => setSettings(s => s && ({ ...s, ...patch }));
  const save = async () => {
    if (!settings) return; setBusy(true);
    try { await invoke("save_desktop_settings", { settings }); setError(null); toast.success("Desktop preferences saved"); }
    catch (e) { setError(String(e)); toast.error(String(e)); } finally { setBusy(false); }
  };
  const setTypingGuard = async (typingGuard: boolean) => {
    const previous = settings?.typingGuard; update({ typingGuard }); setBusy(true);
    try {
      const saved = await invoke<DesktopSettings>("load_desktop_settings");
      await invoke("save_desktop_settings", { settings: { ...saved, typingGuard } });
      setError(null); toast.success(typingGuard ? "Typing protection enabled" : "Typing protection disabled");
    } catch (e) { update({ typingGuard: previous }); setError(String(e)); toast.error(String(e)); }
    finally { setBusy(false); }
  };
  const choose = async (key: "upperWallpaper" | "lowerWallpaper") => {
    setBusy(true);
    try { const path = await invoke<string | null>("choose_desktop_wallpaper"); if (path) update({ [key]: path }); }
    catch (e) { setError(String(e)); } finally { setBusy(false); }
  };
  const test = async (action: string) => {
    setBusy(true);
    try { await invoke("desktop_action", { action }); setError(null); }
    catch (e) { setError(String(e)); toast.error(String(e)); } finally { setBusy(false); }
  };
  if (!settings) return <div>{error || "Loading desktop preferences…"}</div>;
  return <div className="max-w-4xl space-y-6">
    <div><h1 className="text-xl font-semibold tracking-tight">Desktop & keyboard</h1><p className="mt-1 text-sm text-muted-foreground">Make the two screens and keyboard work together.</p></div>
    {error && <div role="alert" className="rounded-lg border border-destructive/30 bg-destructive/10 p-3 text-sm">{error}</div>}
    <section className="space-y-4 rounded-xl border p-5">
      <h2 className="font-medium">Palm rejection while typing</h2>
      <div className="flex items-center justify-between gap-5"><div><Label htmlFor="typing-guard">Suppress accidental taps</Label><p className="mt-1 text-sm text-muted-foreground">Temporarily turn off tap-to-click after typing. Pointer movement and physical clicks remain available.</p></div><Switch id="typing-guard" checked={settings.typingGuard} onCheckedChange={typingGuard => void setTypingGuard(typingGuard)} disabled={busy}/></div>
      <div className="flex items-center gap-4"><Label htmlFor="typing-delay">Resume taps after</Label><Select value={String(settings.typingDelayMs)} onValueChange={value => update({ typingDelayMs: Number(value) })}><SelectTrigger id="typing-delay" className="w-44"><SelectValue/></SelectTrigger><SelectContent>{[400, 750, 1000, 1500].map(ms => <SelectItem key={ms} value={String(ms)}>{ms} ms</SelectItem>)}</SelectContent></Select></div>
      <p className="text-xs text-muted-foreground">The toggle saves automatically. Applies to the Duo keyboard in USB and Bluetooth modes.</p>
      <details className="rounded-lg border bg-muted/30 p-3 text-sm">
        <summary className="cursor-pointer font-medium">How typing protection works</summary>
        <div className="mt-3 space-y-2 text-muted-foreground">
          <p>The Duo keyboard’s touchpad does not expose native disable-while-typing support. We listen for typing key presses and briefly turn off tap-to-click, extending the pause as you keep typing.</p>
          <p>After the selected delay, your previous tap setting returns. Turning protection off also restores it. Pointer movement, scrolling and physical clicks are left alone.</p>
          <p>Only the timing of key presses is used. Typed text is never recorded or saved. This prevents accidental taps; it does not lock the pointer or classify your palm.</p>
        </div>
      </details>
    </section>
    <section className="space-y-4 rounded-xl border p-5">
      <h2 className="font-medium">Wallpapers</h2>
      <div className="grid gap-4 sm:grid-cols-2">{(["upperWallpaper", "lowerWallpaper"] as const).map((key, i) => <div key={key} className="space-y-2"><Label>{i === 0 ? "Upper screen" : "Lower screen"}</Label><WallpaperPreview path={key === "lowerWallpaper" && settings.syncWallpapers ? settings.upperWallpaper : settings[key]}/><div className="flex gap-2"><Button variant="outline" size="sm" disabled={busy || (i === 1 && settings.syncWallpapers)} onClick={() => void choose(key)}>Choose image</Button><Button variant="ghost" size="sm" disabled={busy} onClick={() => update({ [key]: "" })}>Stop managing</Button></div><p className="truncate text-xs text-muted-foreground" title={settings[key]}>{settings[key]?.split("/").at(-1) || "Current desktop wallpaper"}</p></div>)}</div>
      <div className="flex items-center gap-3"><Switch id="sync-wallpaper" checked={settings.syncWallpapers} onCheckedChange={syncWallpapers => update({ syncWallpapers })}/><Label htmlFor="sync-wallpaper">Use the same wallpaper on both screens</Label></div>
      <p className="text-xs text-muted-foreground">Choose images, then save. Each image follows its physical panel through layout changes and docking.</p>
    </section>
    <section className="space-y-4 rounded-xl border p-5">
      <h2 className="font-medium">Keyboard actions</h2>
      <div className="flex items-center gap-3"><Switch id="usb-charge" checked={settings.keepDualOnUsb} onCheckedChange={keepDualOnUsb => update({ keepDualOnUsb })}/><Label htmlFor="usb-charge">Keep both screens on when using USB</Label></div>
      <p className="text-xs text-muted-foreground">Enable when charging the keyboard with a cable. USB and the keyboard dock report the same device, so this also keeps the lower screen on when physically docked. Save before reconnecting the cable.</p>
      {(["f7Action", "f8Action", "f12Action"] as const).map((key, i) => <div key={key} className="flex items-center gap-4"><Label className="w-12" htmlFor={key}>{["F7", "F8", "F12"][i]}</Label><Select value={settings[key]} onValueChange={value => update({ [key]: value })}><SelectTrigger id={key} className="flex-1"><SelectValue/></SelectTrigger><SelectContent>{actions.map(([value, label]) => <SelectItem key={value} value={value}>{label}</SelectItem>)}</SelectContent></Select><Button variant="outline" disabled={busy || settings[key] === "none"} onClick={() => void test(settings[key])}>Try</Button></div>)}
      <div className="flex items-center gap-4"><Label className="w-12">Terminal</Label><Select value={settings.terminal} onValueChange={terminal => update({ terminal })}><SelectTrigger className="w-48"><SelectValue/></SelectTrigger><SelectContent><SelectItem value="konsole">Konsole</SelectItem><SelectItem value="alacritty">Alacritty</SelectItem></SelectContent></Select></div>
      <p className="text-xs text-muted-foreground">USB media mode: F9 toggles touchpad, F10 toggles mic mute, F11 opens emoji. Fn-lock affects which events the keyboard sends. Bluetooth vendor keys depend on keyboard firmware.</p>
      <div className="flex items-center gap-3"><Switch id="hud" checked={settings.hudEnabled} onCheckedChange={hudEnabled => update({ hudEnabled })}/><Label htmlFor="hud">Show keyboard lighting and desktop action feedback</Label></div>
      <p className="text-xs text-muted-foreground">Volume, mute and brightness use the desktop’s native HUD. Extra actions use the same style.</p>
    </section>
    <section className="rounded-xl border p-5"><h2 className="font-medium">Global shortcuts</h2><div className="mt-3 grid gap-2 text-sm"><p><kbd className="rounded bg-muted px-2 py-1">Meta+Shift+D</kbd> Open Duo Control</p><p><kbd className="rounded bg-muted px-2 py-1">Meta+Shift+C</kbd> Open Codex terminal</p><p><kbd className="rounded bg-muted px-2 py-1">Meta+Shift+S</kbd> Swap windows between screens</p></div><p className="mt-3 text-xs text-muted-foreground">Change these bindings in KDE System Settings → Keyboard → Shortcuts.</p></section>
    <Button disabled={busy} onClick={() => void save()}>{busy ? "Working…" : "Save & apply"}</Button>
  </div>;
}
