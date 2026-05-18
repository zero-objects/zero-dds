# Realtime Deployment Guide

**Stand:** 2026-05-02
**Sprint-Bezug:** Phase-5 D.3 (`zerodds-rt-linux`-Crate).

Dieses Dokument beschreibt, wie ein ZeroDDS-Prozess auf einem Linux-
Host so deployt wird, dass die Latenz-Ziele aus Phase-5 (`p99 < 5µs`,
`p999 < 20µs`, `p9999 < 100µs`) gehalten werden koennen. Der Fokus
liegt auf den drei Stellschrauben, die ein normaler User-Space-
Prozess hat: Scheduler-Policy, CPU-Pinning und Kernel-Konfiguration.

Was hier *nicht* dokumentiert ist: Anwendungs-Logik (Lock-Free
History, no_alloc Hot-Path) — die ist in `docs/PHASE5_PLAN.md` D.1+D.4
beschrieben.

---

## 1. Kapabilitaets-Voraussetzungen

`SchedulerProfile::apply_to_current_thread()` ruft je nach Variante
unterschiedliche Linux-Syscalls auf. Welche Capability dafuer
erforderlich ist:

| Profile                | Syscall          | Capability      | Bemerkung                                    |
|------------------------|------------------|-----------------|----------------------------------------------|
| `Default`              | sched_setattr    | keine           | SCHED_OTHER ist privilegienfrei              |
| `RealtimeFifo { 0 }`   | sched_setattr    | RLIMIT_RTPRIO   | Priority 0 = effektiv SCHED_OTHER auf Linux  |
| `RealtimeFifo { > 0 }` | sched_setattr    | `CAP_SYS_NICE`  | Hard-RT-Priority                             |
| `RealtimeRoundRobin`   | sched_setattr    | `CAP_SYS_NICE`  | wie FIFO, aber mit Quantum                   |
| `Deadline { … }`       | sched_setattr    | `CAP_SYS_NICE`  | dazu: Bandbreiten-Reservierung muss passen   |
| Pinning (Affinity)     | sched_setaffinity| keine           | nur erlaubte CPUs der eigenen cpu_set        |

Setzen via `setcap` (empfohlen statt `sudo`):

```bash
sudo setcap 'cap_sys_nice=eip' /opt/zerodds/bin/your-binary
getcap /opt/zerodds/bin/your-binary
# /opt/zerodds/bin/your-binary cap_sys_nice=eip
```

systemd-Units alternativ:

```ini
[Service]
ExecStart=/opt/zerodds/bin/your-binary
AmbientCapabilities=CAP_SYS_NICE
CapabilityBoundingSet=CAP_SYS_NICE
LimitRTPRIO=99
LimitMEMLOCK=infinity
```

---

## 2. Scheduler-Profil-Auswahl

Drei typische Deployments:

### 2.1 Soft-Realtime — Industrial-Telemetrie (Phase-5-Default)

```rust
use zerodds_rt_linux::SchedulerProfile;

SchedulerProfile::RealtimeFifo { priority: 50 }.apply_to_current_thread()?;
```

* Begruendung: Priority 50 ist Mid-Range — kollidiert nicht mit
  Kernel-Threads bei 99, gibt aber dem Hot-Path Vorrang vor allen
  Default-Threads.
* Erwartung: p99 < 50µs auf einem normal getunten Kernel ohne
  preempt_rt.

### 2.2 Hard-Realtime — Avionik / Robotik

```rust
SchedulerProfile::Deadline {
    runtime_ns: 200_000,    // 200µs WCET pro Periode
    deadline_ns: 1_000_000, // 1ms Deadline
    period_ns: 1_000_000,   // 1ms Periode (1 kHz Loop)
}.apply_to_current_thread()?;
```

* Begruendung: SCHED_DEADLINE garantiert (sofern Reservierung
  gewaehrt wird) eine Bandbreite. Forks und Cloning vererben das
  Profil **nicht** — neue Threads laufen wieder als CFS.
