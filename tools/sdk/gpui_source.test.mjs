import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { seal, verify } from './gpui_source.mjs';

test('distribution inventory rejects changed, missing, added and escaping source while ignoring generated artifacts', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'vesta-source-contract-'));
  try {
    writeFileSync(path.join(root, 'Cargo.toml'), '[workspace.package]\nversion = "0.1.0"\n');
    writeFileSync(path.join(root, 'IMPORT.json'), JSON.stringify({ schema_version: 1, original_source_receipt_sha256: '1'.repeat(64) }));
    writeFileSync(path.join(root, 'source.rs'), 'pub fn preserved_contract() {}\n');
    seal(root);
    const original = readFileSync(path.join(root, 'SOURCE.json'));
    assert.equal(verify(root).status, 'verified_candidate');
    mkdirSync(path.join(root, 'target')); writeFileSync(path.join(root, 'target', 'generated'), 'excluded');
    mkdirSync(path.join(root, '.git')); writeFileSync(path.join(root, '.git', 'config'), 'excluded');
    assert.equal(verify(root).status, 'verified_candidate');
    writeFileSync(path.join(root, 'source.rs'), 'changed');
    assert.throws(() => verify(root), /Source content changed/);
    rmSync(path.join(root, 'source.rs'));
    assert.throws(() => verify(root), /Source inventory changed/);
    writeFileSync(path.join(root, 'source.rs'), 'pub fn preserved_contract() {}\n');
    writeFileSync(path.join(root, 'unexpected'), 'extra');
    assert.throws(() => verify(root), /Source inventory changed/);
    rmSync(path.join(root, 'unexpected'));
    const hostile = JSON.parse(original);
    hostile.files[0].path = '../outside';
    writeFileSync(path.join(root, 'SOURCE.json'), JSON.stringify(hostile));
    assert.throws(() => verify(root), /Invalid source entry/);
    hostile.files[0].path = 'C:/outside';
    writeFileSync(path.join(root, 'SOURCE.json'), JSON.stringify(hostile));
    assert.throws(() => verify(root), /Invalid source entry/);
    writeFileSync(path.join(root, 'SOURCE.json'), original);
    assert.equal(verify(root).status, 'verified_candidate');
  } finally {
    assert.equal(path.dirname(path.resolve(root)), path.resolve(os.tmpdir()));
    assert.ok(path.basename(root).startsWith('vesta-source-contract-'));
    rmSync(root, { recursive: true, force: true });
  }
});
