// Maintained source enumeration for the independent source seal and verifier.
import { lstatSync, readdirSync } from 'node:fs';
import path from 'node:path';

export function sdkInventoryFiles(directory, { allowTarget = false, allowGit = false } = {}, prefix = '') {
  return readdirSync(directory).sort().flatMap(name => {
    const relative = prefix ? `${prefix}/${name}` : name;
    const absolute = path.join(directory, name);
    const stat = lstatSync(absolute);
    if (stat.isSymbolicLink()) throw new Error(`SDK refuses links: ${relative}`);
    if (!prefix && name === 'target' && allowTarget) {
      if (!stat.isDirectory()) throw new Error('generated target must be a directory');
      return [];
    }
    if (!prefix && name === '.git' && allowGit && (stat.isDirectory() || stat.isFile())) return [];
    if (stat.isDirectory()) return sdkInventoryFiles(absolute, { allowTarget, allowGit }, relative);
    if (!stat.isFile()) throw new Error(`SDK refuses special files: ${relative}`);
    return [relative];
  });
}
