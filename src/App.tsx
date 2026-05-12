import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";
import "./App.css";
import logoDark from "./assets/veskforge-logo-dark.svg";
import logoLight from "./assets/veskforge-logo-light.svg";

type PluginSource =
  | { kind: "localFile"; path: string }
  | { kind: "localFolder"; path: string }
  | { kind: "git"; url: string; reference?: string };

type PluginRecord = {
  id: string;
  name: string;
  source: PluginSource;
  enabled: boolean;
  installedPath: string;
  lastRevision?: string;
};

type Manifest = {
  updatePolicy: { mode: "manual" | "auto" };
  plugins: PluginRecord[];
  lastSuccessfulBuild?: {
    builtAt: string;
    distPath: string;
    vencordRevision?: string;
  };
};

type ToolStatus = {
  name: string;
  available: boolean;
  version?: string;
};

type EnvironmentStatus = {
  appDataDir: string;
  workspaceDir: string;
  vencordDir: string;
  distDir: string;
  vesktopStateCandidates: string[];
  selectedVesktopState?: string;
  tools: ToolStatus[];
  manifest: Manifest;
};

type CommandResult = {
  ok: boolean;
  message: string;
  log: string;
};

const sourceLabels: Record<PluginSource["kind"], string> = {
  localFile: "Local file",
  localFolder: "Local folder",
  git: "Git",
};

function sourceSummary(source: PluginSource) {
  if (source.kind === "git") return `${source.url}${source.reference ? ` @ ${source.reference}` : ""}`;
  return source.path;
}

function errorMessage(error: unknown) {
  const message = error instanceof Error ? error.message : String(error);
  if (message.includes("invoke")) {
    return "Tauri backend unavailable in browser preview. Launch with `pnpm tauri dev` for live system commands.";
  }
  return message;
}

