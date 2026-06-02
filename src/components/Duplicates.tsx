import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check } from "lucide-react";
import type { DuplicateGroup } from "../types";

export function Duplicates() {
  const [groups, setGroups] = useState<DuplicateGroup[]>([]);

  useEffect(() => {
    fetchDuplicates();
  }, []);

  const fetchDuplicates = async () => {
    try {
      const data = await invoke<DuplicateGroup[]>("get_duplicate_groups");
      setGroups(data);
    } catch (e) {
      console.error(e);
    }
  };

  const handleMerge = async (preferredId: string, discardedIds: string[]) => {
    if (!confirm("Are you sure you want to merge and move the other files to the Trash?")) return;
    try {
      await invoke("merge_and_clean", { preferredId, discardedIds });
      fetchDuplicates();
    } catch (e) {
      alert("Error: " + e);
    }
  };

  const formatSize = (bytes: number) => `${(bytes / (1024 * 1024)).toFixed(0)} MB`;

  if (groups.length === 0) {
    return <div style={{ color: 'var(--text-secondary)' }}>No duplicates found. Your library is clean!</div>;
  }

  return (
    <div className="animate-fade-in list-view">
      {groups.map(group => (
        <div key={group.hash} className="duplicate-group">
          <div className="duplicate-group-header">
            Identical Content (Hash: {group.hash.substring(0, 8)}...) • {group.items.length} copies
          </div>
          
          <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
            {group.items.map(item => {
              const others = group.items.filter(i => i.id !== item.id).map(i => i.id);
              
              return (
                <div key={item.id} className="list-item">
                  <div style={{ display: 'flex', flexDirection: 'column' }}>
                    <span style={{ fontWeight: 500, fontSize: '0.95rem' }}>{item.relative_path}</span>
                    <span style={{ color: 'var(--text-secondary)', fontSize: '0.8rem' }}>
                      Size: {formatSize(item.size_bytes)}
                    </span>
                  </div>
                  
                  <div>
                    <button 
                      className="glass-button primary" 
                      onClick={() => handleMerge(item.id, others)}
                      title="Keep this one, move others to Trash"
                    >
                      <Check size={16} /> Keep & Clean Others
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      ))}
    </div>
  );
}
