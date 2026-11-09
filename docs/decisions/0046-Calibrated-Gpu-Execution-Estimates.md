# Calibrated GPU execution estimates

Status: Accepted; implementation in progress under IN02/IN03 of decision 0043.
Date: 2026-11-10.

Enable a supported Vulkan KHR/EXT calibrated-timestamp device extension without
making it a renderer requirement. On Windows, query the advertised device/QPC
domains through the existing checked Vulkan boundary. Ordinary rendering and
non-traced profile streams perform no timestamp calibration calls. Calibrate
each completed traced submission; do not reuse an unbounded-age offset.

The renderer's Windows-only `windows-sys` dependency enables solely
`Win32_System_Performance`: QueryPerformanceCounter/Frequency bridge the QPC
domain required by the Vulkan extension. These calls stay inside the existing
private checked-pipeline FFI boundary. The dependency guard pins that narrow
target/feature allowance; it still rejects native host, input and author-model
dependencies. No window, monitor, event-loop or clipboard ownership moves out
of the native adapter, and the eight-library dependency direction is unchanged.

Keep raw device/host calibration ticks, QPC frequency, host bracketing ticks,
capture-relative CPU bracketing times and the driver's reported maxDeviation.
Validate counter wrapping, arithmetic, capability, capture ownership and physical
recording/completion bounds. Failed or unsupported calibration preserves the
original submission duration and explicitly lacks CPU execution placement.
CPU recording time remains an association and never substitutes for execution.

Project estimates into the CPU elapsed-time domain with the calibrated pair and
the device timestamp period. Report the anchor's conservative driver/host-bridge
uncertainty and distance from the anchor separately. Vulkan does not provide a
universal extrapolation drift bound; do not describe anchor uncertainty as a
guaranteed error bound for all mapped marks. Display/query placements as estimates,
retain original evidence keys and expose unknown clock coverage to Agents.

The new GPU extension version remains compatible with existing CPU-v2/GPU-v1
captures. Native GPU tracks and typed queries share immutable placement evidence.
Range filtering of execution estimates must disclose its estimated semantics;
existing CPU-context submission queries retain their meaning. These timestamps
still do not measure presentation/display latency or prove CPU/GPU causality.

Reference: Khronos [calibrated timestamp API](https://docs.vulkan.org/refpages/latest/refpages/source/vkGetCalibratedTimestampsKHR.html)
and [device / QPC time domains](https://docs.vulkan.org/refpages/latest/refpages/source/VkTimeDomainKHR.html).
