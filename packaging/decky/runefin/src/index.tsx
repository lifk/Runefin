import { callable, definePlugin, toaster } from "@decky/api";
import { DialogButton, Focusable, PanelSection, PanelSectionRow } from "@decky/ui";
import { useEffect, useState } from "react";
import { FaTv } from "react-icons/fa";

type Status = { running: boolean; playback: string };
type Installation = { installed: boolean; wrapper: string; flatpak: boolean };

const getStatus = callable<[], Status>("status");
const getInstallation = callable<[], Installation>("installation");
const sendCommand = callable<[string], boolean>("command");

const APP_NAME = "Runefin";

function findRunefin() {
  return appStore.allApps.find(
    (app) => app.BIsShortcut() && app.display_name === APP_NAME,
  );
}

function wait(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** Launch the host installer's stable tile, or create one from its wrapper. */
async function openRunefin(wrapper: string, running: boolean) {
  if (running) {
    await sendCommand("raise");
    return;
  }
  const existing = findRunefin();
  if (existing) {
    SteamClient.Apps.RunGame(existing.gameid, "", -1, 100);
    return;
  }
  if (!wrapper) {
    throw new Error("Install the Steam tile first (scripts/steam-install.py).");
  }
  const appid = await SteamClient.Apps.AddShortcut(APP_NAME, wrapper, "", "");
  for (let attempt = 0; attempt < 20; attempt++) {
    await wait(100);
    const created = appStore.GetAppOverviewByAppID(appid);
    if (!created) continue;
    // AddShortcut can ignore some fields on the first call; set them once the overview exists.
    SteamClient.Apps.SetShortcutName(appid, APP_NAME);
    SteamClient.Apps.SetShortcutExe(appid, wrapper);
    SteamClient.Apps.SetShortcutLaunchOptions(appid, "");
    SteamClient.Apps.SetShortcutStartDir(appid, "");
    await wait(100);
    const ready = appStore.GetAppOverviewByAppID(appid);
    if (ready) {
      SteamClient.Apps.RunGame(ready.gameid, "", -1, 100);
      return;
    }
  }
  throw new Error("Steam did not create the Runefin shortcut.");
}

function showError(error: unknown) {
  toaster.toast({
    title: "Runefin",
    body: error instanceof Error ? error.message : String(error),
  });
}

function Content() {
  const [status, setStatus] = useState<Status>({
    running: false,
    playback: "",
  });
  const [installation, setInstallation] = useState<Installation>({
    installed: false,
    wrapper: "",
    flatpak: false,
  });
  const [busy, setBusy] = useState(false);

  const refresh = async () => {
    try {
      const next = await getStatus();
      setStatus(next);
    } catch {
      setStatus({ running: false, playback: "" });
    }
  };

  useEffect(() => {
    getInstallation().then(setInstallation).catch(showError);
    refresh();
    const timer = setInterval(refresh, 2000);
    return () => clearInterval(timer);
  }, []);

  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    try {
      await action();
      await refresh();
    } catch (error) {
      showError(error);
    } finally {
      setBusy(false);
    }
  };

  const transport = (action: string) => run(async () => {
    await sendCommand(action);
  });

  const open = () => run(async () => {
    await openRunefin(installation.wrapper, status.running);
  });

  const addToMedia = () => run(async () => {
    const app = findRunefin();
    if (!app) throw new Error("Open Runefin once first.");
    // Steam has no public collection API; this is the same untyped path other Decky
    // plugins use. It only touches the user's own collections, never system ones.
    type Collection = {
      displayName: string;
      AsDragDropCollection(): { AddApps(apps: unknown[]): void } | null;
      Save(): Promise<void>;
    };
    const store = collectionStore as unknown as {
      userCollections?: Collection[];
      NewUnsavedCollection(name: string, filter?: unknown, apps?: unknown[]): Collection;
    };
    const existing = store.userCollections?.find(
      (collection) => collection.displayName.toLowerCase() === "media",
    );
    const collection = existing ?? store.NewUnsavedCollection("Media", undefined, []);
    const editable = collection.AsDragDropCollection();
    if (!editable) throw new Error("Steam did not expose the Media collection for editing.");
    editable.AddApps([app]);
    await collection.Save();
    toaster.toast({ title: "Runefin", body: "Added to the Media collection." });
  });

  const state = !status.running
    ? "Not running"
    : status.playback === "Playing"
      ? "Playing"
      : status.playback === "Paused"
        ? "Paused"
        : "Open";

  return (
    <PanelSection>
      <PanelSectionRow>
        <Focusable style={{ display: "flex", alignItems: "center", gap: "8px" }}>
          <span style={{ fontSize: "1.2rem", fontWeight: 600 }}>{APP_NAME}</span>
          <span style={{ opacity: 0.7, marginLeft: "auto" }}>{state}</span>
        </Focusable>
      </PanelSectionRow>

      <PanelSectionRow>
        <DialogButton onClick={open} disabled={busy}>
          {status.running ? "Show Runefin" : "Open Runefin"}
        </DialogButton>
      </PanelSectionRow>

      {status.running && (
        <PanelSectionRow>
          <Focusable style={{ display: "flex", gap: "8px" }}>
            <DialogButton style={{ flex: 1 }} disabled={busy} onClick={() => transport("previous")}>
              ◀◀
            </DialogButton>
            <DialogButton style={{ flex: 1 }} disabled={busy} onClick={() => transport("playpause")}>
              {status.playback === "Playing" ? "Pause" : "Play"}
            </DialogButton>
            <DialogButton style={{ flex: 1 }} disabled={busy} onClick={() => transport("next")}>
              ▶▶
            </DialogButton>
          </Focusable>
        </PanelSectionRow>
      )}

      {status.running && (
        <PanelSectionRow>
          <Focusable style={{ display: "flex", gap: "8px" }}>
            <DialogButton style={{ flex: 1 }} disabled={busy} onClick={() => transport("stop")}>
              Stop
            </DialogButton>
            <DialogButton style={{ flex: 1 }} disabled={busy} onClick={() => transport("quit")}>
              Quit to Steam
            </DialogButton>
          </Focusable>
        </PanelSectionRow>
      )}

      <PanelSectionRow>
        <DialogButton onClick={addToMedia} disabled={busy}>
          Add to Media collection
        </DialogButton>
      </PanelSectionRow>

      {!installation.installed && (
        <PanelSectionRow>
          <span style={{ opacity: 0.7 }}>
            The Steam tile is not installed. Run scripts/steam-install.py on the host once.
          </span>
        </PanelSectionRow>
      )}

      {!status.running && (
        <PanelSectionRow>
          <span style={{ opacity: 0.6 }}>
            Transport controls appear while Runefin is open. HDR remains a Desktop Mode feature;
            this panel does not change the compositor.
          </span>
        </PanelSectionRow>
      )}
    </PanelSection>
  );
}

export default definePlugin(() => ({
  name: APP_NAME,
  content: <Content />,
  icon: <FaTv />,
}));
