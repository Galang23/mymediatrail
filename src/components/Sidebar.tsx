import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Folder, Search, CheckCircle, FileVideo, HardDrive, AlertTriangle, Plus, X } from "lucide-react";
import type { LibraryRoot } from "../types";

interface SidebarProps {
  currentView: string;
  setCurrentView: (view: string) => void;
  roots: LibraryRoot[];
  setRoots: (roots: LibraryRoot[]) => void;
  scanMsg: string;
  isScanning: boolean;
}

export function Sidebar({ currentView, setCurrentView, roots, setRoots, scanMsg, isScanning }: SidebarProps) {
  const [isAddingRoot, setIsAddingRoot] = useState(false);
  const [newRootLabel, setNewRootLabel] = useState("");
  const [newRootPath, setNewRootPath] = useState("");
  
  const [isRebinding, setIsRebinding] = useState<string | null>(null);
  const [rebindPath, setRebindPath] = useState("");

  const handleAddRoot = async () => {
    if (!newRootLabel || !newRootPath) return;
    try {
      await invoke("add_root", { label: newRootLabel, path: newRootPath });
      setIsAddingRoot(false);
      setNewRootLabel("");
      setNewRootPath("");
      fetchRoots();
    } catch (e) {
      alert("Error adding root: " + e);
    }
  };

  const handleRebind = async (rootId: string) => {
    if (!rebindPath) return;
    try {
      await invoke("rebind_root", { rootId, newPath: rebindPath });
      setIsRebinding(null);
      setRebindPath("");
      fetchRoots();
    } catch (e) {
      alert("Error rebinding root: " + e);
    }
  };

  const fetchRoots = async () => {
    try {
      const fetchedRoots = await invoke<LibraryRoot[]>("get_roots");
      setRoots(fetchedRoots);
    } catch (e) {
      console.error(e);
    }
  };

  const triggerScan = async (rootId: string) => {
    try {
      await invoke("trigger_scan", { rootId });
    } catch (e) {
      console.error(e);
    }
  };

  return (
    <aside className="sidebar glass-panel">
      <div className="sidebar-header">
        <FileVideo size={24} color="var(--accent-color)" className="pulse-slow" />
        <h2>MyMediaTrail</h2>
      </div>

      <nav className="sidebar-nav">
        <button 
          className={`nav-item ${currentView === 'library' ? 'active' : ''}`}
          onClick={() => setCurrentView('library')}
        >
          <Search size={18} /> Library
        </button>
        <button 
          className={`nav-item ${currentView === 'duplicates' ? 'active' : ''}`}
          onClick={() => setCurrentView('duplicates')}
        >
          <CheckCircle size={18} /> Duplicates
        </button>
        <button 
          className={`nav-item ${currentView === 'cleanup' ? 'active' : ''}`}
          onClick={() => setCurrentView('cleanup')}
        >
          <HardDrive size={18} /> Cleanup
        </button>
      </nav>

      <div className="roots-section">
        <div className="roots-header">
          <h3>Locations</h3>
          <button className="icon-button add-btn-round" onClick={() => setIsAddingRoot(true)}>
            <Plus size={16} />
          </button>
        </div>

        <div className="roots-list">
          {roots.map(root => {
            const isOffline = root.root_status === 'missing';
            
            return (
              <div key={root.id} className={`root-item ${isOffline ? 'missing' : 'active'}`}>
                <div className="root-info">
                  <Folder 
                    size={16} 
                    className="folder-icon"
                    color={isOffline ? "var(--warning-color)" : "var(--success-color)"} 
                  />
                  <div className="root-text">
                    <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
                      <span className={`status-dot-small ${isOffline ? 'offline' : 'online'}`}></span>
                      <span className="root-label">{root.label}</span>
                    </div>
                    <span className="root-path" title={root.selected_path}>{root.selected_path}</span>
                  </div>
                </div>
                
                <div className="root-actions">
                  {isOffline ? (
                    <button 
                      onClick={() => {
                        setIsRebinding(isRebinding === root.id ? null : root.id);
                        setRebindPath(root.selected_path);
                      }} 
                      className="rebind-btn" 
                      title="Volume offline. Click to manually rebind path."
                    >
                      <AlertTriangle size={14} color="var(--warning-color)" /> Rebind
                    </button>
                  ) : (
                    <button onClick={() => triggerScan(root.id)} className="scan-btn" disabled={isScanning}>
                      Scan
                    </button>
                  )}
                </div>

                {isRebinding === root.id && (
                  <div className="rebind-dialog animate-fade-in">
                    <input 
                      type="text" 
                      className="input-field" 
                      placeholder="New Path (e.g. /media/usb1)" 
                      value={rebindPath} 
                      onChange={e => setRebindPath(e.target.value)}
                    />
                    <div style={{ display: 'flex', gap: '0.5rem' }}>
                      <button className="glass-button primary small" onClick={() => handleRebind(root.id)}>Save</button>
                      <button className="glass-button small" onClick={() => setIsRebinding(null)}>Cancel</button>
                    </div>
                  </div>
                )}
              </div>
            );
          })}
          {roots.length === 0 && (
            <div style={{ fontSize: '0.8rem', color: 'var(--text-secondary)', padding: '0.5rem 0' }}>
              No active locations.
            </div>
          )}
        </div>

        {isAddingRoot && (
          <div className="add-root-form animate-fade-in">
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '0.5rem' }}>
              <span style={{ fontSize: '0.8rem', fontWeight: 600, color: 'var(--accent-hover)' }}>Add Location</span>
              <button className="icon-button" onClick={() => setIsAddingRoot(false)}>
                <X size={14} />
              </button>
            </div>
            <input 
              className="input-field" 
              placeholder="Label (e.g. Movies)" 
              value={newRootLabel} 
              onChange={e => setNewRootLabel(e.target.value)} 
            />
            <input 
              className="input-field" 
              placeholder="Absolute Path" 
              value={newRootPath} 
              onChange={e => setNewRootPath(e.target.value)} 
            />
            <div style={{ display: 'flex', gap: '0.5rem' }}>
              <button className="glass-button primary" onClick={handleAddRoot}>Add</button>
              <button className="glass-button" onClick={() => setIsAddingRoot(false)}>Cancel</button>
            </div>
          </div>
        )}
      </div>

      {isScanning && (
        <div className="scan-progress animate-fade-in">
          <div className="scan-spinner"></div>
          <div className="scan-text">{scanMsg}</div>
        </div>
      )}
    </aside>
  );
}
