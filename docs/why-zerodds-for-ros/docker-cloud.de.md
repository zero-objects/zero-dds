# Docker / Kubernetes / Cloud

← [Zurück zur Übersicht](index.md)

## Der Schmerz

Containerisiertes und Cloud-ROS-2 vervielfacht das Discovery-Problem (**19 Reports**): Container-Network-Namespaces, Overlay-Netze und Kubernetes-CNIs lassen UDP-Multicast per Default nicht durch, sodass DDS-Discovery still über Pods/Container hinweg scheitert.

- Nodes in verschiedenen Containern können sich ohne `host`-Networking oder einen hand-gebauten Discovery-Server nicht entdecken.
- Ein Simulator/Container kann die ihm gegebene DDS-Config ignorieren und unerreichbar bleiben.
- Wenn ein Host sowohl WiFi als auch Ethernet hat, scheitert ein containerisierter Node bei der Registrierung, weil das falsche Interface announct wird.
- Multicast durch Kubernetes (Cilium und Freunde) zu bekommen ist ein eigenes Projekt.

### Jüngstes Beispiel

**[IsaacSim#407 — „Isaac Sim in Docker unreachable and ignores CycloneDDS config"](https://github.com/isaac-sim/IsaacSim/issues/407)** (2026-01-09). Eine containerisierte Isaac-Sim-Instanz ist über ROS 2 unerreichbar und befolgt die ihr gegebene Cyclone-DDS-Konfiguration nicht — Discovery-in-Containern, das genau so scheitert, wie der Cluster vorhersagt.

### Referenzliste (jüngste zuerst)

| Datum | Quelle | Problem |
|---|---|---|
| 2026-01-09 | [IsaacSim#407](https://github.com/isaac-sim/IsaacSim/issues/407) | Container unerreichbar, ignoriert DDS-Config |
| 2024-10-23 | [rmw_fastrtps#786](https://github.com/ros2/rmw_fastrtps/issues/786) | Docker-Host-Net-Node scheitert bei Registrierung mit WiFi+Ethernet |
| 2024-03-27 | [ROS Discourse](https://discourse.openrobotics.org/t/ros-2-dds-flying-in-cloud-with-cilium-kubernetes/36845) | DDS-Multicast unter Cilium/Kubernetes zum Laufen bringen |
| 2024-02-17 | [create3 discussion #549](https://github.com/iRobotEducation/create3_docs/discussions/549) | Discovery-Server-Config nötig für ROS 2 in Docker |
| 2024-02-14 | [ROS Discourse](https://discourse.openrobotics.org/t/ros-2-fast-dds-discovery-server-with-kubernetes/36086) | Discovery-Server-Turnübungen für Kubernetes |

## Wie ZeroDDS es löst

**Unicast-Discovery + Interface-Pinning ist genau das Modell, das Container und Clouds wollen.**

- **Kein Multicast nötig, nirgends.** `ZERODDS_NO_MULTICAST=1` + `ZERODDS_PEERS` ist Unicast end-to-end, was genau das ist, was Overlay-Netze und Kubernetes-CNIs *durchlassen*. Du adressierst Pods/Container per IP/Service — kein Multicast, das das CNI droppen könnte, kein Discovery-Server-Pod zu betreiben.
- **Interface-Pinning fixt den Multi-Interface-Registrierungs-Fehler.** `ZERODDS_INTERFACE=<ip>` bindet und announct ein Interface über alle Transporte, sodass der „WiFi+Ethernet-Host in Docker scheitert bei Registrierung"-Fehler ([rmw_fastrtps#786](https://github.com/ros2/rmw_fastrtps/issues/786)) ein Ein-Variablen-Fix ist.
- **Config wird befolgt, nicht ambient.** Discovery-Konfiguration ist explizite Umgebungsvariablen, beim Start gelesen — es gibt kein separates XML, das die Runtime still ignorieren kann.
- **TCP, wo Overlays es bevorzugen.** Ein First-Class-TCP-Transport ist für Netze verfügbar, die nur TCP sauber forwarden.

## Warum es kein Schmerz mehr sein muss

Container-/Cloud-Schmerz ist *Multicast durch Netze, die Multicast nicht forwarden*. Die default-fähige Unicast-Discovery von ZeroDDS plus Interface-Pinning bildet direkt darauf ab, wie Container- und Cloud-Networking Traffic tatsächlich routet — das Deployment wird „liste die Peer-IPs", nicht „betreibe einen Discovery-Server und kämpfe mit dem CNI".

## Selbst reproduzieren

```bash
# Unicast, multicast-freie Discovery (das Container-/Cloud-Modell), cross-vendor:
crates/ros2-rmw/interop/run_multicast_free_xvendor.sh
```

→ [Zurück zur Übersicht](index.md) · Weiter: [Security](security.md)
