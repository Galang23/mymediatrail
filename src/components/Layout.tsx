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
  
  const [isScanning, setIsScanning] = useState(false);
  const [scanMsg, setScanMsg] = useState("");

  useEffect(() => {
    // Initial fetch
    invoke<LibraryRoot[]>("get_roots").then(setRoots).catch(console.error);

    // Setup events
    let unlistenProgress: any;
    let unlistenComplete: any;

    import("@tauri-apps/api/event").then(({ listen }) => {
      listen("scan:progress", (event: any) => {
        setIsScanning(true);
        setScanMsg(`Scanned: ${event.payload.files_scanned}`);
      }).then(u => unlistenProgress = u);
      
      listen("scan:complete", (event: any) => {
        setScanMsg(`Done! New: ${event.payload.new_items}`);
        setTimeout(() => setIsScanning(false), 3000);
        invoke<LibraryRoot[]>("get_roots").then(setRoots).catch(console.error);
      }).then(u => unlistenComplete = u);
    });

    return () => {
      if (unlistenProgress) unlistenProgress();
      if (unlistenComplete) unlistenComplete();
    };
  }, []);

  return (
    <div className="app-container">
      <Sidebar 
        currentView={currentView} 
        setCurrentView={setCurrentView} 
        roots={roots}
        setRoots={setRoots}
        scanMsg={scanMsg}
        isScanning={isScanning}
      />
      
      <main className="main-content">
        {currentView === "library" && <Library roots={roots} />}
        {currentView === "duplicates" && <Duplicates />}
        {currentView === "cleanup" && <Cleanup />}
      </main>
    </div>
  );
}