* Voraussetzung: `runtime <= deadline <= period`. Wenn die
  Bandbreitenreservierung das Default-Limit (95 %) sprengt, lehnt
  der Kernel mit `EBUSY` ab.

Reservierungslimit hochsetzen (Cluster-Operator):

```bash
sysctl -w kernel.sched_rt_runtime_us=950000  # default
# oder fuer SCHED_DEADLINE explizit:
echo 950000 > /proc/sys/kernel/sched_rt_runtime_us
```

### 2.3 Best-Effort — Standard-Server-Workload

```rust
// Default — nichts zu tun. SchedulerProfile::Default ist no-op
// (laeuft auf SCHED_OTHER wie jeder andere Thread).
```

* Empfohlen wenn der Host mit anderen Workloads geteilt wird und
  RT-Garantien nicht eingelagert werden duerfen.

---

## 3. CPU-Pinning + Isolation

ZeroDDS' Hot-Path-Threads (Discovery, Reader-Loop, Writer-Loop) sind
pro Domain-Participant separat anpinnbar:

```rust
use zerodds_rt_linux::pin_current_thread_to_cpus;
pin_current_thread_to_cpus(&[3])?;  // pin to CPU 3
```

Ohne Kernel-Isolation bringt das wenig — andere Prozesse landen
trotzdem auf CPU 3. Echte Isolation braucht Boot-Parameter.

### 3.1 isolcpus / nohz_full / rcu_nocbs

GRUB-Editierung (`/etc/default/grub`):

```
GRUB_CMDLINE_LINUX="isolcpus=2-7 nohz_full=2-7 rcu_nocbs=2-7 \
                    irqaffinity=0-1 mitigations=off"
```

* `isolcpus=2-7`: CPUs 2-7 werden dem CFS-Default-Scheduler entzogen;
  nur explizit gepinnte Threads laufen darauf.
* `nohz_full=2-7`: kein Tick-Interrupt auf 2-7 wenn nur ein
  Userspace-Task laeuft (deutlich reduzierter Jitter).
* `rcu_nocbs=2-7`: RCU-Callbacks auf andere CPUs; verhindert Tick-
  Pegel von RCU-Threads.
* `irqaffinity=0-1`: alle Hardware-Interrupts gehen auf 0+1.
* `mitigations=off`: hebt Spectre-/Meltdown-Workarounds auf — **nur**
  in vertrauenswuerdigen Umgebungen.

Update + Reboot:

```bash
sudo update-grub  # Debian/Ubuntu
# oder: sudo grub2-mkconfig -o /boot/grub2/grub.cfg  # RHEL/Fedora
sudo reboot
```

Verifikation nach Reboot:

```bash
cat /sys/devices/system/cpu/isolated   # erwartet: 2-7
cat /sys/devices/system/cpu/nohz_full  # erwartet: 2-7
cat /proc/cmdline                       # GRUB-Args sehen
```

### 3.2 Pinning-Layout-Empfehlung

Beispiel-Layout fuer einen 8-Core-Host:

| CPUs   | Rolle                                  |
|--------|----------------------------------------|
| 0      | OS + alle System-Threads (kthreads)    |
| 1      | Hardware-Interrupts (irqaffinity)      |
| 2      | DDS-Discovery + Heartbeat (Background) |
| 3      | DDS-Writer Hot-Path                    |
| 4      | DDS-Reader Hot-Path                    |
| 5-7    | Anwendungs-Threads                     |

```rust
use std::thread;
use zerodds_rt_linux::{pin_current_thread_to_cpus, SchedulerProfile};

let writer_thread = thread::spawn(|| {
    pin_current_thread_to_cpus(&[3])?;
    SchedulerProfile::RealtimeFifo { priority: 60 }
        .apply_to_current_thread()?;
    // … Hot-Path-Loop
    Ok::<_, std::io::Error>(())
});
```

---

## 4. PREEMPT_RT (optional)

Fuer p99 < 5 µs und harten Determinismus braucht man preempt_rt.
Standard-Distros liefern es nicht; selbst kompilieren oder
distro-spezifische Kernels nutzen.

