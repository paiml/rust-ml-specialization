# Lab Commands — Network Performance for ML Systems

> **Track 6 · Course 57** · Anchored in: `apr serve`, `apr profile`, moving aprender model files.
> Companion lab/command reference for the course idea
> [`ideas/network-performance-testing.md`](../ideas/network-performance-testing.md). This is the
> ML-systems reframe of the Network Performance Testing diagnostic course: same rigorous method,
> pointed at ML data movement.

Every command below was **actually run** during the originating investigation (two hosts on a
10 GbE LAN: a client and `intel` at `192.168.50.100`). Numbers in comments are the real observed
results. The **gotchas are the teaching moments** — they are called out, not hidden. Substitute your
own two hosts' addresses.

Canonical results this lab reproduces: `iperf3` ~9.9 Gbit/s · `scp`/`rsync` ~620 MB/s (chacha20) →
~1.1 GB/s (aes256-gcm) · raw `nc` ~1.2 GB/s.

---

## For ML Systems

This course teaches **network performance as a diagnostic discipline** — line rate vs goodput, and
the NIC → MTU → CPU/cipher → firewall bottleneck-triage framework — and then points it at the data
movement that dominates real ML systems. The same gap between a link's theoretical capacity and the
goodput an application actually gets shows up everywhere in an ML stack, just with bigger files and
tighter latency budgets:

- **Moving large model files fast.** A multi-GB `safetensors` or `GGUF` checkpoint copied between a
  training box and a serving box at 620 MB/s instead of 1.2 GB/s nearly doubles your transfer wall
  time. At checkpoint-every-N-steps cadence, that gap is paid over and over. Module 4's cipher fix is
  the single most common reason `scp model.safetensors` is slower than the link allows.
- **Inference-serving throughput and tail latency.** An `apr serve` endpoint is judged on p50/p99
  latency and tokens/sec under load. Module 1's `ping` baseline tells you the *network floor* under
  that latency; if RTT is sub-millisecond on the LAN, a slow endpoint is the model or the server, not
  the wire — and you have proven it.
- **Distributed-training bandwidth.** Gradient/parameter sync (all-reduce, parameter-server pulls)
  is network-bound at scale. Module 2's `iperf3 -P` parallel-stream test is exactly the shape of
  multi-connection collective traffic, and tells you the aggregate bandwidth a sync step can hope for.
- **Data-loading pipelines.** A loader streaming shards from a remote store to feed GPUs must keep
  the accelerators fed; if the loader caps below the link, the GPUs starve. The same line-rate-vs-
  goodput math decides whether the bottleneck is the network, the disk, or the decode CPU.

The build track (Module 7) ships `netprobe`, a contract-governed Rust probe (TCP throughput +
TCP-connect RTT) — built contract-first with provable contracts, which previews Track 7
(Correctness).

> **Reproducibility note.** Throughput figures below are memory-to-memory (`dd`/`iperf3`) to isolate
> the network/CPU bound from disk. A real multi-GB `safetensors` transfer is *also* gated by disk
> read/write and the SSH per-channel window, so a real-file copy may not show the full cipher win on
> a slow disk — that is expected, and Module 4 explains why.

## Module 0 — Units (bits vs bytes)

```bash
# Networking is bits/sec; transfers report bytes/sec. Always convert before judging.
#   1 byte = 8 bits  →  multiply MB/s by 8 to get Mbit/s
#   620 MB/s  × 8  = 4960 Mbit/s ≈ 5.0 Gbit/s   (only HALF of a 10 GbE link → why?)
#   1.2 GB/s  × 8  ≈ 9.6 Gbit/s                  (≈ wire speed)
python3 -c 'print(620*8/1000, "Gbit/s")'   # 4.96
```

```bash
# ML framing: how long to move a checkpoint at each rate? Wall time = size / goodput.
#   A 14 GB safetensors checkpoint at 620 MB/s vs 1.2 GB/s:
python3 -c 'print(14000/620, "s @ 620 MB/s"); print(14000/1200, "s @ 1.2 GB/s")'
#   22.6 s @ 620 MB/s   vs   11.7 s @ 1.2 GB/s  → the cipher fix ~halves checkpoint copy time.
```

## Module 1 — Latency & loss (`ping`)

