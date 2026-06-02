import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Trash2 } from "lucide-react";
import type { MediaItem } from "../types";

export function Cleanup() {
  const [items, setItems] = useState<MediaItem[]>([]);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  useEffect(() => {
    fetchSuggestions();
  }, []);

  const fetchSuggestions = async () => {
    try {
      const data = await invoke<MediaItem[]>("get_cleanup_suggestions");
      setItems(data);
    } catch (e) {
      console.error(e);
    }
  };

  const toggleSelect = (id: string) => {
    const next = new Set(selectedIds);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    setSelectedIds(next);
  };

  const toggleSelectAll = () => {
    if (selectedIds.size === items.length) {
      setSelectedIds(new Set());
    } else {
      setSelectedIds(new Set(items.map(i => i.id)));
    }
  };

  const handleDelete = async (permanently: boolean) => {
    if (selectedIds.size === 0) return;
    if (!confirm(`Are you sure you want to delete ${selectedIds.size} files?`)) return;
    
    try {
      await invoke("delete_media_items", { 
        mediaItemIds: Array.from(selectedIds), 
        permanently 
      });
      setSelectedIds(new Set());
      fetchSuggestions();
    } catch (e) {
      alert("Error: " + e);
    }
  };

  const formatSize = (bytes: number) => {
    const gb = bytes / (1024 * 1024 * 1024);
    return gb > 1 ? `${gb.toFixed(1)} GB` : `${(bytes / (1024 * 1024)).toFixed(0)} MB`;
  };

  const totalFreed = items
    .filter(i => selectedIds.has(i.id))
    .reduce((acc, i) => acc + i.size_bytes, 0);

  if (items.length === 0) {
    return <div style={{ color: 'var(--text-secondary)' }}>No watched items available to clean up.</div>;
  }

  return (
    <div className="animate-fade-in">
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '2rem' }}>
        <div style={{ color: 'var(--text-secondary)' }}>
          Select watched files to delete and free up space.
        </div>
        
        {selectedIds.size > 0 && (
          <div style={{ display: 'flex', gap: '1rem', alignItems: 'center' }}>
            <span style={{ color: 'var(--success-color)', fontWeight: 600 }}>
              Freeing {formatSize(totalFreed)}
            </span>
            <button className="glass-button" onClick={() => handleDelete(false)}>
              Move to Trash
            </button>
            <button className="glass-button danger" onClick={() => handleDelete(true)}>
              <Trash2 size={16} /> Delete Permanently
            </button>
          </div>
        )}
      </div>

      <div className="list-view">
        <div className="list-item" style={{ background: 'transparent', border: 'none', paddingBottom: 0 }}>
          <label style={{ display: 'flex', alignItems: 'center', gap: '1rem', cursor: 'pointer' }}>
            <input 
              type="checkbox" 
              checked={selectedIds.size > 0 && selectedIds.size === items.length}
              onChange={toggleSelectAll}
              style={{ width: '18px', height: '18px' }}
            />
            <span style={{ fontWeight: 600 }}>Select All</span>
          </label>
        </div>

        {items.map(item => (
          <div key={item.id} className="list-item">
            <label style={{ display: 'flex', alignItems: 'center', gap: '1rem', width: '100%', cursor: 'pointer' }}>
              <input 
                type="checkbox" 
                checked={selectedIds.has(item.id)}
                onChange={() => toggleSelect(item.id)}
                style={{ width: '18px', height: '18px' }}
              />
              <div style={{ display: 'flex', flexDirection: 'column' }}>
                <span style={{ fontWeight: 500, fontSize: '0.95rem' }}>{item.relative_path}</span>
                <span style={{ color: 'var(--text-secondary)', fontSize: '0.8rem' }}>
                  {formatSize(item.size_bytes)} • Played {item.play_count} times
                </span>
              </div>
            </label>
          </div>
        ))}
      </div>
    </div>
  );
}
