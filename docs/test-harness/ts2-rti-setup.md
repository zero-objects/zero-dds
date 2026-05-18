# TS-2 — RTI Connext DDS Multi-Vendor-Interop

Stand 2026-05-02. **Status: extern blockiert** (Eval-License + Setup).

## Ziel

Dritter Vendor neben Cyclone DDS und eProsima Fast-DDS in der Live-Interop-
Matrix. Komplettiert die "drei großen" DDS-Implementierungen für Cross-
Vendor-Wire-Compliance-Tests.

## Status-Quo (CI-1 + CI-3b)

| Vendor | Pub-Test | Sub-Test | Speed-Bench |
|---|---|---|---|
| Cyclone DDS | ✓ via `ddsperf` | ✓ via `ddsperf` | ✓ Self-Bench (CI-3) |
| eProsima Fast-DDS | ✓ via `fastdds_pub` (CI-1 Suite) | (nicht gebaut) | (deferred) |
| RTI Connext | **deferred** | **deferred** | **deferred** |

## Eval-License-Beschaffung

RTI Connext DDS Professional ist kommerziell. Für die Test-Harness reicht
die kostenlose **30-Tage-Eval** oder die noch limitiertere **Connext
Express Edition**:

1. Account auf https://www.rti.com/free-trial registrieren (Firma:
   Ifyna; Use-case: "DDS-Implementation Cross-Vendor-Testing").
2. Eval-Lizenz-Datei (`rti_license.dat`) per E-Mail.
3. Connext-Bundle pro Plattform (Linux x86_64 + aarch64): ~600 MB
   tar.gz mit Bundle-Installer.
4. Lizenz für Eval bindet an MAC-Adresse — auf llvm-Bench-Host
   installieren, nicht auf Workstations.

## Installation auf llvm

```bash
# Auf llvm-Host als root:
mkdir -p /opt/rti
cd /opt/rti
tar -xzf ~/Downloads/rti_connext_dds-eval.tar.gz
cp ~/Downloads/rti_license.dat /opt/rti/rti_license.dat
echo 'export NDDSHOME=/opt/rti/connextdds-X.Y.Z'           >> /etc/profile.d/rti.sh
echo 'export RTI_LICENSE_FILE=/opt/rti/rti_license.dat'    >> /etc/profile.d/rti.sh
echo 'export PATH=$NDDSHOME/bin:$PATH'                     >> /etc/profile.d/rti.sh
```

Verifikation:

```bash
. /etc/profile.d/rti.sh
rtiddsping -help    # sollte version + flags zeigen
```

## Test-Skelett (vorbereitend)

`tests/interop/rti_connext_matrix.sh` (zu erstellen wenn Eval da):

```bash
#!/usr/bin/env bash
# Skip wenn RTI nicht installiert.
if [ -z "${NDDSHOME:-}" ] || [ ! -x "$NDDSHOME/bin/rtiddsping" ]; then
    echo "[ts2-rti] SKIP: NDDSHOME nicht gesetzt oder rtiddsping fehlt" >&2
    exit 77
fi

# Cross-Vendor Pub/Sub-Roundtrip:
# - ZeroDDS-Pub vs RTI-Sub (rtiddspong)
# - RTI-Pub (rtiddsping) vs ZeroDDS-Sub
# - 5 Samples pro Richtung mindestens
```

## CI-Job-Stub

`live-interop-rti` in `.gitlab-ci.yml`:

```yaml
live-interop-rti:
  stage: interop
  resource_group: dcps-multicast
  rules:
    - if: '$RTI_CONNEXT_AVAILABLE == "true"'
      when: on_success
    - when: never   # default skip
  script:
    - bash tests/interop/rti_connext_matrix.sh
```

`RTI_CONNEXT_AVAILABLE`-Variable wird pro Pipeline gesetzt sobald die
Lizenz auf glr1 oder llvm vorhanden ist.

## Wire-Capture-Vorgehen

1. `tcpdump -i any -w rti_baseline.pcap port 7400 or portrange 7401-7500`
   während rtiddsping läuft.
2. tshark-Dissector für RTPS verifizieren:
   `tshark -r rti_baseline.pcap -Y rtps -V > rti_decode.txt`
3. Cross-Vendor: `ZeroDDS-Pub` + `rtiddssub` mitlaufen lassen, gleiche
   pcap. Verifizieren dass beide Seiten die gleichen Submessages
   austauschen.
4. Diff gegen Cyclone-Capture für Wire-Drift-Detection.

## Folgeschritte (sobald Lizenz da)

* [ ] Lizenz-Beschaffung
* [ ] Connext-Install auf llvm
* [ ] `tests/interop/rti_connext_matrix.sh` schreiben
* [ ] CI-Job `live-interop-rti` aktivieren
* [ ] Wire-Capture-Baseline gegen Cyclone diffen
* [ ] Bekannte Drift-Punkte in `docs/architecture/rtps-vendor-quirks.md`
      dokumentieren