```bash
ping -c 10 192.168.50.100
# --- 192.168.50.100 ping statistics ---
# 10 packets transmitted, 10 received, 0% packet loss
# rtt min/avg/max/mdev = 0.191/0.297/0.773/0.180 ms
# Read it: sub-millisecond avg + 0% loss = a clean same-switch LAN path.
# If avg were tens of ms or loss > 0, latency/loss — not bandwidth — is your problem.
```

```bash
# ML framing: this RTT is the NETWORK FLOOR under an inference request. Compare it to the
# end-to-end latency of an apr serve endpoint on the same host to attribute the latency:
#   1) network floor (this ping)   ~0.3 ms
#   2) end-to-end request latency  (curl below)
# If (2) >> (1), the cost is the model/server, not the wire — and now you've proven it.
curl -s -o /dev/null -w 'connect=%{time_connect}s  ttfb=%{time_starttransfer}s  total=%{time_total}s\n' \
  http://192.168.50.100:8080/v1/completions \
  -H 'Content-Type: application/json' \
  -d '{"prompt":"hello","max_tokens":16}'
# connect ≈ the TCP-handshake RTT (close to ping); ttfb − connect ≈ model time-to-first-token.
```

## Module 2 — Throughput with `iperf3`

```bash
# --- SERVER (on the receiving host) ---
# GOTCHA 1: `iperf3 -s -1` (one-shot) and `-D` (daemon) can die when the launching
# SSH session closes. Robust detach that survives the session:
setsid sh -c "iperf3 -s -p 8081 >/tmp/iperf3.log 2>&1" </dev/null >/dev/null 2>&1 &
# (or just run `iperf3 -s -p 8081` in a foreground terminal you keep open)

# GOTCHA 2: pick a port the host's firewall ALLOWS (see Module 6). On a default-DROP
# host, iperf3 on a blocked port HANGS forever instead of erroring. We used 8081.

# --- CLIENT ---
iperf3 -c 192.168.50.100 -p 8081 -t 10           # single stream  → ~9.85 Gbit/s
iperf3 -c 192.168.50.100 -p 8081 -t 10 -R        # reverse        → ~9.90 Gbit/s
iperf3 -c 192.168.50.100 -p 8081 -t 10 -P 4      # 4 parallel SUM → ~9.83 Gbit/s
# A single stream already saturates 10 GbE here, so -P barely changes the number.
# -P is the move when ONE stream is CPU- or window-limited (not the case on this LAN).
```

```bash
# ML framing: -P models DISTRIBUTED-TRAINING sync traffic. Gradient/parameter all-reduce
# opens many concurrent connections; the parallel SUM is the aggregate bandwidth a sync
# step can hope for — the ceiling your collective-comms library divides among ranks.
iperf3 -c 192.168.50.100 -p 8081 -t 10 -P 8      # 8 parallel SUM ≈ link ceiling (~9.8 Gbit/s)
# If the SUM climbs well above a single stream, ONE stream was the limit — exactly the
# regime where adding ranks/connections helps. If it doesn't climb, you're already at the
# wire and more ranks just contend for the same bytes/sec.
```

### Quick throughput without iperf3 — `dd` over `ssh`

```bash
# Handy when iperf3 isn't installed. Rides port 22 (already open), encrypted.
# download (server -> client):
ssh intel 'dd if=/dev/zero bs=1M count=1024 2>/dev/null' | dd of=/dev/null bs=1M
# upload (client -> server):
dd if=/dev/zero bs=1M count=1024 2>/dev/null | ssh intel 'dd of=/dev/null bs=1M'
# ~550-600 MB/s here — NOTE this is SSH-cipher-bound, not the network. See Module 4.
```

## Module 3 — Reading the hardware ceiling

```bash
# NIC link speed (Mb/s) and MTU, straight from sysfs — no root needed:
cat /sys/class/net/eno2/speed      # 10000   → 10 GbE
cat /sys/class/net/eno2/mtu        # 9000    → jumbo frames enabled
ip -br addr show                   # which iface holds the LAN IP

# ethtool gives the same plus negotiation detail (may need sudo):
ethtool eno2 | grep -i speed

# Theoretical max: 10 Gbit/s line rate. With jumbo frames (MTU 9000), framing
# overhead is tiny → practical TCP goodput ceiling ~9.9 Gbit/s. With standard
# MTU 1500 you'd cap nearer ~9.4 Gbit/s. Both ends must agree on MTU.
```

