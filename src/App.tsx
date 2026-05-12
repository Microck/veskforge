import {
  ArrowClockwise,
  Check,
  Coffee,
  Copy,
  Desktop,
  DotsThreeVertical,
  Folder,
  GitBranch,
  GithubLogo,
  Hammer,
  MagnifyingGlass,
  Package,
  Play,
  Plus,
  ShieldCheck,
  Sparkle,
  TerminalWindow,
  Trash,
  UploadSimple,
} from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { Icon } from "@phosphor-icons/react";
import type { ReactNode } from "react";
import { useEffect, useMemo, useState } from "react";
import "./App.css";

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

type View = "sources" | "build" | "vesktop";

const sourceLabels: Record<PluginSource["kind"], string> = {
  localFile: ".ts file",
  localFolder: "Local folder",
  git: "Git repository",
};

const sourceIcons = {
  localFile: <span className="file-badge">TS</span>,
  localFolder: <Folder size={23} weight="regular" />,
  git: <GitBranch size={23} weight="regular" />,
} satisfies Record<PluginSource["kind"], ReactNode>;

const views = [
  { id: "sources", label: "Sources", icon: Folder },
  { id: "build", label: "Build", icon: Hammer },
  { id: "vesktop", label: "Vesktop", icon: Desktop },
] satisfies { id: View; label: string; icon: Icon }[];

const repositoryUrl = "https://github.com/Microck/veskforge";
const kofiUrl = "https://ko-fi.com/microck";

function sourceSummary(source: PluginSource) {
  if (source.kind === "git") return `${source.url}${source.reference ? ` @ ${source.reference}` : ""}`;
  return source.path;
}

function formatDate(value?: string) {
  if (!value) return "Never";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  }).format(date);
}

function errorMessage(error: unknown) {
  const message = error instanceof Error ? error.message : String(error);
  if (message.includes("invoke")) {
    return "Tauri backend unavailable in browser preview. Launch with `pnpm tauri dev` for live system commands.";
  }
  return message;
}

async function openExternal(url: string) {
  try {
    await openUrl(url);
  } catch {
    window.open(url, "_blank", "noopener,noreferrer");
  }
}

