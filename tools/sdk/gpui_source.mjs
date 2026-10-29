// Maintained-source integrity for this independent repository; no monorepo export.
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { sdkInventoryFiles } from './gpui_inventory.mjs';

const version = '0.1.0';
const candidate = '0.1.0-rc.1';
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const readJson = name => JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(readFileSync(name)));
const validPath = name => typeof name === 'string' && name !== '' &&
  !name.includes('\\') && !name.includes('\0') && !path.posix.isAbsolute(name) &&
  !name.includes(':') && !name.split('/').some(part => part === '' || part === '.' || part === '..') &&
  !['.git', 'target', 'SOURCE.json'].includes(name.split('/')[0]);
const tree = entries => sha(entries.map(e => `${e.path}\0${e.sha256}\0${e.bytes}\n`).join(''));
const inventory = directory => sdkInventoryFiles(directory, { allowGit: true, allowTarget: true })
  .filter(name => name !== 'SOURCE.json').sort();
function sourceFiles(directory) {
  return inventory(directory).map(name => {
    if (!validPath(name)) throw new Error('Invalid source path');
    const bytes = readFileSync(path.join(directory, name));
    return { path: name, bytes: bytes.length, sha256: sha(bytes) };
  });
}
export function seal(directory) {
  const origin = readJson(path.join(directory, 'IMPORT.json'));
  const manifest = new TextDecoder('utf-8', { fatal: true }).decode(readFileSync(path.join(directory, 'Cargo.toml')));
  if (!/^version = "0\.1\.0"$/m.test(manifest) || origin.schema_version !== 1) {
    throw new Error('Unexpected source family');
  }
  const files = sourceFiles(directory);
  const receipt = {
    schema_version: 2, sdk_version: version, candidate, status: 'candidate',
    authority: 'independently-maintained-source', immutable_git_revision: null,
    import_receipt_sha256: origin.original_source_receipt_sha256,
    tree_sha256: tree(files), files
  };
  writeFileSync(path.join(directory, 'SOURCE.json'), JSON.stringify(receipt, null, 2) + '\n');
  return { status: 'sealed_candidate', files: files.length, tree_sha256: receipt.tree_sha256 };
}
export function verify(directory) {
  const receipt = readJson(path.join(directory, 'SOURCE.json'));
  if (receipt.schema_version !== 2 || receipt.sdk_version !== version ||
      receipt.candidate !== candidate || receipt.status !== 'candidate' ||
      receipt.authority !== 'independently-maintained-source' || !Array.isArray(receipt.files)) {
    throw new Error('Unsupported maintained-source receipt');
  }
  for (const entry of receipt.files) {
    if (!validPath(entry.path) || !Number.isSafeInteger(entry.bytes) || entry.bytes < 0 ||
        !/^[0-9a-f]{64}$/.test(entry.sha256)) throw new Error('Invalid source entry');
  }
  const names = receipt.files.map(entry => entry.path);
  if (new Set(names).size !== names.length || JSON.stringify(names) !== JSON.stringify([...names].sort()) ||
      JSON.stringify(inventory(directory)) !== JSON.stringify(names)) {
    throw new Error('Source inventory changed');
  }
  for (const entry of receipt.files) {
    const bytes = readFileSync(path.join(directory, entry.path));
    if (bytes.length !== entry.bytes || sha(bytes) !== entry.sha256) throw new Error(`Source content changed: ${entry.path}`);
  }
  if (tree(receipt.files) !== receipt.tree_sha256) throw new Error('Source tree identity changed');
  const origin = readJson(path.join(directory, 'IMPORT.json'));
  if (origin.original_source_receipt_sha256 !== receipt.import_receipt_sha256) throw new Error('Import identity changed');
  return { status: 'verified_candidate', files: names.length, tree_sha256: receipt.tree_sha256 };
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [mode, directory, extra] = process.argv.slice(2);
  if (!['--seal', '--verify'].includes(mode) || !directory || extra) {
    console.error('Usage: node tools/sdk/gpui_source.mjs --seal|--verify <directory>');
    process.exitCode = 2;
  } else {
    try { console.log(JSON.stringify((mode === '--seal' ? seal : verify)(path.resolve(directory)))); }
    catch (error) { console.error(error.message); process.exitCode = 1; }
  }
}
