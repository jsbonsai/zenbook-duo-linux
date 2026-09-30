import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

type Preferences = { profile: string | null; turboEnabled: boolean | null; chargeLimit: number | null };
type PowerStatus = {
  profile: string | null; availableProfiles: string[]; turboEnabled: boolean | null;
  chargeLimit: number | null; batteryPercent: number | null; batteryStatus: string | null;
  batteryHealth: number | null; batteryCycles: number | null; energyFullWh: number | null;
  energyDesignWh: number | null; cpuTemperature: number | null; fanRpm: number[];
  preferences: Preferences; persistenceError: string | null;
};
type Action = { action: "set_profile"; profile: string } | { action: "set_turbo"; enabled: boolean } | { action: "set_charge_limit"; percent: number };
const profileLabels: Record<string, string> = { "power-saver": "Quiet", balanced: "Balanced", performance: "Performance" };
const value = (n: number | null | undefined, unit = "", digits = 0) => n == null ? "Unavailable" : `${n.toFixed(digits)}${unit}`;

export default function Power() {
  const [status, setStatus] = useState<PowerStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const writeInProgress = useRef(false);
  const readInProgress = useRef(false);
  const alive = useRef(true);
  const refresh = useCallback(async () => {
    if (writeInProgress.current || readInProgress.current) return;
    readInProgress.current = true;
    try {
      const next = await invoke<PowerStatus>("get_power_status");
      if (alive.current && !writeInProgress.current) { setStatus(next); setError(null); }
    } catch (e) { if (alive.current) setError(String(e)); }
    finally { readInProgress.current = false; }
  }, []);
  useEffect(() => {
    alive.current = true;
    void refresh();
    const timer = window.setInterval(() => void refresh(), 3000);
    return () => { alive.current = false; window.clearInterval(timer); };
  }, [refresh]);
  const change = async (action: Action) => {
    if (writeInProgress.current) return;
    writeInProgress.current = true; setBusy(true);
    try {
      const next = await invoke<PowerStatus>("set_power_control", { action });
      if (alive.current) { setStatus(next); setError(null); toast.success("Applied and saved"); }
    } catch (e) {
      if (alive.current) { setError(String(e)); toast.error(String(e)); }
    } finally {
      writeInProgress.current = false;
      if (alive.current) { setBusy(false); void refresh(); }
    }
  };
  const disabled = busy || error !== null;
  return <div className="space-y-5">
    <div>
      <h1 className="text-xl font-semibold tracking-tight">Power & battery</h1>
      <p className="mt-1 text-sm text-muted-foreground">Manage performance, cooling and charging.</p>
    </div>
    {error && <div role="alert" className="rounded-xl border border-destructive/40 p-4 text-sm text-destructive">{error}<Button className="ml-3" variant="outline" size="sm" onClick={() => void refresh()}>Retry</Button></div>}
    {!status ? <p className="text-sm text-muted-foreground">Reading hardware status…</p> : <>
      <div className="glass-card rounded-xl p-5 space-y-5">
        <h2 className="text-sm font-semibold">Performance & cooling</h2>
        <Row label="Firmware profile" description="Quiet favors lower fan speeds. Balanced and Performance allow more cooling and power.">
          <Select value={status.profile ?? ""} onValueChange={profile => void change({ action: "set_profile", profile })} disabled={disabled || !status.availableProfiles.length}>
            <SelectTrigger className="w-44" aria-label="Firmware profile"><SelectValue placeholder="Unavailable" /></SelectTrigger>
            <SelectContent>{status.availableProfiles.map(p => <SelectItem key={p} value={p}>{profileLabels[p] ?? p}</SelectItem>)}</SelectContent>
          </Select>
        </Row>
        <Row label="CPU turbo boost" description="Disable to limit boost frequency. Responsiveness depends on your workload.">
          <div className="flex items-center gap-3"><span className="text-xs text-muted-foreground">{status.turboEnabled == null ? "Unavailable" : status.turboEnabled ? "On" : "Off"}</span><Switch aria-label="CPU turbo boost" checked={status.turboEnabled ?? false} disabled={disabled || status.turboEnabled == null} onCheckedChange={enabled => void change({ action: "set_turbo", enabled })} /></div>
        </Row>
        <p className="text-xs text-muted-foreground">Fan behavior follows the firmware profile. Quiet can reduce noise without reducing CPU temperature. Custom fan curves are not supported in this build.</p>
      </div>
      <div className="glass-card rounded-xl p-5 space-y-5">
        <h2 className="text-sm font-semibold">Battery care</h2>
        <Row label="Charge ceiling" description="80% is a useful everyday compromise. Lowering the ceiling stops charging; it does not force discharge.">
          <Select value={status.chargeLimit == null ? "" : String(status.chargeLimit)} onValueChange={n => void change({ action: "set_charge_limit", percent: Number(n) })} disabled={disabled || status.chargeLimit == null}>
            <SelectTrigger className="w-44" aria-label="Charge ceiling"><SelectValue placeholder="Unavailable" /></SelectTrigger>
            <SelectContent>{Array.from(new Set([60, 70, 75, 80, 90, 100, ...(status.chargeLimit == null ? [] : [status.chargeLimit])])).sort((a,b) => a-b).map(n => <SelectItem key={n} value={String(n)}>{n}%{n === 80 ? " · Recommended" : ""}</SelectItem>)}</SelectContent>
          </Select>
        </Row>
        <p className="text-xs text-muted-foreground">Changed preferences are saved and reapplied at service start and resume. The reported ceiling is a firmware setting, not a guarantee of an exact charge percentage.</p>
        {status.persistenceError && <p role="alert" className="text-sm text-destructive">{status.persistenceError}</p>}
        <p className="text-xs text-muted-foreground">Saved: profile {status.preferences.profile ? profileLabels[status.preferences.profile] ?? status.preferences.profile : "unchanged"}; turbo {status.preferences.turboEnabled == null ? "unchanged" : status.preferences.turboEnabled ? "on" : "off"}; ceiling {status.preferences.chargeLimit == null ? "unchanged" : `${status.preferences.chargeLimit}%`}.</p>
      </div>
      <div className="glass-card rounded-xl p-5">
        <h2 className="mb-4 text-sm font-semibold">Live readings</h2>
        <dl className="grid grid-cols-2 gap-5 sm:grid-cols-3">
          <Metric name="CPU temperature" reading={value(status.cpuTemperature, "°C", 1)} />
          <Metric name="Fans" reading={status.fanRpm.length ? status.fanRpm.map(n => `${n} RPM`).join(" / ") : "Unavailable"} />
          <Metric name="Battery charge" reading={value(status.batteryPercent, "%")} />
          <Metric name="Battery status" reading={status.batteryStatus ?? "Unavailable"} />
          <Metric name="Battery health" reading={value(status.batteryHealth, "%", 1)} />
          <Metric name="Charge cycles" reading={value(status.batteryCycles)} />
          <Metric name="Full capacity" reading={value(status.energyFullWh, " Wh", 2)} />
          <Metric name="Design capacity" reading={value(status.energyDesignWh, " Wh", 2)} />
        </dl>
        <p className="mt-4 text-xs text-muted-foreground">Updates every three seconds. Battery health is the reported full capacity divided by design capacity.</p>
      </div>
    </>}
  </div>;
}
function Row({ label, description, children }: { label: string; description: string; children: React.ReactNode }) {
  return <div className="flex items-center justify-between gap-5"><div><Label className="text-sm font-medium">{label}</Label><p className="mt-1 max-w-md text-xs text-muted-foreground">{description}</p></div><div className="shrink-0">{children}</div></div>;
}
function Metric({name, reading}: {name:string; reading:string}) {
  return <div><dt className="text-xs text-muted-foreground">{name}</dt><dd className="mt-1 text-sm font-semibold">{reading}</dd></div>;
}
