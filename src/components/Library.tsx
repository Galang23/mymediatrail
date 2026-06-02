import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Play, AlertCircle, Plus, HardDrive, Flame } from "lucide-react";
import type { MediaItem, LibraryRoot } from "../types";

function MediaThumbnail({ itemId, isOffline }: { itemId: string; isOffline: boolean }) {
  const [thumb, setThumb] = useState<string | null>(null);

  useEffect(() => {
    if (isOffline) return;
    
    let active = true;
    invoke<string | null>("get_thumbnail", { mediaItemId: itemId })
      .then(base64 => {
        if (active && base64) {
          setThumb(`data:image/jpeg;base64,${base64}`);
        }
      })
      .catch(console.error);

    return () => {
      active = false;
    };
  }, [itemId, isOffline]);

  if (isOffline) {
    return <AlertCircle size={48} className="alert-icon" />;
  }

  if (thumb) {
    return <img src={thumb} alt="video preview" style={{ width: '100%', height: '100%', objectFit: 'cover' }} className="animate-fade-in" />;
  }

  return <Play size={48} />;
}

export function Library({ roots }: { roots: LibraryRoot[] }) {
  const [items, setItems] = useState<MediaItem[]>([]);
  const [activeRoot, setActiveRoot] = useState<string | null>(null);

  useEffect(() => {
    if (roots.length > 0 && !activeRoot) {
      setActiveRoot(roots[0].id);
    }
  }, [roots, activeRoot]);

  useEffect(() => {
    if (activeRoot) {
      fetchItems(activeRoot);
    }
  }, [activeRoot]);

  const fetchItems = async (rootId: string) => {
    try {
      const data = await invoke<MediaItem[]>("get_media_items", { rootId });
      setItems(data);
    } catch (e) {
      console.error(e);
    }
  };

  const handlePlay = async (e: React.MouseEvent, item: MediaItem) => {
    e.stopPropagation();
    
    // Alert user if the file is missing/offline (Ref Issue #4)
    if (item.metadata_status === "missing") {
      alert(`"File Not Found"\n\nThis media file is currently offline.\n- If it's on an external drive, please connect it.\n- If you relocated the file, click 'Scan' in the sidebar to automatically re-detect it.`);
      return;
    }

    try {
      await invoke("play_media", { mediaItemId: item.id });
      // Refresh items to update watch status
      if (activeRoot) fetchItems(activeRoot);
    } catch (e: any) {
      alert("Failed to open media: " + e);
    }
  };

  const toggleWatchStatus = async (e: React.MouseEvent, item: MediaItem) => {
    e.stopPropagation();
    if (item.metadata_status === "missing") return;

    const nextStatus = item.watch_status === 'watched' ? 'unwatched' : 'watched';
    try {
      await invoke("update_watch_status", { mediaItemId: item.id, status: nextStatus });
      if (activeRoot) fetchItems(activeRoot);
    } catch (e) {
      console.error(e);
    }
  };

  const formatSize = (bytes: number) => {
    const gb = bytes / (1024 * 1024 * 1024);
    return gb > 1 ? `${gb.toFixed(1)} GB` : `${(bytes / (1024 * 1024)).toFixed(0)} MB`;
  };

  const formatDuration = (sec?: number) => {
    if (!sec) return "--:--";
    const h = Math.floor(sec / 3600);
    const m = Math.floor((sec % 3600) / 60);
    if (h > 0) return `${h}h ${m}m`;
    return `${m}m`;
  };

  return (
    <div className="animate-fade-in">
      {roots.length > 0 && (
        <div className="tab-container" style={{ display: 'flex', gap: '1rem', marginBottom: '2rem' }}>
          {roots.map(root => (
            <button 
              key={root.id}
              className={`glass-button ${activeRoot === root.id ? 'primary' : ''} ${root.root_status === 'missing' ? 'offline-root-btn' : ''}`}
              onClick={() => setActiveRoot(root.id)}
            >
              <span className={`status-dot ${root.root_status === 'missing' ? 'offline' : 'online'}`}></span>
              {root.label}
            </button>
          ))}
        </div>
      )}

      {roots.length === 0 || !activeRoot ? (
        <div className="walkthrough-container animate-fade-in">
          <div className="walkthrough-hero">
            <h1 className="walkthrough-title">Welcome to MyMediaTrail 🎥</h1>
            <p className="walkthrough-subtitle">Your local-first, path-resilient, offline media manager. Let's get your library configured in 3 simple steps:</p>
          </div>
          
          <div className="walkthrough-steps">
            <div className="walkthrough-card">
              <div className="step-badge">
                <Plus size={18} />
              </div>
              <h3 className="step-title">1. Add a Location</h3>
              <p className="step-desc">
                Click the <strong style={{color:'var(--accent-hover)'}}>+</strong> button in the sidebar under <strong>Locations</strong>. Give it a name (e.g., "Movies") and paste its absolute path.
              </p>
            </div>
            
            <div className="walkthrough-card">
              <div className="step-badge">
                <Flame size={18} />
              </div>
              <h3 className="step-title">2. Rescan Folder</h3>
              <p className="step-desc">
                Click the <strong style={{color:'var(--accent-hover)'}}>Scan</strong> button next to your new location. The scanner will index files, extract metadata, and calculate unique hashes.
              </p>
            </div>
            
            <div className="walkthrough-card">
              <div className="step-badge">
                <Play size={18} />
              </div>
              <h3 className="step-title">3. Play & Track</h3>
              <p className="step-desc">
                Click any indexed item in your library. It launches in your default system player and automatically increments play counts locally!
              </p>
            </div>
          </div>

          <div className="walkthrough-features-box glass-panel">
            <h4 style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', marginBottom: '0.75rem', color: 'var(--accent-hover)' }}>
              <HardDrive size={18} /> Truly Portable & Secure
            </h4>
            <ul>
              <li><strong>Zero Cloud Tracking</strong>: All states and histories are stored in a local SQLite database (<code>mymediatrail.db</code>) inside the folder where the application was launched.</li>
              <li><strong>Path Resilience</strong>: Relocate directories or change USB slots; MyMediaTrail dynamically auto-heals broken paths.</li>
              <li><strong>Deduplication</strong>: Find identical duplicate files by content hashes in the <strong>Duplicates</strong> tab.</li>
            </ul>
          </div>
        </div>
      ) : (
        <div className="media-grid">
          {items.map(item => {
            const isOffline = item.metadata_status === "missing";
            
            return (
              <div 
                key={item.id} 
                className={`media-card ${isOffline ? 'offline' : ''}`} 
                onClick={(e) => handlePlay(e, item)}
                title={isOffline ? "File is offline. Drive disconnected or path changed." : "Click to Play"}
              >
                <div className="card-thumbnail">
                  <MediaThumbnail itemId={item.id} isOffline={isOffline} />
                  
                  {isOffline ? (
                    <div className="status-badge status-missing">
                      OFFLINE
                    </div>
                  ) : (
                    <div className={`status-badge status-${item.watch_status}`}>
                      {item.watch_status.replace('_', ' ')}
                    </div>
                  )}
                </div>
                <div className="card-content">
                  <div className="card-title" title={item.relative_path}>
                    {item.relative_path.split(/[/\\]/).pop()}
                  </div>
                  <div className="card-meta">
                    <span>{formatDuration(item.duration_sec)}</span>
                    <span>{formatSize(item.size_bytes)}</span>
                  </div>
                  <div style={{ marginTop: '0.75rem', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <span style={{ fontSize: '0.75rem', color: 'var(--text-secondary)' }}>
                      {item.resolution_text || "Unknown"}
                    </span>
                    <button 
                      className={`scan-btn ${isOffline ? 'disabled' : ''}`} 
                      onClick={(e) => toggleWatchStatus(e, item)}
                      disabled={isOffline}
                    >
                      Mark {item.watch_status === 'watched' ? 'Unwatched' : 'Watched'}
                    </button>
                  </div>
                </div>
              </div>
            );
          })}
          {items.length === 0 && (
            <div className="empty-state-card" style={{ gridColumn: '1 / -1', padding: '3rem', textAlign: 'center', color: 'var(--text-secondary)', background: 'var(--bg-surface)', borderRadius: 'var(--radius-md)', border: '1px dashed var(--border-color)' }}>
              No media items found. Click 'Scan' in the sidebar to index this folder.
            </div>
          )}
        </div>
      )}
    </div>
  );
}
