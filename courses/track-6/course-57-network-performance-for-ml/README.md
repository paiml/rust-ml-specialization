# Course 57 — Network Performance for ML Systems

> **Track 6** · Anchored in: apr serve, apr profile, netprobe

## Overview

Almost everyone can run `iperf3` and read a number. Almost no one can answer the
next question: *is that number good, and if not, where is the ceiling — the NIC,
the MTU, the CPU/cipher, or a firewall?* This course teaches **network
performance as a diagnostic discipline** and points it squarely at the data
movement that dominates real ML systems.

ML is increasingly network-bound. The bottleneck is rarely your matmul; it is the
bytes that have to travel:

- **Moving model files fast** — pulling a multi-GB `safetensors` or GGUF
  checkpoint between machines, or staging an aprender model file onto a serving
  box. A 7 GB checkpoint at 620 MB/s is ~11 s; at wire speed it is ~6 s — and at
  fleet scale that gap is real money and real deploy latency.
- **Inference serving throughput and tail latency** — an `apr serve` endpoint
  under load is gated by connection RTT and per-stream goodput long before it is
  gated by the model. The same line-rate-vs-goodput reasoning explains a p99 that
  refuses to come down.
- **Distributed-training bandwidth** — gradient and parameter sync (all-reduce,
  parameter-server pulls) is network-bound at scale. The cipher/MTU/NIC triage
  framework is exactly how you find out whether a slow step is compute or fabric.
- **Data-loading pipelines** — a loader feeding a training loop from a remote
  store starves the GPU the instant the link cannot keep the batch queue full.

The spine of the course is one **real investigation** on a 10 GbE LAN, reused
verbatim in every module. Two hosts on the same switch (MTU 9000): `iperf3`
clocks **~9.9 Gbit/s**, yet `scp`/`rsync` top out near **620 MB/s**. The course
walks the exact reasoning that closes that gap — the link is fine, the link is
not the bottleneck, the single-core `chacha20-poly1305` cipher is; switching SSH
to AES-NI `aes256-gcm` reaches **~1.1 GB/s (~1.8x)**, and raw `nc` hits
**~1.2 GB/s (~9.6 Gbit/s)**, near wire speed. Along the way a `ufw` default-DROP
policy makes a test *hang* instead of fail — the canonical "is it me, the host,
or the path?" lesson.

The hands-on tool is **netprobe**, a contract-governed Rust probe that measures
TCP throughput and TCP-connect RTT. It is built **contract-first under provable
contracts** — the kernel invariants are specified *before* any code — which makes
this course a deliberate preview of Track 7 (Correctness).

This is a **reframe** of PAIML's "Network Performance Testing" course: the
rigorous diagnostic method is unchanged; the data it diagnoses is now ML data.

## Prerequisites

- Comfort in a Linux shell (run commands, read output, pipe, redirect).
- Basic TCP/IP literacy: IP addresses, ports, TCP vs UDP, what a packet is.
- SSH basics (keys, `~/.ssh/config`).
- Access to **two hosts that can talk to each other** — a home lab, two cloud VMs
  in the same placement group, or two containers (with a documented bandwidth
  caveat).
- Track 3 Rust fluency for the netprobe build track; Track 5 familiarity with
  `safetensors`/GGUF and `apr serve` makes the ML framing land harder.

## Learning Outcomes

After this course a learner can:

- Distinguish **bits vs bytes** and **line rate vs goodput**, and convert fluently
  (Gbit/s to MB/s and back), then judge whether an ML transfer number is good.
- Measure **latency and loss** with `ping` and interpret the RTT distribution
  (min/avg/max/mdev) — and tie tail RTT to an `apr serve` p99.
- Measure **throughput** with `iperf3` (single stream, `-R` reverse, `-P`
  parallel) and read sender-vs-receiver retransmits.
- Read the **hardware ceiling**: NIC link speed (`/sys/class/net`, `ethtool`),
  MTU and jumbo frames, and whether two hosts share a switch — then compute a
  link's theoretical max goodput.
- Identify the **binding constraint** when goodput < capacity: NIC, MTU,
  CPU/cipher, or firewall — and *prove* it with evidence, not assertion.
- Quantify **encryption overhead** and tune it (AES-NI `aes-gcm` vs software
  `chacha20`; pinning a cipher in `~/.ssh/config`) on a real checkpoint transfer.
- Choose the right **transport** for an ML artifact: encrypted (`scp`/`rsync`)
  vs raw (`nc` + `tar`) on a trusted LAN, and justify the trade-off.
- **Diagnose a hung transfer or a stalled serving endpoint** — default-DROP
  firewall, wrong port, MTU blackhole — systematically rather than by guessing.
- Map the framework onto ML workloads: checkpoint distribution, inference
  serving, distributed-training sync, and data-loading pipelines.
