import test from 'node:test';
import assert from 'node:assert/strict';
import {verifyArchitecture} from './architecture.mjs';

function fixture() {
  const names = ['vesta-gpui', 'vesta-gpui-widgets', 'vesta-gpui-winit',
    'vesta-gpui-vulkan', 'vesta-gpui-webview', 'vesta-gpui-workspace',
    'vesta-workspace', 'vesta-viewport'];
  return {version: 1, workspace_members: [...names], packages: names.map(name =>
    ({id: name, name, targets: [{kind: ['lib']}], features: {}, dependencies: []})),
    resolve: {nodes: names.map(id => ({id, features: [], deps: []}))}};
}
function edge(metadata, from, to, kind = null, extra = {}) {
  metadata.packages.find(pkg => pkg.id === from).dependencies.push({name: to, kind, optional: false, target: null, ...extra});
  metadata.resolve.nodes.find(node => node.id === from).deps.push({name: extra.rename ?? to,
    pkg: to, dep_kinds: [{kind, target: extra.target ?? null}]});
}
function external(metadata, name) {
  metadata.packages.push({id: name, name, targets: [{kind: ['lib']}], dependencies: [], features: {}});
  metadata.resolve.nodes.push({id: name, features: [], deps: []});
}

test('production ownership rejects hidden native and reverse paths without treating test-only paths as production', () => {
  const valid = fixture();
  edge(valid, 'vesta-gpui-widgets', 'vesta-gpui');
  external(valid, 'winit');
  edge(valid, 'vesta-gpui-winit', 'winit');
  edge(valid, 'vesta-gpui', 'winit', 'dev');
  assert.equal(verifyArchitecture(valid).status, 'passed');

  const reversed = fixture();
  edge(reversed, 'vesta-gpui', 'vesta-gpui-widgets', null,
    {optional: true, target: 'cfg(windows)', rename: 'helper'});
  assert.throws(() => verifyArchitecture(reversed), /architecture_dependency_direction/);

  for (const kind of [null, 'build']) {
    const hidden = fixture();
    external(hidden, 'font-helper'); external(hidden, 'winit');
    edge(hidden, 'vesta-gpui', 'font-helper', kind);
    edge(hidden, 'font-helper', 'winit');
    assert.throws(() => verifyArchitecture(hidden), /architecture_native_leak.*font-helper -> winit/);
  }
  const hiddenUi = fixture();
  external(hiddenUi, 'model-helper');
  edge(hiddenUi, 'vesta-workspace', 'model-helper');
  edge(hiddenUi, 'model-helper', 'vesta-gpui');
  assert.throws(() => verifyArchitecture(hiddenUi), /architecture_dependency_direction/);
  const productLeak = fixture();
  external(productLeak, 'bridge'); external(productLeak, 'vesta-graph');
  edge(productLeak, 'vesta-gpui', 'bridge');
  edge(productLeak, 'bridge', 'vesta-graph');
  assert.throws(() => verifyArchitecture(productLeak), /architecture_product_dependency/);
  const incomplete = fixture();
  incomplete.packages[0].features = {platform_bridge: []};
  assert.throws(() => verifyArchitecture(incomplete), /architecture_features_incomplete/);
  assert.throws(() => verifyArchitecture({...fixture(), resolve: null}), /architecture_metadata_required/);
});
