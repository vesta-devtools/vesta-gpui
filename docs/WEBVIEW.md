# Native WebView carrier

`vesta-gpui-webview` is an optional native child browser, currently Windows
WebView2 through Wry. It owns browser adaptation and bounded read-only resources.
The Host owns placement, permission policy, author actions and installed app
trust. It is not another GPUI renderer or author runtime. Non-Windows creation
returns `web_platform_unavailable`; portable Rust checks do not qualify a native
browser on another platform.

## Public surface and ownership

| Surface | Contract and caller responsibility |
| --- | --- |
| `ViewOwner`, `WebEvent` | Host-supplied view/generation tags are captured at creation. They are not an identity allocator or authority check. Reject stale events in the Host; change identity by disposing/recreating the child. |
| `EventKind`, `EventSink` | Loading/loaded URLs, titles and messages are notifications. The sink must enqueue quickly; never block the native event thread or treat page messages as approved author commands. |
| `Page::Html`, `Url`, `Assets` | Temporary private/incognito preview. HTML receives a local resource origin; URL previews admit HTTPS or explicit loopback HTTP. Asset pages use the given directory capability. |
| `Page::Application`, `AppProfile` | Host-approved installed content has its own package origin and durable data directory. `validate` checks ID/path syntax, not ownership, trust, signature or exclusive use of that directory. The Host supplies a private absolute root and stable ID. |
| `WebView::create` | Synchronous native creation on the event thread; initially hidden. Parent outlives child. Keep the supplied retirement group even when creation fails. |
| `sync`, `visible` | Derived native bounds/visibility cache. Positive finite bounds are required to show. Hide, move, show effects are sequenced; each success is retained across a later error. The Host decides overlay occlusion. |
| `navigate`, `reload`, `evaluate`, `focus_parent` | Native requests, not completion promises. Explicit navigation validates preview URLs and is rejected for installed apps. Evaluate runs only Host-supplied scripts; it does not grant author permission. |
| `WebRetirement` | CPU-only cloneable observation group for workers created with that group, including failed native creation. It owns no window, sender, profile or executor. `pending` observes outstanding work; `observe` returns completion, first failure, or an observation timeout. |
| `WebError` | Stable `code`, explanatory `message`, standard Display/Error. Underlying native/I/O failures are formatted messages, not a preserved typed source chain. Match codes, not message wording. |
| `AssetRoot`, `Resource`, `MAX_RESOURCE_BYTES` | A shared open directory capability and owned bytes/MIME. `relative_entry` converts an existing selected file; `read` accepts a percent-encoded request path; `read_relative` preserves a literal filename and caller read budget. Run filesystem calls on an I/O worker. |
| `validate_url` | Returns a normalized admitted URL or structured failure. No credentials, arbitrary file URLs, JavaScript URLs or non-loopback HTTP. |

There is one authoritative owner for each state: the Host for event identity and
placement intent, `WebView` for successful native synchronization observations,
the native child for browser state, the per-view I/O worker for its resource
capability and temporary profile, and the retirement group for completion/error
observations only. No mutable identity copy or second browser controller.

## Resource and permission limits

URLs and encoded resource paths are limited to 8 KiB; HTML/files to 32 MiB;
Host scripts to 64 KiB; IPC bodies to 32 KiB. Titles are truncated to 256
characters. File reads reject unsupported extensions and paths escaping the
directory capability, including open-time symlink resolution. Literal `%` names
are escaped before browser URL construction and decoded once on resource reads.

The resource worker uses a bounded 32-entry nonblocking queue. GET and HEAD are
admitted; HEAD omits response bytes but currently reads the file. Other methods
return 405, unavailable resources 404, a closed/full/unavailable queue 503.
Responses set `nosniff` and `no-store`. After close, the worker completes any
current file read, discards queued work, releases its capability and retires its
SDK-created temporary profile. There is no arbitrary filesystem cancellation.

Native permission requests, downloads and new windows are denied. Installed
apps admit only their package origin (plus initial `about:blank`) and receive a
same-origin CSP; previews can navigate to admitted network URLs. A read-only
asset mount and page IPC do not make remote content trusted. Browser-native
child pixels are outside GPUI scene capture. Browser runtime storage, networking
and process limits are not supplied by the resource queue budget.

## Shutdown

Create one `WebRetirement::default()` group for a Host run and pass its reference
as the fifth `WebView::create` argument. Dispose children on their event thread
before parents. Closing signals the existing per-view resource worker; it never
waits for disk deletion on that thread and adds no separate cleanup executor.
After the event loop returns, call `group.observe(budget)` before process exit.
SDK temporary profiles are retried for up to ten seconds after native disposal;
individual filesystem operations are not preempted. Host application profiles
are never deleted.

Zero-budget observation does not wait. A timeout does not terminate a worker or
prove cleanup failure; retain the group and observe again. First cleanup/worker
interruption failure remains recorded. Dropping a group or exiting a process
early is not a drain. Concurrent creation must end before final observation;
the group is not a Host shutdown gate. Successful retirement proves this SDK's
I/O ownership ended, not that every browser process exited; process owners must
also verify their actual children when that is part of acceptance.

## Autonomous consumer and evidence boundary

Run `cargo run -p vesta-gpui-standalone --example webview_lifecycle --
--fixture-dir=<new-directory>` on Windows. This finite public-API consumer creates
local HTML, literal-percent/Unicode asset paths and an installed app. It checks
real IPC origins, GET/HEAD/POST, reload, generation tags, durable storage and
child-before-parent retirement without user input, renderer or account. It
refuses an existing fixture directory and retains its Host-owned app profile.

The family review also covers existing Agent/Builder callers and focused
partial-native-effect, failed-creation and locked-profile contracts. Controlled
native-operation errors are boundary tests, not claims of a reproduced COM
failure. Actual process/profile qualification is recorded in the product
handoff. Preview API migration is in the [candidate notes](../release/0.1.0-rc.2.md);
immutable product adoption remains a separate exit in [the programme](REFINEMENT.md).
