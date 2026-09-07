// The NSIS bundle name is derived from productName, and the product must stay
// "boardown" — so the Tauri shell's installer carries its own marker instead:
// `boardown_<version>_<arch>-tauri-setup.exe`, distinguished from the Electron
// build while keeping the standard `-setup.exe` ending.
import { readdirSync, renameSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const bundleDir = path.resolve(here, '..', 'src-tauri', 'target', 'release', 'bundle', 'nsis');

for (const name of readdirSync(bundleDir)) {
  if (!name.endsWith('-setup.exe')) continue;
  const renamed = name.replace('-setup.exe', '-tauri-setup.exe');
  renameSync(path.join(bundleDir, name), path.join(bundleDir, renamed));
  console.log(`renamed ${name} -> ${renamed}`);
}