function App() {
  const [status, setStatus] = useState<EnvironmentStatus | null>(null);
  const [activeView, setActiveView] = useState<View>("sources");
  const [busy, setBusy] = useState<string | null>(null);
  const [log, setLog] = useState("Ready.");
  const [pluginKind, setPluginKind] = useState<PluginSource["kind"]>("localFolder");
  const [pluginPath, setPluginPath] = useState("");
  const [pluginName, setPluginName] = useState("");
  const [gitRef, setGitRef] = useState("");
  const [vesktopStatePath, setVesktopStatePath] = useState("");
  const [search, setSearch] = useState("");
  const [showComposer, setShowComposer] = useState(false);

  const manifest = status?.manifest;
  const plugins = manifest?.plugins ?? [];
  const filteredPlugins = useMemo(() => {
    const query = search.trim().toLowerCase();
    if (!query) return plugins;
    return plugins.filter((plugin) => {
      const source = sourceSummary(plugin.source).toLowerCase();
      return plugin.name.toLowerCase().includes(query) || source.includes(query) || sourceLabels[plugin.source.kind].toLowerCase().includes(query);
    });
  }, [plugins, search]);
  const enabledCount = plugins.filter((plugin) => plugin.enabled).length;
  const missingTools = useMemo(
    () => status?.tools.filter((tool) => !tool.available).map((tool) => tool.name) ?? [],
    [status],
  );
  const logLines = useMemo(() => log.split("\n").filter((line) => line.trim().length > 0), [log]);
  const canBuild = !busy && missingTools.length === 0;
  const detectedInstall = Boolean(vesktopStatePath || status?.selectedVesktopState);

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

    await runAction("Adding source", async () =>
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
    setShowComposer(false);
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
    <main className="app-shell">
      <div className="desktop-frame">
        <aside className="sidebar">
          <nav aria-label="Primary">
            {views.map((view) => {
              const Icon = view.icon;
              return (
                <button
                  className={activeView === view.id ? "nav-item active" : "nav-item"}
                  key={view.id}
                  onClick={() => setActiveView(view.id)}
                >
                  <Icon size={27} weight="regular" />
                  <span>{view.label}</span>
                </button>
              );
            })}
          </nav>

          <div className="sidebar-footer">
            <button aria-label="Open GitHub repository" onClick={() => openExternal(repositoryUrl)} title="GitHub">
              <GithubLogo size={25} />
            </button>
            <button aria-label="Open Ko-fi" onClick={() => openExternal(kofiUrl)} title="Ko-fi">
              <Coffee size={25} />
            </button>
          </div>
        </aside>

        <section className="workspace" aria-live="polite">
          {activeView === "sources" && (
            <section className="view sources-view">
              <ViewHeader title="Plugin sources" subtitle="Manage custom plugin inputs for your next build." />

              <div className="source-toolbar">
                <label className="search-field">
                  <MagnifyingGlass size={24} />
                  <input value={search} onChange={(event) => setSearch(event.currentTarget.value)} placeholder="Search sources..." />
                </label>
                <button className="primary add-source" disabled={!!busy} onClick={() => setShowComposer((current) => !current)}>
                  <Plus size={24} />
                  Add source
                </button>
              </div>

              <div className="source-table">
                <div className="source-head">
                  <span />
                  <span>Name</span>
                  <span>Type</span>
                  <span>Path</span>
                  <span>Enabled</span>
                  <span />
                </div>
                <div className="source-body">
                  {filteredPlugins.length ? (
                    filteredPlugins.map((plugin) => (
                      <article className="source-row" key={plugin.id}>
                        <div className="source-icon">{sourceIcons[plugin.source.kind]}</div>
                        <strong>{plugin.name}</strong>
                        <span>{sourceLabels[plugin.source.kind]}</span>
                        <span className="path-text" title={sourceSummary(plugin.source)}>
                          {sourceSummary(plugin.source)}
                        </span>
                        <Switch
                          checked={plugin.enabled}
                          disabled={!!busy}
                          label={`${plugin.enabled ? "Disable" : "Enable"} ${plugin.name}`}
                          onChange={(enabled) =>
                            runAction("Updating source", async () =>
                              invoke<Manifest>("set_plugin_enabled", {
                                pluginId: plugin.id,
                                enabled,
                              }),
                            )
                          }
                        />
                        <button
                          className="icon-button danger"
                          disabled={!!busy}
                          aria-label={`Remove ${plugin.name}`}
                          onClick={() => runAction("Removing source", async () => invoke<Manifest>("remove_plugin", { pluginId: plugin.id }))}
                        >
                          <Trash size={21} />
                        </button>
                      </article>
                    ))
                  ) : (
                    <div className="empty-row">{search ? "No sources match this search." : "No plugin sources added yet."}</div>
                  )}
                </div>
              </div>

              <AddSourcePanel
                visible={showComposer}
                busy={!!busy}
                pluginKind={pluginKind}
                pluginPath={pluginPath}
                pluginName={pluginName}
                gitRef={gitRef}
                setPluginKind={setPluginKind}
                setPluginPath={setPluginPath}
                setPluginName={setPluginName}
                setGitRef={setGitRef}
                addPlugin={addPlugin}
              />

              <FooterNote icon={<ShieldCheck size={23} />}>Plugin sources are application code. Only add sources you trust.</FooterNote>
            </section>
          )}

          {activeView === "build" && (
            <section className="view build-view">
              <ViewHeader title="Build" subtitle="Generate and inspect your managed Vencord build." />

              <div className="build-summary">
                <div>
                  <Folder size={23} />
                  <span>
                    {enabledCount}/{plugins.length} sources
                  </span>
                </div>
                <div>
                  <TerminalWindow size={23} />
                  <span>{missingTools.length ? `Missing ${missingTools.join(", ")}` : "Build toolchain"}</span>
                </div>
                <div>
                  <ArrowClockwise size={23} />
                  <span>{manifest?.updatePolicy.mode === "auto" ? "Auto updates" : "Manual updates"}</span>
                </div>
              </div>

              <div className="build-actions">
                <button className="primary build-now" disabled={!canBuild} onClick={() => runAction("Building Vencord", () => invoke("build_vencord"))}>
                  <Play size={25} weight="regular" />
                  Build now
                </button>
                <button className="secondary" disabled={!canBuild} onClick={() => runAction("Checking updates", () => invoke("check_updates"))}>
                  <ArrowClockwise size={25} />
                  Check updates
                </button>
                {missingTools.length > 0 && (
                  <button className="secondary install-tools" disabled={!!busy} onClick={() => runAction("Installing tools", () => invoke("install_toolchain"))}>
                    <TerminalWindow size={22} />
                    Install missing tools
                  </button>
                )}
                <StatusPill ok={missingTools.length === 0}>{missingTools.length ? "Toolchain incomplete" : "Ready to build"}</StatusPill>
              </div>

              <section className="data-section">
                <h3>Build output</h3>
                <div className="data-card">
                  <KeyValue label="Checkout path" value={status?.vencordDir ?? "Loading..."} copy />
                  <KeyValue label="Dist path" value={status?.distDir ?? "Loading..."} copy />
                  <KeyValue
                    label="Last build"
                    value={manifest?.lastSuccessfulBuild ? `${formatDate(manifest.lastSuccessfulBuild.builtAt)}` : "Never"}
                    status={manifest?.lastSuccessfulBuild ? "Succeeded" : undefined}
                  />
                </div>
              </section>

              <section className="data-section">
                <h3>Activity log</h3>
                <div className="activity-log">
                  {logLines.map((line, index) => (
                    <LogLine
                      key={`${line}-${index}`}
                      tone={index === 0 && !busy ? "green" : "blue"}
                      time={index === 0 ? (busy ? "Now" : "Ready") : `${index}`.padStart(2, "0")}
                      text={line}
                    />
                  ))}
                </div>
              </section>
            </section>
          )}

          {activeView === "vesktop" && (
            <section className="view vesktop-view">
              <ViewHeader title="Vesktop target" subtitle="Choose where the validated build should be applied." />

              <section className="target-card">
                <label>
                  <span>state.json</span>
                  <div className="target-input">
                    <input value={vesktopStatePath} onChange={(event) => setVesktopStatePath(event.currentTarget.value)} placeholder="Autodetect or paste path" />
                    <button className="secondary" disabled={!!busy} onClick={autodetectVesktopState}>
                      Autodetect
                      <Sparkle size={21} />
                    </button>
                  </div>
                </label>
              </section>

              <section className="install-card">
                <div className={detectedInstall ? "check-ring ok" : "check-ring muted"}>
                  <Check size={27} />
                </div>
                <div>
                  <h3>{detectedInstall ? "Detected install" : "No install selected"}</h3>
                  <p>{detectedInstall ? "Vesktop installation found and ready." : "Use autodetect or paste a state.json path."}</p>
                </div>
                <div className="last-applied">
                  <span>Last applied</span>
                  <strong>{manifest?.lastSuccessfulBuild ? formatDate(manifest.lastSuccessfulBuild.builtAt) : "Never"}</strong>
                </div>
              </section>

              <section className="rebuild-card">
                <div>
                  <h3>Auto rebuild</h3>
                  <p>Automatically rebuild when sources change.</p>
                </div>
                <Switch
                  checked={manifest?.updatePolicy.mode === "auto"}
                  disabled={!!busy}
                  label="Toggle auto rebuild"
                  onChange={(enabled) =>
                    runAction("Saving update policy", async () =>
                      invoke<Manifest>("set_update_policy", {
                        request: { mode: enabled ? "auto" : "manual" },
                      }),
                    )
                  }
                />
              </section>

              <section className="review-card">
                <h3>Review</h3>
                <KeyValue icon={<Folder size={22} />} label="Target path" value={vesktopStatePath || "No target selected"} />
                <KeyValue icon={<Package size={22} />} label="Build version" value={manifest?.lastSuccessfulBuild?.vencordRevision ?? "Not built"} />
                <KeyValue
                  icon={<ShieldCheck size={22} />}
                  label="Validation status"
                  value={manifest?.lastSuccessfulBuild ? "Validated build ready" : "Build required"}
                  status={manifest?.lastSuccessfulBuild ? "Validated build ready" : undefined}
                />
              </section>

              <div className="apply-footer">
                <button
                  className="primary apply-build"
                  disabled={!!busy}
                  onClick={() =>
                    runAction("Applying to Vesktop", () =>
                      invoke("apply_to_vesktop", {
                        request: { statePath: vesktopStatePath.trim() || undefined },
                      }),
                    )
                  }
                >
                  <UploadSimple size={24} />
                  Apply build
                </button>
              </div>
            </section>
          )}
        </section>
      </div>
    </main>
  );
}

