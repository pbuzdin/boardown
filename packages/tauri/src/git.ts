import { invoke } from '@tauri-apps/api/core';
import {
  readTaskCommits,
  type GitHistoryReader,
  type GitRun,
  type GitRunResult,
} from '@boardown/core';

// The host's whole share of the feature: run git and report what happened.
// Every decision about what the answer means lives in `readTaskCommits` in
// core, so no shell can classify a git answer differently.
const run: GitRun = async (args) => {
  try {
    return await invoke<GitRunResult>('git_run', { args });
  } catch {
    return { kind: 'unavailable' };
  }
};

export const gitHistory: GitHistoryReader = {
  commitsForTask: (taskId: string) => readTaskCommits(taskId, run),
};
