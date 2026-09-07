import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { App, useBoardStore } from '@boardown/ui';
import type { Theme } from '@boardown/core';
import { useEffect, useRef, useState } from 'react';
import { fs, projectFiles } from './adapter';
import { gitHistory } from './git';
import { folderName, suggestIdPrefix } from './project-name';
import styles from './Root.module.css';

export function Root() {
  const [folder, setFolder] = useState<string | null>(null);
  const [theme, setTheme] = useState<Theme>(() =>
    window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light',
  );

  useEffect(() => {
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    const onChange = (): void => setTheme(mq.matches ? 'dark' : 'light');
    mq.addEventListener('change', onChange);
    return () => mq.removeEventListener('change', onChange);
  }, []);

  // With no board mounted, @boardown/ui's theme.css isn't loaded; keep the
  // shell palette in sync with the OS theme. Once App mounts it drives
  // data-theme from the forced theme below.
  useEffect(() => {
    if (folder === null) {
      document.documentElement.setAttribute('data-theme', theme);
    }
  }, [folder, theme]);

  // The host signals when .boardown/ changed on disk outside this window
  // (git, the CLI, another editor); refresh in place via reloadSilent.
  useEffect(() => {
    const unlisten = listen('board-changed', () => void useBoardStore.getState().reloadSilent());
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);

  const openFolder = (): void => {
    void invoke<string | null>('open_folder').then((picked) => {
      if (picked !== null) setFolder(picked);
    });
  };

  // App's defaultTheme only seeds a brand-new board's onboarding; it must stay
  // stable for each mount, or an OS theme change would re-fire App's load
  // effect and reload the board mid-edit. Re-capture the current theme only
  // when the board changes (App is keyed by folder).
  const openThemeRef = useRef(theme);
  const openFolderRef = useRef(folder);
  if (openFolderRef.current !== folder) {
    openFolderRef.current = folder;
    openThemeRef.current = theme;
  }

  return folder === null ? (
    <div className={styles.empty}>
      <p className={styles.emptyTitle}>No project open</p>
      <p className={styles.emptyHint}>Open a folder to load its board.</p>
      <button type="button" className={styles.emptyButton} onClick={openFolder}>
        Open Folder…
      </button>
    </div>
  ) : (
    <App
      key={folder}
      fs={fs}
      projectFiles={projectFiles}
      gitHistory={gitHistory}
      forcedTheme={theme}
      defaultTheme={openThemeRef.current}
      defaultProjectName={folderName(folder)}
      defaultIdPrefix={suggestIdPrefix(folderName(folder))}
      onCancel={() => {
        // Always return to the welcome screen, even if the host fails to
        // clear its board context.
        const reset = (): void => setFolder(null);
        void invoke('close_folder').then(reset, reset);
      }}
    />
  );
}