* Debian: `linux-image-rt-amd64` aus `bookworm-backports`.
* Ubuntu Pro: `--realtime` Subscription.
* Fedora: `kernel-rt`.
* Yocto: `meta-realtime`-Layer.

Verifikation:

```bash
uname -v | grep -i preempt_rt   # PREEMPT_RT in der Version-String
cat /sys/kernel/realtime          # Wert "1"
```

Mit preempt_rt:

* Spinlocks werden zu Sleeping-Locks → SCHED_FIFO-Threads koennen
  jeden anderen Thread blockieren (auch Kernel). Vorsicht bei
  langen Locks im Hot-Path — Cluster-D D.4 (Lock-Free History)
  adressiert das im DDS-Pfad.
* Interrupt-Handler sind threaded → priorisierbar.

---

## 5. Latenz-Bench (Phase-5 D.5)

Der Throughput-Bench aus D.2 ist nicht das gleiche wie der p99-
Latenz-Bench aus D.5. Letzterer (siehe `docs/PHASE5_PLAN.md` D.5)
braucht busy-poll-Reader gegen busy-poll-Writer auf demselben Host
mit isolcpus, gepinnten Threads und SCHED_FIFO. Befehl wenn das in
Sprint 22+ landet:

```bash
sudo zerodds-perf roundtrip-1us --writer-cpu 3 --reader-cpu 4 \
    --priority 60 --duration 60s --histogram
```

---

## 6. Troubleshooting

| Symptom                                  | Ursache                                  | Fix                                                     |
|------------------------------------------|------------------------------------------|---------------------------------------------------------|
| `apply_to_current_thread()` → `EPERM`    | fehlende CAP_SYS_NICE                    | `setcap` siehe §1                                        |
| `SCHED_DEADLINE` → `EBUSY`               | Bandbreitenreservierung voll             | sched_rt_runtime_us hochsetzen oder runtime_ns kuerzen   |
| `pin_current_thread_to_cpus()` → `EINVAL`| CPU offline / nicht in /sys/.../online   | `cat /sys/devices/system/cpu/online`                    |
| Latenz-Spikes alle ~10ms                 | Tick-Interrupt aktiv                     | `nohz_full` setzen + `cat /proc/interrupts` checken      |
| Latenz-Spikes alle ~1s                   | RCU-Callbacks                            | `rcu_nocbs` setzen                                       |
| Sporadische 100µs-Spikes                 | Hardware-Interrupts auf RT-CPU           | `irqaffinity` korrekt? `cat /proc/irq/*/smp_affinity`   |
| Performance-Regression nach Kernel-Update | preempt_rt-Patches geaendert             | mit `perf record -e sched:sched_switch` auswerten        |

---

## 7. Validierung

Soak-Test einmalig vor Produktions-Roll-Out:

```bash
# 24-Stunden-Soak mit Last
zerodds-perf aes-gcm --bytes 100GB --block 1024 &
sudo cyclictest -p 99 -t 4 -i 1000 -l 86400000 -h 200 -m
```

`cyclictest`-Erwartung mit korrekter Konfig:

* `Min`: 1-3 µs
* `Avg`: 2-5 µs
* `Max`: < 100 µs (ohne preempt_rt) bzw. < 20 µs (mit preempt_rt)

Falls `Max` deutlich daneben liegt: Kernel-Konfig durchgehen
(siehe §3.1 + §6).

---

## 8. Referenzen

* `sched(7)` — Uebersicht der Linux-Scheduler-Policies.
* `sched_setattr(2)` — Syscall-Doku.
* `sched_setaffinity(2)` — CPU-Pinning-Syscall.
* `cpuset(7)` — Cgroup-basierte Isolation (alternative zu isolcpus).
* `RT_PREEMPT HOWTO` — https://wiki.linuxfoundation.org/realtime/start
* `osadl.org` — Open Source Automation Development Lab, RT-Latency-Plots.
