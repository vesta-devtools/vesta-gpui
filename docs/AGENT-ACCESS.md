# Structured UI access for Agents

Standalone provides an explicitly enabled JSONL interface. It uses the same
committed Element, layout and action routes as the visible host. Normal startup
does not enable it, open a network service, or start an Agent/model session.

```powershell
cargo build --locked -p vesta-gpui-standalone
.\target\debug\vesta-gpui-standalone.exe --ui-query-stdio
# Start a separate owned instance with this flag when control is authorized:
.\target\debug\vesta-gpui-standalone.exe --ui-control-stdio
```

The caller owns the launched process's stdin/stdout. Query mode is read-only;
control mode additionally exposes admitted commands, logical input and capture.
It does not attach to an already running user's window. Close owned instances
normally after use. Human logs use stderr; stdout contains JSON replies only.

The explicitly started transport is process-lived: it exclusively locks stdin
and stdout until EOF, input/output failure, oversized input or a lost ordinary
host reply. Start it once per native host process. It has no cancellation/join
handle; window closure does not cancel a blocked stdin read. Embedded controllers
reuse the containing host's connection. An ordinary request waits up to ten
seconds for its host response. Dispatch must enqueue promptly on the host's
existing event-loop proxy, and response serialization must be nonblocking.

Read-only `wait` has a default 1,000 ms budget, configurable from 1 to 5,000 ms.
Polls, reply waits and retry sleeps share one deadline. Budget exhaustion returns
`wait_timeout` and keeps the transport usable; a disconnected host reply returns
`host_unavailable` and retires it. Each poll owns its reply channel, so late
replies cannot satisfy a successor request. These budgets do not preempt host
callback work, serialization or a blocked stdout pipe. A query timeout does not
cancel a previously accepted host action.

## Query the presented state

Every line is a UTF-8 JSON object with `schema_version:1` and a 1–128-byte
`request_id`. Keep instance/window/revision/element/owner IDs as decimal strings;
64/128-bit identities must not round-trip through JavaScript Number.

```json
{"schema_version":1,"request_id":"q1","op":"windows"}
{"schema_version":1,"request_id":"q2","op":"snapshot","instance":"<instance>","window":"1","limit":128}
{"schema_version":1,"request_id":"q3","op":"changes","instance":"<instance>","window":"1","since":"<pinned revision>"}
{"schema_version":1,"request_id":"q4","op":"wait","instance":"<instance>","window":"1","after_revision":"<observed revision>","timeout_ms":3000}
```

Snapshots report roles/names, bounded text and values, focus/modal state,
selected/expanded/read-only/busy state, visible/clip geometry, scrolling and
action references. Native hosts publish snapshots from their last successfully
presented frame. Read subsequent pages with the returned `revision` and `after`
cursor. A snapshot has at most 4096 nodes, 128 levels and 64 KiB of text, with
4096 bytes per text field; query pages contain at most 256 nodes. Truncation and
incompleteness are explicit.

Only the realized UI tree is available. Missing rows are not proof that the
business model has no such data. Changes from retained revisions distinguish
removed realized nodes from incomplete capture. Visible bounds describe geometry,
not the OS compositor's final unobstructed view. Use the application's own typed
business API for Graphs, assets and authoring transactions.

## Discover commands before invoking

```json
{"schema_version":1,"request_id":"d1","op":"commands","instance":"<instance>","window":"1"}
{"schema_version":1,"request_id":"m1","op":"invoke","instance":"<instance>","window":"1","element":"<element>","owner":"<owner>","command":"activate"}
{"schema_version":1,"request_id":"m2","op":"invoke","instance":"<instance>","window":"1","element":"<editor>","owner":"<owner>","command":"replace_text","args":{"expected":"old text","text":"new text"}}
```

Use only the commands returned for that live element and its owner token.
Replacement, removal and relevant semantic changes invalidate old ownership.
Current disabled/modal state, editing preconditions and composition still gate
execution. `replace_text` checks the complete expected source; password editing
and sensitive commands are not automatically exposed.

The last 128 modification receipts are bounded in-memory retry protection.
Reusing a request ID requires identical parameters; a restarted process or an
evicted receipt is not persistent exactly-once execution. A completed synchronous
UI command is not proof of a presented frame or a finished asynchronous business
operation. Wait for a newer presentation and query the actual result.

The controller's loop is: query the current window; discover commands and fixed
source revision; check preconditions; invoke the intended action; wait and
re-query its effect. Planning, approval, file permissions, credentials and author
transactions remain Host responsibilities.

## Logical input and images

The `commands` response also describes the current `input` schema and action
coverage. `input` requires current owner/revision/geometry and uses the original
host event handlers. Pointer presses/releases must balance in one request;
ordinary logical input cannot stand in for a physical held gesture, system IME
candidate selection or native accessibility verification. Stale geometry, edits,
focus or pending presentation require fresh observation, not guessed coordinates.

```json
{"schema_version":1,"request_id":"c1","op":"capture","instance":"<instance>","window":"1","revision":"<pinned revision>","rect":{"x":0,"y":0,"width":320,"height":100}}
```

Without an explicit viewport, capture reads the next successful presentation's
GPU color target and returns `mode:next_presented` with the bound semantic
revision. An explicit viewport reflows the retained tree and returns
`mode:offscreen`; it is not the screen or a historical resource replay. Neither
mode loads missing business data. Rectangles are logical pixels. Output is
bounded to a 4096-pixel edge and 4,194,304 pixels, with one in-flight capture per
renderer. GPU completion is observed asynchronously.

Sensitive text is redacted from semantics and sensitive pixels are rejected.
System dialogs, title-bar compositor layers and native WebView child content
are outside the GPUI color target and ordinary GPUI command authority. This
interface does not grant general desktop, browser, approval or credential access.
