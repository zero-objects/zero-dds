# RT tuning host config checklist — D.5b phase C

Prerequisite for reaching the `roundtrip-1us` CI gates:
* p99 < 5 µs
* p999 < 20 µs
* p9999 < 100 µs

For 64-byte payloads on a dedicated 1 GbE direct connection
between `llvm` and a peer.

## 1. Kernel: PREEMPT_RT

Linux 6.x has `PREEMPT_RT` integrated from mainline 6.12. Before that:
[Linux RT wiki](https://wiki.linuxfoundation.org/realtime/start)
patch set.

**Verification:**

```bash
uname -v | grep -i 'PREEMPT_RT'
cat /sys/kernel/realtime    # expected: 1
```

On llvm currently **not** RT. Prerequisite for phase C:

```bash
sudo apt install linux-image-rt-amd64    # Debian RT kernel variant
sudo reboot
```

## 2. Boot parameters

Extend `GRUB_CMDLINE_LINUX_DEFAULT` in `/etc/default/grub` with:

```
isolcpus=2-7 nohz_full=2-7 rcu_nocbs=2-7 \
processor.max_cstate=1 intel_idle.max_cstate=0 \
mce=off audit=0 noht
```

Effect:
* `isolcpus=2-7` — CPUs 2 to 7 out of the scheduler pool. No
  userspace process lands there, except explicitly via `taskset`.
* `nohz_full=2-7` — switch off ticks on isolated cores →
  no timer interrupt jitter.
* `rcu_nocbs=2-7` — delegate RCU callbacks to other cores.
* `processor.max_cstate=1` + `intel_idle.max_cstate=0` — no
  deep sleeps, otherwise the first few wake-up µs are lost.
* `mce=off` — switch off machine check exceptions (sporadic
  100µs+ interrupts).
* `noht` — hyper-threading off (cache sharing between sibling
  threads is a latency killer).

```bash
sudo update-grub
sudo reboot
```

## 3. cyclictest baseline

**Before** the ZeroDDS bench: measure the hardware latency first.

```bash
sudo apt install rt-tests
sudo taskset -c 2 chrt -f 80 cyclictest \
    -p 80 -t 1 -n -m -i 200 -l 1000000 -q
```

Expectation on a correctly tuned machine:
* min < 1 µs
* avg < 3 µs
* max < 30 µs (after 1M iterations)

With p99 > 50 µs in cyclictest the ZeroDDS bench is pointless —
the hardware/kernel setup has already failed to reach the floor.

## 4. CPU governor + IRQ affinity

```bash
# performance governor (no DVFS frequency hopping):
for c in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do
    echo performance | sudo tee "$c"
done

# route IRQs away from the isolated cores (ONLY core 0+1):
for irq in /proc/irq/*/smp_affinity; do
    echo 3 | sudo tee "$irq"   # 0b11 = Core 0+1
done
```

Check:
```bash
cat /proc/interrupts | head
```

Columns 2-7 (core 2..core 7) should all show ~0.

## 5. NIC tuning

```bash
# RX coalescing off (otherwise interrupt bursts → ms spikes):
sudo ethtool -C eth0 rx-usecs 0 rx-frames 1

# Increase tx-queue:
sudo ip link set eth0 txqueuelen 10000

# busy_poll: NAPI polling instead of interrupts (costs CPU, pushes down
# the latency tail).
echo 50 | sudo tee /proc/sys/net/core/busy_poll
echo 50 | sudo tee /proc/sys/net/core/busy_read
```

## 6. roundtrip-1us invocation command

Pong (echo endpoint) on isolated core 2:

```bash
sudo taskset -c 2 chrt -f 80 \
    /usr/local/bin/roundtrip-1us \
    --role pong --bind 0.0.0.0:7400 --max-runtime 600
```

Ping (measuring endpoint) on isolated core 3:

```bash
sudo taskset -c 3 chrt -f 80 \
    /usr/local/bin/roundtrip-1us \
    --role ping \
    --remote 192.0.2.10:7400 --bind 0.0.0.0:7401 \
    --warmup 10000 --samples 1000000 \
    --hgrm /tmp/zerodds-rt-bench.hgrm \
    --ci-gate
```

`--ci-gate` exits 1 if p99 ≥ 5µs / p999 ≥ 20µs / p9999 ≥ 100µs.

## 7. CI pipeline

`ci/jobs/rt-bench.yml` (D.5b phase C plan):

```yaml
rt-bench:
  stage: bench
  rules:
    - if: '$CI_PIPELINE_SOURCE == "schedule" && $SCHEDULE_NAME == "rt-nightly"'
  tags: [rt-host]   # only runners with a PREEMPT_RT kernel
  before_script:
    - bash tests/perf/rt-tuning/preflight.sh
  script:
    - bash tests/perf/rt-tuning/run-bench.sh
  artifacts:
    paths:
      - /tmp/zerodds-rt-bench.hgrm
```

The `preflight.sh` check verifies PREEMPT_RT + isolcpus +
performance governor and exits 1 on an untuned host.