function ViewHeader({ title, subtitle }: { title: string; subtitle: string }) {
  return (
    <header className="view-header">
      <h1>{title}</h1>
      <span>{subtitle}</span>
    </header>
  );
}

function AddSourcePanel({
  visible,
  busy,
  pluginKind,
  pluginPath,
  pluginName,
  gitRef,
  setPluginKind,
  setPluginPath,
  setPluginName,
  setGitRef,
  addPlugin,
}: {
  visible: boolean;
  busy: boolean;
  pluginKind: PluginSource["kind"];
  pluginPath: string;
  pluginName: string;
  gitRef: string;
  setPluginKind: (value: PluginSource["kind"]) => void;
  setPluginPath: (value: string) => void;
  setPluginName: (value: string) => void;
  setGitRef: (value: string) => void;
  addPlugin: () => void;
}) {
  return (
    <section className={visible ? "drop-panel editing" : "drop-panel"}>
      {visible ? (
        <div className="source-form">
          <label>
            <span>Type</span>
            <select value={pluginKind} onChange={(event) => setPluginKind(event.currentTarget.value as PluginSource["kind"])}>
              <option value="localFolder">Local folder</option>
              <option value="localFile">.ts file</option>
              <option value="git">Git repository</option>
            </select>
          </label>
          <label>
            <span>{pluginKind === "git" ? "Git URL" : "Path"}</span>
            <input
              value={pluginPath}
              onChange={(event) => setPluginPath(event.currentTarget.value)}
              placeholder={pluginKind === "git" ? "https://github.com/user/vencord-plugin.git" : "/path/to/plugin"}
            />
          </label>
          <label>
            <span>Name</span>
            <input value={pluginName} onChange={(event) => setPluginName(event.currentTarget.value)} placeholder="Optional" />
          </label>
          {pluginKind === "git" && (
            <label>
              <span>Ref</span>
              <input value={gitRef} onChange={(event) => setGitRef(event.currentTarget.value)} placeholder="branch, tag, or commit" />
            </label>
          )}
          <button className="primary" disabled={busy} onClick={addPlugin}>
            <Plus size={22} />
            Add source
          </button>
        </div>
      ) : (
        <>
          <div className="drop-icon">
            <UploadSimple size={24} />
          </div>
          <strong>Drop folder, .ts file, or Git URL here</strong>
          <span>to add a new source</span>
        </>
      )}
    </section>
  );
}