```bash
# ML framing: this ceiling is the hard upper bound on how fast a checkpoint can move or
# how fast a data loader can stream shards. ~9.9 Gbit/s ÷ 8 ≈ 1.24 GB/s — no transfer
# (safetensors, GGUF, or training shards) can beat that on this link, regardless of tool.
python3 -c 'print(9.9/8, "GB/s ceiling")'   # 1.2375
```

## Module 4 — The CPU / crypto ceiling (why `scp` is slow)

```bash
# 1. Does the CPU have AES-NI? If yes, AES-GCM is the FAST path; chacha20 is not.
grep -m1 -o aes /proc/cpuinfo      # prints "aes" when AES-NI is present (both ends)

# 2. What cipher is SSH actually negotiating? (default chacha20 = software, 1 core)
ssh -G intel | grep -i '^ciphers'              # configured preference list
ssh -v intel true 2>&1 | grep -i 'cipher:'     # the negotiated cipher

# 3. Measure each cipher memory-to-memory (dd from /dev/zero), so DISK and the SSH
#    window don't confound the result — this isolates the CRYPTO cost:
for c in chacha20-poly1305@openssh.com aes128-gcm@openssh.com aes256-gcm@openssh.com; do
  printf "%-32s " "$c:"
  dd if=/dev/zero bs=1M count=4096 2>/dev/null | ssh -c "$c" intel 'dd of=/dev/null bs=1M 2>&1' | tail -1
done
# chacha20-poly1305 : ~619 MB/s   (software, single core)
# aes128-gcm        : ~966 MB/s   (AES-NI)
# aes256-gcm        : ~1.1 GB/s   (AES-NI)  ← ~1.8x the default
```

```bash
# 4. THE FIX — pin the fast cipher for this host in ~/.ssh/config:
#    (speeds up ssh, scp, AND rsync automatically)
cat >> ~/.ssh/config <<'EOF'

Host intel
    HostName 192.168.50.100
    User noah
    Ciphers aes256-gcm@openssh.com,aes128-gcm@openssh.com
EOF
ssh -G intel | grep -i '^ciphers'   # verify it now lists aes256-gcm first

# 5. Or choose the cipher ad-hoc, no config change:
scp -c aes256-gcm@openssh.com bigfile.bin intel:/tmp/
rsync -e "ssh -c aes256-gcm@openssh.com" -a ./dir/ intel:/tmp/dir/
```

```bash
# ML framing: time a REAL multi-GB safetensors checkpoint under each cipher. This is the
# everyday "why is scp model.safetensors slow?" moment. Use a checkpoint you actually have
# (an aprender model file, an exported safetensors, or a GGUF); here CKPT is multi-GB.
CKPT=model.safetensors
ls -lh "$CKPT"                                  # confirm it's multi-GB

# default (chacha20) baseline:
time scp -c chacha20-poly1305@openssh.com "$CKPT" intel:/tmp/   # ~620 MB/s class
# AES-NI fix:
time scp -c aes256-gcm@openssh.com        "$CKPT" intel:/tmp/   # ~1.1 GB/s class

# Expect ~1.8x on a fast disk; on a slow disk the real-file copy is ALSO gated by disk read
# + the SSH per-channel window, so you may see less than the memory-to-memory 1.8x. That is
# the line-rate-vs-goodput lesson applied to a checkpoint: prove which layer binds before
# you "fix" the wrong one.
```

## Module 5 — Encrypted vs raw transport

```bash
# On a trusted LAN, encryption is pure overhead. Raw `nc` hits wire speed.
# GOTCHA: OpenBSD netcat listens with `nc -l 8081` — NOT `nc -l -p 8081`.
# GOTCHA: a detached listener can die with its launching shell; hold it open by
#         keeping the SSH session that runs it alive in the background.

# --- raw throughput test ---
ssh intel 'nc -l 8081 >/dev/null' &                          # receiver (held open)
sleep 2
dd if=/dev/zero bs=1M count=4096 2>/tmp/rate | nc -q1 192.168.50.100 8081
tail -1 /tmp/rate    # ~1.2 GB/s (~9.6 Gbit/s) — near wire speed, no crypto

# --- bulk file/dir copy at wire speed (nc + tar) ---
# receiver:
ssh intel 'cd /dest && nc -l 8081 | tar xf -'
# sender:
tar cf - mydir | nc -q1 192.168.50.100 8081
# Trade-off: zero confidentiality/integrity — trusted LAN only. For repeated syncs
# with checksums/resume, prefer an rsync daemon (unencrypted rsync:// protocol).
```

