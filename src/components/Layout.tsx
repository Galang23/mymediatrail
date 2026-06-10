import { useState, useEffect } from "react";
import { Sidebar } from "./Sidebar";
import { Library } from "./Library";
import { Duplicates } from "./Duplicates";
import { Cleanup } from "./Cleanup";
import type { LibraryRoot } from "../types";
import { invoke } from "@tauri-apps/api/core";

export function Layout() {
  const [currentView, setCurrentView] = useState("library");
  const [roots, setRoots] = useState<LibraryRoot[]>([]);
  const [error, setError] = useState("");
  const [scanStatus, setScanStatus] = useState<{ msg: string; scanning: boolean }>({ msg: "", scanning: false });

  useEffect(() => {
    invoke<LibraryRoot[]>("get_roots")
      .then(setRoots)
      .catch(e => setError("Failed to load roots: " + e));

    let unlistenProgress: any;
    let unlistenComplete: any;
    let unlistenError: any;

    import("@tauri-apps/api/event").then(({ listen }) => {
      listen("scan:progress", (event: any) => {
        setScanStatus({ scanning: true, msg: `Scanning: ${event.payload.current_file} (${event.payload.files_scanned} files)` });
      }).then(u => unlistenProgress = u);

      listen("scan:error", (event: any) => {
        setScanStatus({ scanning: false, msg: "" });
        setError("Scan error: " + event.payload);
      }).then(u => unlistenError = u);

      listen("scan:complete", (_event: any) => {
        setScanStatus({ scanning: false, msg: "" });
        invoke<LibraryRoot[]>("get_roots").then(setRoots).catch(console.error);
        setError("");
      }).then(u => unlistenComplete = u);
    });

    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "1") setCurrentView("library");
      if (e.key === "2") setCurrentView("duplicates");
      if (e.key === "3") setCurrentView("cleanup");
    };
    window.addEventListener("keydown", handleKey);

    return () => {
      if (unlistenProgress) unlistenProgress();
      if (unlistenComplete) unlistenComplete();
      if (unlistenError) unlistenError();
      window.removeEventListener("keydown", handleKey);
    };
  }, []);

  return (
    <div className="app-container">
      <Sidebar
        currentView={currentView}
        setCurrentView={setCurrentView}
        roots={roots}
        setRoots={setRoots}
        scanStatus={scanStatus}
      />
      <main className="main-content">
        {error && (
          <div className="error-banner" onClick={() => setError("")}>
            {error}
            <span style={{ marginLeft: '1rem', cursor: 'pointer' }}>&times;</span>
          </div>
        )}
        {scanStatus.scanning && (
          <div className="scan-progress-bar">
            {scanStatus.msg}
          </div>
        )}
        {currentView === "library" && <Library roots={roots} />}
        {currentView === "duplicates" && <Duplicates />}
        {currentView === "cleanup" && <Cleanup />}
      </main>
    </div>
  );
}