- *(Build track)* implement and run **netprobe**, a contract-governed Rust TCP
  throughput + RTT probe, and validate its numbers against `iperf3`.

## Structure

```text
Course → Module → Lesson (3–5 videos ≤ 6 min each) → Key Terms + Reflection
```

Each module ends with a **Critical Thinking Assessment** (quiz + role-play) and a
copy-paste lab against two hosts. The course ends with one **Graded Quiz**
(5 questions, 80% pass) and the Capstone. Every shell command in the labs was
**actually run** during the originating 10 GbE investigation, with the real
observed numbers in the comments.

## Modules

### Module 1 — The mental model: line rate vs goodput, for ML bytes

Bits vs bytes; Gbit/s vs MB/s; the difference between a link's **theoretical
capacity** and the **goodput an application actually gets**; and the
"where is the ceiling?" triage framework: **NIC → MTU → CPU/cipher → firewall**.
Grounded in the headline conversion: `620 MB/s × 8 = 4.96 Gbit/s` — only *half*
of a 10 GbE link, so the first question is "why only half?" Reframed in ML terms:
a 7 GB safetensors checkpoint at 620 MB/s vs at wire speed, and why that gap is
the thing standing between you and a fast deploy.

### Module 2 — Latency, loss, and serving tail latency

`ping`; reading `rtt min/avg/max/mdev`; recognizing when *latency*, not
bandwidth, is the real problem. On the case LAN: ~0.3 ms avg, 0% loss — a clean
same-switch path. ML reframe: an `apr serve` endpoint's p99 is a latency
distribution, not a throughput number; this module connects connection-RTT and
queueing to the tail you actually ship.

### Module 3 — Throughput and the hardware ceiling

`iperf3` client/server, single stream, `-R` reverse, `-P` parallel — and why on
this LAN one stream already saturates (4 streams ≈ same ~9.9 Gbit/s), so `-P` is
the move only when a single stream is CPU- or window-limited. Then read the
**hardware ceiling** directly: `cat /sys/class/net/<if>/speed` (10000 → 10 GbE),
MTU 9000 (jumbo frames), `ethtool`, same-switch topology, and the theoretical max
goodput. ML reframe: this is the ceiling your checkpoint transfers and your
training-fabric all-reduce are measured against.

### Module 4 — The CPU / crypto ceiling: why moving a checkpoint is slow

