import { invoke } from '@tauri-apps/api/core';
import {
  classifyProjectFile,
  type FileStat,
  type FsAdapter,
  type FsEntry,
  type ProjectFileRead,
  type ProjectFileReader,
} from '@boardown/core';

// The wire shape of one FsAdapter operation, mirroring packages/electron's
// FsRequest: the host resolves every path against the picked project folder's
// .boardown/, so the renderer never names an absolute path.
interface FsRequest {
  method: 'read' | 'write' | 'list' | 'stat' | 'mkdir' | 'remove';
  path: string;
  content?: string;
}

class TauriFs implements FsAdapter {
  async read(path: string): Promise<string> {
    const content = await invoke<string | null>('fs_op', {
      req: { method: 'read', path } satisfies FsRequest,
    });
    // The host answers a missing file with null; FsAdapter.read rejects.
    if (content === null) throw new Error(`File not found: ${path}`);
    return content;
  }

  async write(path: string, content: string): Promise<void> {
    await invoke('fs_op', { req: { method: 'write', path, content } satisfies FsRequest });
  }

  async list(dir: string): Promise<FsEntry[]> {
    return await invoke<FsEntry[]>('fs_op', {
      req: { method: 'list', path: dir } satisfies FsRequest,
    });
  }

  async stat(path: string): Promise<FileStat | null> {
    return await invoke<FileStat | null>('fs_op', {
      req: { method: 'stat', path } satisfies FsRequest,
    });
  }

  async mkdir(dir: string): Promise<void> {
    await invoke('fs_op', { req: { method: 'mkdir', path: dir } satisfies FsRequest });
  }

  async remove(path: string): Promise<void> {
    await invoke('fs_op', { req: { method: 'remove', path } satisfies FsRequest });
  }
}

export const fs: FsAdapter = new TauriFs();

// Read-only access to the open project folder (the parent of .boardown/), for
// repo file links. Deliberately not part of `fs`: no write path may reach
// outside the board. The host returns raw bytes; the text/binary decision
// stays in core (classifyProjectFile).
export const projectFiles: ProjectFileReader = {
  async readFile(path: string): Promise<ProjectFileRead> {
    try {
      const bytes = await invoke<ArrayBuffer>('read_project_file', { path });
      return classifyProjectFile(new Uint8Array(bytes));
    } catch (err) {
      // The host's Err string is a classification tag (see project_file.rs);
      // anything unrecognized is the least specific kind.
      const tag = String(err);
      if (tag.includes('too-large')) return { kind: 'too-large' };
      if (tag.includes('not-found')) return { kind: 'not-found' };
      return { kind: 'unreadable' };
    }
  },
};
