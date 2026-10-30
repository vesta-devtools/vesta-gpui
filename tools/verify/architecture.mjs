// Production boundary policy; Cargo remains the dependency authority.
import fs from 'node:fs';
import {pathToFileURL} from 'node:url';

const allowed = {
  'vesta-gpui': [],
  'vesta-gpui-widgets': ['vesta-gpui'],
  'vesta-gpui-winit': ['vesta-gpui'],
  'vesta-gpui-vulkan': ['vesta-gpui', 'vesta-viewport'],
  'vesta-gpui-webview': ['vesta-gpui'],
  'vesta-gpui-workspace': ['vesta-gpui', 'vesta-gpui-widgets', 'vesta-workspace'],
  'vesta-workspace': [],
  'vesta-viewport': [],
};
const native = new Set(['winit', 'vulkano', 'wgpu', 'wry']);
const nativeAllowed = {
  'vesta-gpui-winit': new Set(['winit']),
  'vesta-gpui-vulkan': new Set(['winit', 'vulkano']),
  'vesta-gpui-webview': new Set(['wry']),
};
const production = kind => kind === null || kind === 'build';
const fail = (code, detail) => {throw new Error(`${code}: ${detail}`);};

export function verifyArchitecture(metadata) {
  if (metadata?.version !== 1 || !Array.isArray(metadata.packages)
      || !Array.isArray(metadata.workspace_members) || !Array.isArray(metadata.resolve?.nodes)) {
    fail('architecture_metadata_required', 'use cargo metadata --locked --format-version 1 --all-features');
  }
  const packages = new Map(metadata.packages.map(pkg => [pkg.id, pkg]));
  const nodes = new Map(metadata.resolve.nodes.map(node => [node.id, node]));
  const members = metadata.workspace_members.map(id => packages.get(id));
  if (members.some(pkg => !pkg)) fail('architecture_member_missing', 'workspace package is unresolved');
  const libraries = members.filter(pkg => pkg.targets.some(t => t.kind.includes('lib')));
  if (libraries.length !== Object.keys(allowed).length
      || libraries.some(pkg => !Object.hasOwn(allowed, pkg.name))) {
    fail('architecture_library_set_changed', 'review library ownership before adding, removing or replacing a package');
  }
  const family = new Map(libraries.map(pkg => [pkg.name, pkg]));
  if (family.size !== libraries.length) fail('architecture_duplicate_owner', 'library names must be unique');
  const memberIds = new Set(metadata.workspace_members);
  const internalEdges = [];
  for (const pkg of libraries) {
    const reachableFamily = new Set();
    const visitPolicy = name => {
      for (const next of allowed[name]) {
        if (reachableFamily.has(next)) continue;
        reachableFamily.add(next); visitPolicy(next);
      }
    };
    visitPolicy(pkg.name);
    const node = nodes.get(pkg.id);
    if (!node) fail('architecture_node_missing', pkg.name);
    for (const feature of Object.keys(pkg.features)) {
      if (!node.features.includes(feature)) fail('architecture_features_incomplete', `${pkg.name}/${feature}`);
    }
    // Cargo declarations include inactive optional and target-specific edges too.
    for (const dep of pkg.dependencies.filter(dep => production(dep.kind))) {
      if (family.has(dep.name)) {
        if (!allowed[pkg.name].includes(dep.name)) {
          fail('architecture_dependency_direction', `${pkg.name} -> ${dep.name}`);
        }
        internalEdges.push({from: pkg.name, to: dep.name, optional: dep.optional, target: dep.target});
      } else if (dep.name.startsWith('vesta-') || dep.name.startsWith('developer-agent')) {
        fail('architecture_product_dependency', `${pkg.name} -> ${dep.name}`);
      }
    }
    const pending = [{id: pkg.id, via: [pkg.name]}];
    const seen = new Set();
    while (pending.length) {
      const {id, via} = pending.pop();
      if (seen.has(id)) continue;
      seen.add(id);
      const current = packages.get(id), resolved = nodes.get(id);
      if (!current || !resolved) fail('architecture_dependency_unresolved', via.join(' -> '));
      if (id !== pkg.id) {
        if (memberIds.has(id) && !family.has(current.name)) {
          fail('architecture_example_dependency', via.join(' -> '));
        }
        if ((current.name.startsWith('vesta-') || current.name.startsWith('developer-agent'))
            && !family.has(current.name)) {
          fail('architecture_product_dependency', via.join(' -> '));
        }
        if (/^(bevy(?:_|$)|qt(?:_|$)|tauri(?:_|$))/.test(current.name)
            || (native.has(current.name) && !nativeAllowed[pkg.name]?.has(current.name))) {
          fail('architecture_native_leak', via.join(' -> '));
        }
      }
      for (const dep of resolved.deps) {
        if (!dep.dep_kinds.some(kind => production(kind.kind))) continue;
        const next = packages.get(dep.pkg);
        if (!next) fail('architecture_dependency_unresolved', dep.pkg);
        if (family.has(next.name) && family.get(next.name).id !== next.id) {
          fail('architecture_duplicate_source', `${pkg.name} reaches a second ${next.name}`);
        }
        if (family.has(next.name) && !reachableFamily.has(next.name)) {
          fail('architecture_dependency_direction', [...via, next.name].join(' -> '));
        }
        pending.push({id: dep.pkg, via: [...via, next.name]});
      }
    }
  }
  return {schema_version: 1, status: 'passed', libraries: libraries.length,
    internal_edges: internalEdges, scope: 'resolved normal/build dependencies; all declared library features'};
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    if (process.argv.length !== 3) fail('architecture_usage', 'node tools/verify/architecture.mjs <cargo-metadata.json>');
    const text = new TextDecoder('utf-8', {fatal: true}).decode(fs.readFileSync(process.argv[2]));
    console.log(JSON.stringify(verifyArchitecture(JSON.parse(text))));
  } catch (error) {
    console.error(String(error));
    process.exitCode = 1;
  }
}