```bash
# ML framing: moving a whole model DIRECTORY (config + tokenizer + sharded safetensors)
# at wire speed inside a trusted training cluster. nc + tar streams the tree with NO crypto
# tax — the fastest way to push a checkpoint dir between two boxes you both control.
# receiver (on the destination host):
ssh intel 'cd /models && nc -l 8081 | tar xf -'
# sender (the dir holding model-00001-of-00003.safetensors, config.json, tokenizer.json, ...):
tar cf - ./my-llm-checkpoint | nc -q1 192.168.50.100 8081
# Use ONLY on a trusted LAN: this gives up confidentiality and integrity. Crossing an
# untrusted network? Keep the AES-NI scp/rsync from Module 4 instead.
```

## Module 6 — When tests lie or hang (firewall diagnosis)

```bash
# Symptom we hit: iperf3 / nc to an arbitrary port just HUNG (no error, no output).
# Cause: the host runs ufw with a DEFAULT-DROP policy.

# On the target host:
sudo ufw status                    # showed: only 22/tcp + 8081-8083/tcp ALLOW
sudo iptables -L INPUT -n | head   # Chain INPUT (policy DROP)
ss -tlnp | grep 8081               # confirm your server is actually LISTENING

# KEY LESSON — hang vs refuse:
#   • DROP   → packet silently discarded → client BLOCKS until timeout  (looks like "slow"/"hung")
#   • REJECT → ICMP/RST sent back        → client fails INSTANTLY        ("connection refused")
# A hang that isn't a refusal almost always means a DROP firewall or a black-holed route.

# Fixes: use an already-allowed port (we used 8081), or open one temporarily:
sudo ufw allow 5201/tcp            # ... run the test ...
sudo ufw delete allow 5201/tcp     # ... then revert
```

```bash
# ML framing: the EXACT failure mode when "apr serve won't respond" from another box.
# A default-DROP firewall makes the inference port hang (client blocks till timeout), which
# is easy to misread as "the model is slow to load." Distinguish drop from refuse first:
ss -tlnp | grep 8080               # is apr serve actually LISTENING on the host?
sudo ufw status                    # is the serve port (e.g. 8080) ALLOWed?
nc -zv -w 3 192.168.50.100 8080    # succeeds / "refused" (REJECT) / times out (DROP)
# Times out → open the port; refused → the server isn't up; fast success → look at the model.
sudo ufw allow 8080/tcp            # ... expose the endpoint ...   then revert when done:
sudo ufw delete allow 8080/tcp
```

## Module 7 — (Build track) Rust `netprobe`

> **Contract-first, and now implemented.** The contract was written before the code (PAIML
> design-by-provable-contracts), and a reference crate now satisfies it:
> [`netprobe/`](../netprobe/) implements the three kernel functions and turns the contract's
> falsification tests (FALSIFY-NP-001/002/003) into `cargo test` cases. This previews
> Track 7 (Correctness): the same contract → falsification-test discipline applied to ML kernels.

```bash
# 1. The contract that governs netprobe's kernel (throughput, latency, bits/bytes):
pv validate contracts/netprobe-v1.yaml   # -> "Contract is valid."
pv status   contracts/netprobe-v1.yaml   # 3 equations, 3 obligations, 3 falsification tests

# 2. The implementation, gated by fmt + clippy + the contract's tests:
cargo test  --manifest-path netprobe/Cargo.toml      # 6 passed (incl. the 3 falsify_np_* tests)
cargo clippy --manifest-path netprobe/Cargo.toml --all-targets -- -D warnings

# 3. The repo's compliance gate runs all of it at once:
make rust              # fmt + clippy + test
make lint-contracts    # pv validate over contracts/*-v*.yaml
make check             # markdown + bashrs + pv + pmat + rust + structure

# To start your own probe instead, scaffold trait + test stubs from the contract:
pv scaffold contracts/netprobe-v1.yaml
```

`netprobe` measures the same two quantities every ML data-movement question reduces to: TCP
throughput (checkpoint copy speed, loader bandwidth, gradient-sync bandwidth) and TCP-connect RTT
(the network floor under an `apr serve` request). `measure_latency` uses **TCP-connect RTT** (no raw
sockets / root needed). Modules 0–6 above are shell labs and are *not* contract-governed — `pv`
validates Rust functions, not shell one-liners.