The keystone lesson, and the one most networking courses skip. `scp`/`rsync`
trail the link because SSH crypto is **single-core-bound** and the cipher choice
dominates. Isolate the crypto cost **memory-to-memory** with `dd`-over-`ssh` (so
disk and the SSH per-channel window don't masquerade as a network result); check
`/proc/cpuinfo` for `aes`; then compare ciphers on the real 10 GbE path:

```text
chacha20-poly1305  ~619 MB/s   (software, single core)
aes128-gcm         ~966 MB/s   (AES-NI)
aes256-gcm         ~1.1 GB/s   (AES-NI)  ← ~1.8x the default
```

The fix is two lines in `~/.ssh/config` (or `scp -c aes256-gcm@openssh.com`),
and it speeds up *every* `scp` and `rsync` of a model file automatically. ML
reframe: this single change nearly halves the time to stage a multi-GB
safetensors/GGUF checkpoint onto a serving host.

### Module 5 — Encrypted vs raw transport for ML artifacts

On a trusted LAN, encryption is pure overhead: raw `nc` + `tar` hits wire speed
(~1.2 GB/s, ~9.6 Gbit/s) vs encrypted SSH. When dropping encryption for a model
sync is justified (trusted, isolated fabric) and when it absolutely is not
(anything crossing a trust boundary). Covers the OpenBSD `nc -l 8081` listener
quirk and `rsync` daemon mode for repeated, checksummed checkpoint syncs. ML
reframe: choosing the transport for a training-cluster artifact-distribution step
is a throughput/security trade-off you make explicitly, with numbers.

### Module 6 — When tests lie or hang, and building netprobe

Diagnose a transfer that *hangs* rather than fails: a `ufw` **default-DROP**
silently discards packets so the client blocks until timeout, whereas a REJECT
returns instantly ("connection refused"). A hang that is not a refusal almost
always means a DROP firewall or a black-holed route or an MTU blackhole — the
exact failure mode that makes an `apr serve` health check or a data-loader stall
mysteriously. Then build **netprobe** in Rust: a TCP throughput sender + a
TCP-connect RTT prober, written **contract-first** so its invariants
(`t.bytes == bytes`, `min ≤ avg ≤ max`, `0 ≤ loss_pct ≤ 100`, the bits/bytes
conversion) are proven before the code — a working preview of Track 7.

## Capstone

**Diagnose and tune a real ML data path.** Pick one path that matters — staging a
`safetensors`/GGUF checkpoint between two hosts, or an `apr serve` inference
endpoint under load — and produce a one-page report that demonstrates the full
discipline end to end:

1. **Baseline** — measure what you have today. For a transfer: time an actual
   `scp`/`rsync` of the checkpoint and convert to MB/s and Gbit/s. For serving:
   record `apr serve` throughput and the RTT/tail-latency distribution under load
   (use `netprobe` for connect-RTT, `apr profile` to confirm the model is not the
   bound).
2. **Ceiling** — read the NIC link speed and MTU, compute the theoretical max
   goodput, and state the gap between the ceiling and your measured number.
3. **Prove the bottleneck** — attribute the gap to one constraint *with evidence*.
   To blame the cipher, first control for disk and the SSH window by measuring
   crypto **memory-to-memory** (`dd`-over-`ssh`, or transfer from `tmpfs`), so
   disk speed cannot masquerade as a network result. Alternatively prove the gap
   is MTU- or firewall-related. Assertion without evidence does not pass.
4. **Tune** — apply **exactly one** change: swap the SSH cipher to AES-NI
   `aes256-gcm`, enable jumbo frames, or switch transport for the artifact sync.
5. **Re-measure** — present before/after numbers (e.g. ~620 MB/s → ~1.1 GB/s for
   the checkpoint, or a lowered serving p99) and a one-paragraph explanation of
   *why* it worked.

> **Reproducibility note:** a real-file `scp` is gated by disk and the SSH
> per-channel window *in addition to* the cipher. The cipher win is cleanest
> memory-to-memory; on a slow disk a real-file transfer may not show the full
> ~1.8x, and explaining *why* is part of the grade.

**Build-track variant:** ship `netprobe` as the measurement tool of the report —
it measures TCP throughput and TCP-connect RTT between the two hosts and
reproduces an `iperf3` throughput figure within a stated tolerance (e.g. ±10%).
Because `netprobe` is contract-first, `make rust` (fmt + clippy + test) and
`pv validate` over the kernel contract gate it, so the tool you demo is *proven*,
not just tested.

The deliverable is a **LinkedIn-shareable portfolio artifact**: "I made an ML
checkpoint transfer 1.8x faster by proving the bottleneck was the SSH cipher, not
the 10 GbE link" — with the before/after numbers and the reasoning that closed
the gap. Evaluation tiers (PAIML capstone-rubric style):

- **Advanced** — correct theoretical-vs-measured analysis, a *proven* bottleneck,
  one tuning change with a quantified before/after gain, and (build track) a
  `netprobe` that matches `iperf3` within tolerance.
- **Proficient** — clean baseline + ceiling math + a plausible, evidence-backed
  bottleneck and one re-measured tuning.
- **Developing** — runs the tools and reports raw numbers but mislabels line rate
  as goodput, or names a bottleneck without evidence.

## Anchored In

- **`apr serve`** — the inference endpoint whose throughput and tail latency the
  diagnostic framework is pointed at.
- **`apr profile`** — confirms compute is *not* the bound, so a slow path is
  attributable to data movement.
- **netprobe** — the contract-governed Rust TCP throughput + TCP-connect RTT
  probe built and used in this course.
- **Moving aprender model files** — staging multi-GB `safetensors`/GGUF
  checkpoints between machines, the headline ML reframe of the 10 GbE case.

## References

- [iperf3 documentation](https://software.es.net/iperf/) — throughput
  measurement reference behavior.
- [ssh_config(5)](https://man.openbsd.org/ssh_config.5) — `Ciphers` directive
  and per-host configuration.
- [ethtool(8)](https://man7.org/linux/man-pages/man8/ethtool.8.html) — NIC link
  speed and negotiation.
- [RFC 9293 — Transmission Control Protocol](https://www.rfc-editor.org/rfc/rfc9293)
  — the TCP behavior underneath every measurement.
- [RFC 8439 — ChaCha20 and Poly1305 for IETF Protocols](https://www.rfc-editor.org/rfc/rfc8439)
  — the software cipher that bounds default SSH throughput.
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) — the
  conventions netprobe's public API follows.

## Sibling Repos

- [paiml/aprender](https://github.com/paiml/aprender) — Pure-Rust ML framework
  (the `apr serve` / `apr profile` endpoints and the model files this course
  moves).
- [netprobe](./netprobe/) — the contract-governed Rust TCP throughput +
  TCP-connect RTT probe that ships with this course.
- [contracts/netprobe-v1.yaml](./contracts/netprobe-v1.yaml) — the provable
  kernel contract (throughput, latency, and bits/bytes invariants) that governs
  netprobe, validated with `pv`.
- See the top-level [README.md](../../../README.md#foundation-repositories) for
  the full foundation-repository list.