function App() {
  const [status, setStatus] = useState<EnvironmentStatus | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [log, setLog] = useState("Ready.");
  const [pluginKind, setPluginKind] = useState<PluginSource["kind"]>("localFolder");
  const [pluginPath, setPluginPath] = useState("");
  const [pluginName, setPluginName] = useState("");
  const [gitRef, setGitRef] = useState("");
  const [vesktopStatePath, setVesktopStatePath] = useState("");

  const manifest = status?.manifest;
  const enabledCount = manifest?.plugins.filter((plugin) => plugin.enabled).length ?? 0;
  const missingTools = useMemo(
    () => status?.tools.filter((tool) => !tool.available).map((tool) => tool.name) ?? [],
    [status],
  );

  async function refresh() {
    const nextStatus = await invoke<EnvironmentStatus>("get_environment_status");
    setStatus(nextStatus);
    setVesktopStatePath((current) => current || nextStatus.selectedVesktopState || "");
  }

  async function runAction(label: string, action: () => Promise<CommandResult | Manifest>) {
    setBusy(label);
    setLog(`${label}...`);
    try {
      const result = await action();
      if ("plugins" in result) {
        await refresh();
        setLog(`${label} complete.`);
      } else {
        await refresh();
        setLog(`${result.message}\n\n${result.log}`.trim());
      }
    } catch (error) {
      setLog(errorMessage(error));
    } finally {
      setBusy(null);
    }
  }

  async function addPlugin() {
    const trimmedSource = pluginPath.trim();
    if (!trimmedSource) {
      setLog("Enter a local path or Git URL before adding a plugin.");
      return;
    }

    const source: PluginSource =
      pluginKind === "git"
        ? { kind: "git", url: trimmedSource, reference: gitRef.trim() || undefined }
        : pluginKind === "localFile"
          ? { kind: "localFile", path: trimmedSource }
          : { kind: "localFolder", path: trimmedSource };

    await runAction("Adding plugin", async () =>
      invoke<Manifest>("add_plugin", {
        request: {
          source,
          name: pluginName.trim() || undefined,
        },
      }),
    );
    setPluginName("");
    setPluginPath("");
    setGitRef("");
  }

  async function autodetectVesktopState() {
    setBusy("Detecting Vesktop");
    setLog("Checking common Vesktop state locations...");
    try {
      const nextStatus = await invoke<EnvironmentStatus>("get_environment_status");
      setStatus(nextStatus);
      const detectedPath = nextStatus.selectedVesktopState ?? "";
      setVesktopStatePath(detectedPath);
      setLog(
        detectedPath
          ? `Detected Vesktop state.json:\n${detectedPath}`
          : `No Vesktop state.json found in known locations:\n${nextStatus.vesktopStateCandidates.join("\n")}`,
      );
    } catch (error) {
      setLog(errorMessage(error));
    } finally {
      setBusy(null);
    }
  }

  useEffect(() => {
    refresh().catch((error) => setLog(errorMessage(error)));
  }, []);

  return (
    <main className="shell">
      <aside className="sidebar">
        <div className="brand">
          <picture className="brand-mark">
            <source srcSet={logoLight} media="(prefers-color-scheme: light)" />
            <img src={logoDark} alt="" />
          </picture>
          <div>
            <h1>veskforge</h1>
            <p>Custom Vencord builds for Vesktop</p>
          </div>
        </div>

        <nav aria-label="Primary">
          <a href="#plugins">Sources</a>
          <a href="#build">Build</a>
          <a href="#settings">Vesktop</a>
        </nav>

        <div className="trust-note">
          <span>Custom plugins execute inside Vencord. Treat every source like application code.</span>
        </div>
      </aside>

      <section className="content">
        <header className="topbar" id="dashboard">
          <div>
            <p className="eyebrow">Vencord build forge</p>
            <h2>Custom plugins, one managed Vesktop target.</h2>
          </div>
          <button className="secondary" disabled={!!busy} onClick={refresh}>
            Refresh
          </button>
        </header>

        <section className="status-grid">
          <article>
            <span className="metric">{manifest?.plugins.length ?? 0}</span>
            <p>Managed plugins</p>
          </article>
          <article>
            <span className="metric">{enabledCount}</span>
            <p>Enabled for next build</p>
          </article>
          <article>
            <span className={missingTools.length ? "pill danger" : "pill ok"}>
              {missingTools.length ? missingTools.join(", ") : "Ready"}
            </span>
            <p>Toolchain</p>
          </article>
          <article>
            <span className="metric small">{manifest?.updatePolicy.mode ?? "manual"}</span>
            <p>Update policy</p>
          </article>
        </section>

        <section className="workspace">
          <section className="panel source-panel" id="plugins">
            <div className="panel-header">
              <div>
                <p className="eyebrow">Plugin sources</p>
                <h3>Add custom plugin</h3>
              </div>
            </div>
            <div className="form-grid">
              <label>
                Source type
                <select value={pluginKind} onChange={(event) => setPluginKind(event.currentTarget.value as PluginSource["kind"])}>
                  <option value="localFolder">Local folder</option>
                  <option value="localFile">Local file</option>
                  <option value="git">Git URL</option>
                </select>
              </label>
              <label className="wide">
                {pluginKind === "git" ? "Git URL" : "Path"}
                <input
                  value={pluginPath}
                  onChange={(event) => setPluginPath(event.currentTarget.value)}
                  placeholder={pluginKind === "git" ? "https://github.com/user/vencord-plugin.git" : "/path/to/plugin"}
                />
              </label>
              <label>
                Display name
                <input value={pluginName} onChange={(event) => setPluginName(event.currentTarget.value)} placeholder="Optional" />
              </label>
              {pluginKind === "git" && (
                <label>
                  Ref
                  <input value={gitRef} onChange={(event) => setGitRef(event.currentTarget.value)} placeholder="branch, tag, or commit" />
                </label>
              )}
              <button disabled={!!busy} onClick={addPlugin}>
                Add plugin
              </button>
            </div>

            <div className="plugin-list" aria-label="Managed plugins">
              {manifest?.plugins.length ? (
                manifest.plugins.map((plugin) => (
                  <article className="plugin-card" key={plugin.id}>
                    <div>
                      <div className="plugin-title">
                        <h4>{plugin.name}</h4>
                        <span>{sourceLabels[plugin.source.kind]}</span>
                      </div>
                      <p>{sourceSummary(plugin.source)}</p>
                      {plugin.lastRevision && <small>Last revision {plugin.lastRevision}</small>}
                    </div>
                    <div className="plugin-actions">
                      <label className="switch">
                        <input
                          type="checkbox"
                          checked={plugin.enabled}
                          disabled={!!busy}
                          onChange={(event) =>
                            runAction("Updating plugin", async () =>
                              invoke<Manifest>("set_plugin_enabled", {
                                pluginId: plugin.id,
                                enabled: event.currentTarget.checked,
                              }),
                            )
                          }
                        />
                        <span>{plugin.enabled ? "Enabled" : "Disabled"}</span>
                      </label>
                      <button
                        className="danger"
                        disabled={!!busy}
                        onClick={() => runAction("Removing plugin", async () => invoke<Manifest>("remove_plugin", { pluginId: plugin.id }))}
                      >
                        Remove
                      </button>
                    </div>
                  </article>
                ))
              ) : (
                <div className="empty">Drop in a local plugin folder, single `.ts` file, or Git source.</div>
              )}
            </div>
          </section>

          <section className="side-stack">
            <section className="panel" id="build">
              <div className="panel-header">
                <div>
                  <p className="eyebrow">Build output</p>
                  <h3>Managed Vencord checkout</h3>
                </div>
              </div>
              <dl>
                <dt>Checkout</dt>
                <dd>{status?.vencordDir ?? "Loading..."}</dd>
                <dt>Dist</dt>
                <dd>{status?.distDir ?? "Loading..."}</dd>
                <dt>Last build</dt>
                <dd>{manifest?.lastSuccessfulBuild?.builtAt ?? "Never"}</dd>
              </dl>
              <div className="action-row">
                <button disabled={!!busy || missingTools.length > 0} onClick={() => runAction("Checking updates", () => invoke("check_updates"))}>
                  Check updates
                </button>
                <button disabled={!!busy || missingTools.length > 0} onClick={() => runAction("Building Vencord", () => invoke("build_vencord"))}>
                  Build
                </button>
              </div>
            </section>

            <section className="panel" id="settings">
              <div className="panel-header">
                <div>
                  <p className="eyebrow">Vesktop target</p>
                  <h3>Apply validated build</h3>
                </div>
              </div>
              <label>
                state.json
                <div className="inline-field">
                  <input value={vesktopStatePath} onChange={(event) => setVesktopStatePath(event.currentTarget.value)} placeholder="Autodetect or paste path" />
                  <button className="secondary" disabled={!!busy} onClick={autodetectVesktopState}>
                    Autodetect
                  </button>
                </div>
              </label>
              <div className="candidate-list">
                {(status?.vesktopStateCandidates ?? []).slice(0, 3).map((candidate) => (
                  <button className="path-option" disabled={!!busy} key={candidate} onClick={() => setVesktopStatePath(candidate)}>
                    {candidate}
                  </button>
                ))}
              </div>
              <div className="action-row">
                <label className="switch policy">
                  <input
                    type="checkbox"
                    checked={manifest?.updatePolicy.mode === "auto"}
                    disabled={!!busy}
                    onChange={(event) =>
                      runAction("Saving update policy", async () =>
                        invoke<Manifest>("set_update_policy", {
                          request: { mode: event.currentTarget.checked ? "auto" : "manual" },
                        }),
                      )
                    }
                  />
                  <span>Auto rebuild</span>
                </label>
                <button
                  disabled={!!busy}
                  onClick={() =>
                    runAction("Applying to Vesktop", () =>
                      invoke("apply_to_vesktop", {
                        request: { statePath: vesktopStatePath.trim() || undefined },
                      }),
                    )
                  }
                >
                  Apply
                </button>
              </div>
            </section>
          </section>
        </section>

        <section className="log-panel" aria-live="polite">
          <div className="panel-header">
            <h3>{busy ?? "Activity log"}</h3>
          </div>
          <pre>{log}</pre>
        </section>
      </section>
    </main>
  );
}

export default App;