function Switch({
  checked,
  disabled,
  label,
  onChange,
}: {
  checked: boolean;
  disabled?: boolean;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="switch" aria-label={label}>
      <input type="checkbox" checked={checked} disabled={disabled} onChange={(event) => onChange(event.currentTarget.checked)} />
      <span />
    </label>
  );
}

function StatusPill({ ok, children }: { ok: boolean; children: ReactNode }) {
  return (
    <div className="status-pill">
      <span className={ok ? "dot ok" : "dot danger"} />
      {children}
    </div>
  );
}

function KeyValue({
  label,
  value,
  status,
  copy,
  icon,
}: {
  label: string;
  value: string;
  status?: string;
  copy?: boolean;
  icon?: ReactNode;
}) {
  return (
    <div className="key-value">
      <div className="key-label">
        {icon}
        <span>{label}</span>
      </div>
      <div className="key-result">
        {status && (
          <span className="success-status">
            <Check size={21} />
            {status}
          </span>
        )}
        <span>{value}</span>
        {copy && (
          <button className="copy-button" aria-label={`Copy ${label}`}>
            <Copy size={21} />
          </button>
        )}
        {!copy && !status && <DotsThreeVertical className="ghost-dots" size={19} />}
      </div>
    </div>
  );
}

function LogLine({ tone, time, text }: { tone: "green" | "blue"; time: string; text: string }) {
  return (
    <div className="log-line">
      <span className={`dot ${tone}`} />
      <time>{time}</time>
      <p>{text}</p>
    </div>
  );
}

function FooterNote({ icon, children }: { icon: ReactNode; children: ReactNode }) {
  return (
    <footer className="footer-note">
      {icon}
      <span>{children}</span>
    </footer>
  );
}

export default App;
