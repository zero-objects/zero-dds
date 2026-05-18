# RT-Tuning Host-Config-Checkliste — D.5b Phase-C

Voraussetzung fuer das Erreichen der `roundtrip-1us` CI-Gates:
* p99 < 5 µs
* p999 < 20 µs
* p9999 < 100 µs

Fuer 64-Byte-Payloads auf einer dedizierten 1-GbE-Direktverbindung
zwischen `llvm` und einem Peer.

## 1. Kernel: PREEMPT_RT

Linux 6.x ab Mainline 6.12 hat `PREEMPT_RT` integriert. Vorher:
[Linux-RT-Wiki](https://wiki.linuxfoundation.org/realtime/start)
Patchset.

**Verifikation:**

```bash
uname -v | grep -i 'PREEMPT_RT'
cat /sys/kernel/realtime    # erwartet: 1
```

Auf llvm aktuell **nicht** RT. Voraussetzung fuer Phase-C:

```bash
sudo apt install linux-image-rt-amd64    # Debian-RT-Kernel-Variant
sudo reboot
```

## 2. Boot-Parameter

`/etc/default/grub` ergaenzen um `GRUB_CMDLINE_LINUX_DEFAULT`:

```
isolcpus=2-7 nohz_full=2-7 rcu_nocbs=2-7 \
processor.max_cstate=1 intel_idle.max_cstate=0 \
mce=off audit=0 noht
```

Effekt:
* `isolcpus=2-7` — CPUs 2 bis 7 aus dem Scheduler-Pool. Kein
  Userspace-Process landet dort, ausser explizit per `taskset`.
* `nohz_full=2-7` — Ticks auf isolierten Cores ausschalten →
  kein Timer-Interrupt-Jitter.
* `rcu_nocbs=2-7` — RCU-Callbacks auf andere Cores delegieren.
* `processor.max_cstate=1` + `intel_idle.max_cstate=0` — keine
  Deep-Sleeps, sonst sind die ersten paar Wake-Up-µs verloren.
* `mce=off` — Machine-Check-Exceptions abschalten (sporadische
  100µs+ Interrupts).
* `noht` — Hyper-Threading aus (Cache-Sharing zwischen Geschwister-
  Threads ist Latenz-Killer).

```bash
sudo update-grub
sudo reboot
```

## 3. cyclictest-Baseline

**Vor** dem ZeroDDS-Bench: erst die Hardware-Latenz messen.

```bash
sudo apt install rt-tests
sudo taskset -c 2 chrt -f 80 cyclictest \
    -p 80 -t 1 -n -m -i 200 -l 1000000 -q
```

Erwartung auf einer korrekt getunten Maschine:
* min < 1 µs
* avg < 3 µs
* max < 30 µs (nach 1M Iterationen)

Bei p99 > 50 µs in cyclictest erübrigt sich der ZeroDDS-Bench —
die Hardware/Kernel-Setup hat das Floor schon nicht erreicht.

## 4. CPU-Governor + IRQ-Affinity

```bash
# Performance-Governor (kein DVFS-Frequency-Hopping):
for c in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do
    echo performance | sudo tee "$c"
done

# IRQs vom isolierten Cores wegrouten (NUR Core 0+1):
for irq in /proc/irq/*/smp_affinity; do
    echo 3 | sudo tee "$irq"   # 0b11 = Core 0+1
done
```

Pruefung:
```bash
cat /proc/interrupts | head
```

Spalten 2-7 (Core-2..Core-7) sollten alle ~0 zeigen.

## 5. NIC-Tuning

```bash
# RX-Coalescing aus (sonst Interrupt-Bursts → ms-spikes):
sudo ethtool -C eth0 rx-usecs 0 rx-frames 1

# Increase tx-queue:
sudo ip link set eth0 txqueuelen 10000

# busy_poll: NAPI-Polling statt Interrupts (kostet CPU, druckt
# Latenz-Tail).
echo 50 | sudo tee /proc/sys/net/core/busy_poll
echo 50 | sudo tee /proc/sys/net/core/busy_read
```

## 6. Roundtrip-1us Aufruf-Kommando

Pong (Echo-Endpoint) auf isoliertem Core 2:

```bash
sudo taskset -c 2 chrt -f 80 \
    /usr/local/bin/roundtrip-1us \
    --role pong --bind 0.0.0.0:7400 --max-runtime 600
```

Ping (Mess-Endpoint) auf isoliertem Core 3:

```bash
sudo taskset -c 3 chrt -f 80 \
    /usr/local/bin/roundtrip-1us \
    --role ping \
    --remote 192.0.2.10:7400 --bind 0.0.0.0:7401 \
    --warmup 10000 --samples 1000000 \
    --hgrm /tmp/zerodds-rt-bench.hgrm \
    --ci-gate
```

`--ci-gate` exit 1 wenn p99 ≥ 5µs / p999 ≥ 20µs / p9999 ≥ 100µs.

## 7. CI-Pipeline

`ci/jobs/rt-bench.yml` (D.5b-Phase-C-Plan):

```yaml
rt-bench:
  stage: bench
  rules:
    - if: '$CI_PIPELINE_SOURCE == "schedule" && $SCHEDULE_NAME == "rt-nightly"'
  tags: [rt-host]   # nur Runner mit PREEMPT_RT-Kernel
  before_script:
    - bash tests/perf/rt-tuning/preflight.sh
  script:
    - bash tests/perf/rt-tuning/run-bench.sh
  artifacts:
    paths:
      - /tmp/zerodds-rt-bench.hgrm
```

Der `preflight.sh`-Check verifiziert PREEMPT_RT + isolcpus +
performance-Governor und exit'd 1 bei nicht-getuntem Host.
