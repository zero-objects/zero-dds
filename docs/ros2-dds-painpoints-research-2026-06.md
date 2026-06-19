# ROS / ROS 2 ↔ DDS Pain-Point Research — Field Scan

**Compiled:** 2026-06-09 · **Method:** 20-angle parallel web sweep (ROS Discourse, GitHub issues across Fast-DDS / Cyclone DDS / rmw_*, Reddit, Robotics Stack Exchange, vendor blogs, ROS Answers) · **Items:** 349 (deduplicated by URL from 509 raw hits across 20/20 angles).

> Raw field reports of problems people hit running ROS/ROS 2 on DDS middleware. Each item: date · source · headline · precise problem summary. Grouped by category, newest first within each group. This is the unfiltered long list (input for the strategy page in `ros2-dds-painpoints-strategy.md`).

## Categories

| Category | Items |
|---|---|
| Discovery (multicast SDP, discovery storms, nodes not found) | 62 |
| Shared Memory (Iceoryx/SHM segfaults, /dev/shm, same-host fails) | 52 |
| QoS Silent No-Match (incompatible QoS → no data, no error) | 36 |
| Multicast / WiFi (blocked, floods, dropouts) | 34 |
| Cross-Vendor / Inter-Distro Interop | 32 |
| Large Data / Fragmentation (images, point clouds, 262 kB ceiling) | 29 |
| DDS-Security / SROS2 | 22 |
| Configuration Complexity (XML tuning, hidden prerequisites) | 21 |
| Docker / Kubernetes / Cloud | 19 |
| Performance / Latency / CPU Overhead | 19 |
| Scaling / Fleets / Many Nodes | 16 |
| Migration to Zenoh / Alternative Middleware | 7 |
| **Total** | **349** |

---

## Discovery (multicast SDP, discovery storms, nodes not found)

*62 items*

**1. Unexpected piggyback HB to all matched readers breaks EDP recovery loop after sleep/wake cycle**  
2026-05-18 · GitHub eProsima/Fast-DDS#6401 · [link](https://github.com/eProsima/Fast-DDS/issues/6401)  

In a three-node Simple Discovery topology, after a system sleep/wake cycle, one pair of nodes permanently fails to re-match. The root cause is that during EDP recovery of a node-A to node-C link, the async send path in `deliver_sample_to_network` with `m_separateSendingEnabled=false` broadcasts a piggyback Heartbeat to ALL matched readers (including node-B), which corrupts node-B's re-match state machine.

**2. Remote RTPS reader/writer no longer discovered in Fast DDS version: 3.5.0+**  
2026-05-11 · GitHub eProsima/Fast-DDS#6346 · [link](https://github.com/eProsima/Fast-DDS/issues/6346)  

A federation of fast-discovery-servers bridging two machines via separate unicast ports worked reliably below Fast-DDS 3.5.0 but broke in 3.5.0+. Remote readers/writers are no longer reported via `on_reader_discovery`/`on_writer_discovery` callbacks; only local endpoints are discovered. Data still flows once a match exists, but new remote endpoints are invisible to the application.

**3. The new DataReader does not receive data after restarting the Discovery Server**  
2025-10-14 · GitHub eProsima/Fast-DDS#5872 · [link](https://github.com/eProsima/Fast-DDS/issues/5872)  

After stopping and restarting both the Fast-DDS Discovery Server and a subscriber application, the new DataReader never receives data even though the publisher continues running. This is another instance of EDP state not recovering after a server restart, reproducible with the official DiscoveryServerExample.

**4. Listener demo node doesn't receive message only with rmw_cyclonedds, but works fine with rmw_fastrtps_cpp**  
2025-06-23 · GitHub ros2/rmw_cyclonedds#541 · [link](https://github.com/ros2/rmw_cyclonedds/issues/541)  

A ROS 2 Humble installation built from source on Ubuntu 22.04 fails to receive any messages in the listener demo node when using rmw_cyclonedds_cpp; the talker publishes but the listener never fires. Switching to rmw_fastrtps_cpp on the same build makes talker-listener work immediately. No error is shown, indicating a silent CycloneDDS discovery or transport failure in the source-built environment.

**5. Built a free CLI to diagnose ROS2 multi-machine setups: pip install ros2forge**  
2025-05-22 · ROS Discourse (discourse.openrobotics.org) · [link](https://discourse.openrobotics.org/t/built-a-free-cli-to-diagnose-ros2-multi-machine-setups-pip-install-ros2forge/55017)  

A diagnostic CLI tool (ros2forge) is released specifically to address the frequency of ROS 2 multi-machine setup failures, identifying the most common causes: DDS discovery failures, UFW silently blocking UDP traffic on port 7400, clock drift causing DDS handshake failures between machines, multicast configuration issues, and DDS port unavailability. Nodes remain invisible across devices even when ping succeeds — illustrating that network-level reachability does not guarantee DDS discovery works. The tool checks ROS2 installation, DDS/RMW configuration, network interfaces, NTP synchronization, daemon health, and UDP peer reachability.

**6. ROS2 jazzy with ROS_DISCOVERY_SERVER and ROS_SUPER_CLIENT [TCP/UDP] 2 PC physical Server and Client ros2 node list - EMPTY by ros2 topic list OK**  
2024-11-03 · GitHub ros2/ros2#1617 · [link](https://github.com/ros2/ros2/issues/1617)  

On ROS 2 Jazzy with FastDDS Discovery Server and SUPER_CLIENT configuration over both TCP and UDP (localhost and remote IP), 'ros2 topic list' correctly shows topics like /chatter but 'ros2 node list' consistently returns empty — with and without the daemon. This inconsistency between topic and node enumeration in the discovery mechanism persists across all transport configurations tested. No root cause is identified in the issue report.

**7. Previously configured peer never gets undiscovered, even if removed from the peer list**  
2024-10-30 · GitHub ros2/rmw_cyclonedds#520 · [link](https://github.com/ros2/rmw_cyclonedds/issues/520)  

When using unicast static-peer discovery in ROS 2 Humble with 250+ nodes, removing a peer (robot) from the CYCLONEDDS_URI peer list does not stop it from being rediscovered. The removed peer continues sending INFO_TS messages to the local PC, causing it to re-appear in ros2 node list indefinitely. The only remedy is killing all ROS 2 processes on the robot, making dynamic fleet management impractical.

**8. When these processes start at the same time, many dropped packets were generated by the 127.0.0.1 network**  
2024-07-09 · GitHub eProsima/Fast-DDS#4668 · [link](https://github.com/eProsima/Fast-DDS/issues/4668)  

Running 20 processes with 130 topics concurrently on one machine (Fast-DDS v2.12.0, UDP whitelist = 127.0.0.1 for discovery + SHM for data) generates massive loopback packet loss during startup. Increasing OS socket buffers to 200 MB and raising `txqueuelen` to 10000 has no effect, showing that the PDP broadcast storm during simultaneous startup saturates the loopback interface.

**9. Overcoming Wireless Communication Challenges in Robotics**  
2024-06-28 · ZettaScale Technologies news · [link](https://www.zettascale.tech/news/overcoming-wireless-communication-challenges-in-robotics/)  

ZettaScale describes DDS peer-to-peer architecture requiring all nodes to be aware of all others, causing scalability problems as fleet size grows. DDS discovery requires complete remote-node discovery leading to resource-intensive startup and network flooding. Each DDS node exposes an open UDP port increasing the attack surface. Request/reply patterns double topics because DDS uses pub/sub for both directions.

**10. CPU spikes of existing nodes when starting new node**  
2024-02-16 · GitHub ros2/rmw_fastrtps#741 · [link](https://github.com/ros2/rmw_fastrtps/issues/741)  

In a production system running ~70 nodes, launching a single new node causes all existing nodes to spike to approximately double their CPU consumption for several seconds. The issue is not present with CycloneDDS, suggesting it is specific to FastDDS's discovery process re-announcing all endpoints when a new participant joins. The issue remained open with no assignees or linked fixes.

**11. ROS Discovery server - topics fail to recover from network disruptions**  
2024-02-09 · GitHub eProsima/Fast-DDS#4111 · [link](https://github.com/eProsima/Fast-DDS/issues/4111)  

In a multi-robot ROS 2 Galactic system using Fast-DDS Discovery Server over WiFi, brief network outages permanently disrupt topic flow. After the WiFi recovers, `ros2 topic list` shows the topics as visible but no messages ever arrive. Only restarting the affected ROS nodes restores communication, making this a production reliability issue for mobile robotics.

**12. Configure Cyclone 0.10.0 to use multiple network interfaces**  
2024-01-19 · GitHub eclipse-cyclonedds/cyclonedds#1915 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/1915)  

On a machine with two NICs (192.168.4.100 and 192.168.0.100), only the machine connected to whichever interface appears first in the XML config can see published topics; the other machine is invisible. Changing the order of interfaces in the XML flips which subnet is reachable. A regression from Galactic (0.8.0) to Humble (0.10.0) — previously both subnets were reachable simultaneously. There is no working configuration to make topics visible on both connected subnets at once.

**13. Problem using VLAN tagging**  
2023-11-14 · GitHub eclipse-cyclonedds/cyclonedds#1834 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/1834)  

When CycloneDDS is configured with a VLAN tag, the tag is applied to the user-data multicast locator but not to the metatraffic (discovery) multicast locator. This asymmetry means discovery packets travel untagged on the wrong VLAN while data packets are correctly tagged, causing complete failure to establish communication in VLAN-segmented network environments.

**14. DDS Fails Discovery with containers in different hosts**  
2023-10-27 · GitHub eclipse-cyclonedds/cyclonedds#1454 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/1454)  

CycloneDDS discovery works fine between containers on the same host but fails completely when containers are on different hosts. The interface eth0 inside the container is not multicast-capable, causing CycloneDDS to disable multicast and emit a warning. Even with a peer-based unicast config, data traffic does not flow between hosts although same-host container communication is functional.

**15. the problem in the ipv6 environment**  
2023-08-31 · GitHub eclipse-cyclonedds/cyclonedds#1820 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/1820)  

Two Windows 10 machines connected directly with an Ethernet cable and configured for IPv6-only cannot discover each other: CycloneDDS falls back to the loopback interface, which is not multicast-capable, and emits 'disabling multicast'. UDP multicast works correctly in standard socket programs on the same machines, pointing to CycloneDDS's interface-selection logic failing to recognize the IPv6 link-local Ethernet interface as the correct one.

**16. Cyclone DDS is not allowing multi-machine discovery of nodes in ROS2 Humble**  
2023-07-25 · ROS Answers (answers.ros.org / robotics.stackexchange.com) · [link](https://answers.ros.org/question/417725/cyclone-dds-is-not-allowing-multimachine-discovery-of-nodes-in-ros2-humble/)  

After switching to rmw_cyclonedds_cpp on a Raspberry Pi 4 and host Ubuntu 22.04 system, nodes on the Pi are invisible from the host even with ROS_DOMAIN_ID and CYCLONEDDS_URI set. Root cause was CycloneDDS defaulting to an incorrect network interface; the fix requires an explicit cyclonedds.xml specifying the wlan0/wlo1 interface with DontRoute=true before restarting the ROS daemon.

**17. FastDDS Discovery Server does not work locally without a network connection [13652]**  
2023-06-22 · GitHub eProsima/Fast-DDS#2031 · [link](https://github.com/eProsima/Fast-DDS/issues/2031)  

Running `fastdds discovery -i 0` on ROS 2 Galactic/Foxy when all network interfaces (WiFi + Ethernet) are disabled fails immediately with a cryptic 'fast-discovery-server tool not found' message instead of a meaningful error. The RTPS participant cannot bind without a real network interface, making offline development impossible.

**18. [18257] Version compatibility issue for participant discovery when upgrading to version 2.10.1**  
2023-06-13 · GitHub eProsima/Fast-DDS#3460 · [link](https://github.com/eProsima/Fast-DDS/issues/3460)  

Applications compiled against Fast-DDS 2.10.0 using the `on_participant_discovery` callback (2-parameter form) silently stop receiving discovery events when the runtime library is upgraded to 2.10.1. The ABI change moved the callback dispatch logic to a new 3-parameter overload in the shared library, breaking all binaries compiled against the old header without any error or warning.

**19. ROS2 Humble DDS not working on two different machines**  
2023-06-07 · ROS Answers (answers.ros.org / robotics.stackexchange.com) · [link](https://answers.ros.org/question/416272/ros2-humble-dds-not-working-on-two-different-machine/)  

Two Ubuntu Humble machines that can successfully ping each other fail to exchange ROS2 messages. DDS relies on UDP multicast for participant discovery and the two machines' multicast traffic is not bridged correctly. The responder suggests testing multicast reachability with 'ros2 multicast send/receive' and verifying ROS_DOMAIN_ID consistency, as DDS discovery silently falls back to no-communication when multicast is blocked.

**20. Endpoints discovery doesn't recover after Discovery server re-start [13985]**  
2023-04-13 · GitHub eProsima/Fast-DDS#2534 · [link](https://github.com/eProsima/Fast-DDS/issues/2534)  

After killing and restarting the Fast-DDS Discovery Server, EDP (endpoint discovery) permanently breaks for already-running participants on Fast-DDS 2.5.0. Newly joined entities are not discovered and 'entity lost' events stop firing. The bug traces to a race where a Heartbeat with a high sequence number from the DS built-in writer arrives after PDPClient has already reset its history on the DS-lost event.

**21. Connectivity issue — TurtleBot4 only shows 2 of 40+ topics over WiFi**  
2023-04-08 · GitHub turtlebot/turtlebot4#137 · [link](https://github.com/turtlebot/turtlebot4/issues/137)  

A TurtleBot4 with Create3 on 2.4GHz WiFi and Raspberry Pi on 5GHz WiFi shows only /parameter_events and /rosout on the operator's PC while the RPi sees 40+ topics including /scan, /odom, /imu. Both CycloneDDS and FastDDS were tried without success. The mixed-band WiFi topology prevents DDS discovery from propagating the full topic set across the network boundary.

**22. ros2cli tools not working with Discovery Server through a router (cross-machine)**  
2023-02-06 · GitHub ros2/rmw_fastrtps#668 · [link](https://github.com/ros2/rmw_fastrtps/issues/668)  

The documented workaround for issue #499 (configuring the ros2 daemon to also use the Discovery Server) only works when all machines are on the same host. When two machines are connected through a router, the ros2cli daemon does not pass topic or node information across the router boundary even though talker/listener communication works, leaving introspection tools blind in multi-machine setups.

**23. Proposed changes to how ROS performs discovery of nodes**  
2022-10-05 · ROS Discourse (discourse.openrobotics.org) · [link](https://discourse.openrobotics.org/t/proposed-changes-to-how-ros-performs-discovery-of-nodes/27640)  

Open Robotics formally documents two critical default DDS behavior problems: (1) nodes discover unrelated nodes too easily, causing unintended robot motion when unrelated robots share a network, and (2) discovery traffic floods the entire network by default, capable of bringing networks down or degrading performance significantly. The proposal introduces ROS_AUTOMATIC_DISCOVERY_RANGE with values for off/localhost/subnet and ROS_STATIC_PEERS to address uncontrolled discovery scope; the default would shift to localhost-only to prevent multicast from escaping the host.

**24. ROS2 Multicast works but nodes can't communicate or see each other over multiple machines**  
2022-09-28 · ROS Answers archive · [link](https://answers.ros.org/question/405451/)  

Users report that even verified multicast connectivity does not guarantee ROS2 DDS node discovery across machines; the paradox of working OS-level multicast yet silent DDS discovery affects both Fast DDS and Cyclone DDS. This is a recurring WiFi/mixed-network problem where the DDS discovery layer fails without any error message or log entry pointing to the root cause.

**25. DDS discovery not working with ROS2 Humble and Fast DDS when nodes are on same machine**  
2022-09-27 · ROS Answers archive · [link](https://answers.ros.org/question/407025/dds-discovery-not-working-with-ros2-humble-and-fast-dds-when-nodes-are-on-same-machine/)  

FastDDS discovery fails between two ROS 2 Humble nodes on the same Linux machine (Ubuntu 22.04, VirtualBox VM and Raspberry Pi4): 'ros2 node list' finds no nodes and talker/listener demo fails entirely. The same setup works correctly with CycloneDDS, and cross-machine FastDDS communication also works — the failure is specific to FastDDS intra-host discovery on Linux. No permanent FastDDS fix is identified in the thread; switching to CycloneDDS is the only confirmed solution.

**26. DDS discovery not working with ROS2 Humble and Fast DDS when nodes are on same machine**  
2022-09-27 · ROS Answers answers.ros.org/question/407025/ · [link](https://answers.ros.org/question/407025/)  

IamNotaRoBot found that two ROS 2 Humble nodes on the same Ubuntu 22.04 machine (both VM and Raspberry Pi 4) cannot discover each other using FastDDS, while cross-machine communication works fine and switching to CycloneDDS resolves the problem. Suspected causes include shared memory configuration conflicts and firewall interactions. The issue persisted across multiple Humble patch releases without resolution.

**27. ROS2 Humble DDS not working on two different machines**  
2022-09-22 · ROS Answers archive · [link](https://answers.ros.org/question/416272/)  

Users on Humble with Cyclone DDS cannot achieve node discovery between two machines on the same WiFi network despite the ros2multicast tool confirming multicast works. The Cyclone DDS rmw layer silently fails to discover peers even when UDP multicast packets are flowing at the OS level, requiring manual peer list configuration as a workaround.

**28. Cannot see topics when subscriber is on ethernet and host is connected over WiFi on the same network**  
2022-09-01 · GitHub ros2/ros2#1319 · [link](https://github.com/ros2/ros2/issues/1319)  

On a mixed network where a Windows PC uses Ethernet and a Raspberry Pi uses WiFi, Fast DDS multicast discovery fails silently—neither side can see the other's nodes or topics despite being on the same LAN. Direct IP communication (ping) works normally. The issue disappears when the Ethernet machine also enables its WiFi adapter, suggesting Fast DDS multicast does not traverse between interface types on the same local network.

**29. ROS2 Galactic — Changing RMW to FastRTPS causes errors**  
2022-08-26 · ROS Answers (answers.ros.org / robotics.stackexchange.com) · [link](https://answers.ros.org/question/405582/ros2-galactic-changing-rmw-to-fastrtps-causes-errors/)  

Switching from CycloneDDS to rmw_fastrtps_cpp in ROS2 Galactic causes lifecycle nodes to fail during startup with 'cannot publish data' on lifecycle transition topics. The error manifests as 'string capacity not greater than size' in rosidl_generator_c_String, and appears to be triggered by Fast DDS discovery traffic overwhelming lifecycle state publication during node initialization. CycloneDDS or synchronous-publish mode workarounds are required.

**30. Static Discovery Support — DataReader and DataWriter never match with Static EDP**  
2022-06-24 · GitHub ros2/rmw_fastrtps#617 · [link](https://github.com/ros2/rmw_fastrtps/issues/617)  

Attempting to use Fast DDS Static EDP (Endpoint Discovery Protocol) with ROS 2 Humble Talker/Listener nodes via XML profile results in participants appearing in Fast DDS Monitor but DataWriter and DataReader never matching, so no messages are exchanged. Dynamic discovery works fine; static EDP configuration through FASTRTPS_DEFAULT_PROFILES_FILE appears broken for ROS 2 topic names.

**31. FastDDS without Discovery Server?**  
2022-06-21 · ROS Discourse (discourse.openrobotics.org) · [link](https://discourse.openrobotics.org/t/fastdds-without-discovery-server/26117)  

Original poster reports that on ROS 2 Humble with FastDDS, approximately 75% of the time after restarting launch files, certain topics (especially tf frames) fail to connect; switching to CycloneDDS eliminates the problem entirely. Other respondents describe services not responding, nodes not being discovered at all, and the listener/talker demo failing on a single machine with FastDDS while working fine with CycloneDDS. A hidden prerequisite is documented: localhost-only mode silently requires multicast enabled on the loopback interface via 'ip link set lo multicast on', information described as nearly impossible to find.

**32. Unconfigured DDS considered harmful to Networks**  
2022-05-24 · ROS Discourse (discourse.openrobotics.org) · [link](https://discourse.openrobotics.org/t/unconfigured-dds-considered-harmful-to-networks/25689)  

User joshnewans reports that default FastDDS on Foxy, when running on a Linux machine with multiple network interfaces (real NIC plus Docker/VMware virtual interfaces), transmitted all discovered interface IPs to the publisher; the publisher then sent four 64-layer LiDAR streams — hundreds of megabits — to each address, routing traffic for virtual interfaces out to the internet and saturating external bandwidth for days. The thread establishes that unconfigured DDS multicast floods wired and wireless networks until they grind to a halt, and documents multiple teams accidentally bringing down their lab network by running ROS 2 without interface-restriction configuration.

**33. Memory leak when using ROS 2 actions**  
2022-04-13 · GitHub ros2/rmw_cyclonedds#388 · [link](https://github.com/ros2/rmw_cyclonedds/issues/388)  

Each ROS 2 action invocation via CycloneDDS causes a steady increase in virtual memory that is never deallocated, eventually crashing robots after hours of continuous action commands. The leak is specific to CycloneDDS and does not occur with FastDDS, and is most pronounced when action servers reject goal requests. The issue was observed on the iRobot Create 3 running ROS 2 Galactic on Ubuntu 20.04.

**34. How to configure multiple network**  
2022-03-10 · GitHub eclipse-cyclonedds/cyclonedds#1190 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/1190)  

Machine A has two NICs (192.168.1.1 and 192.168.2.1) configured as subscriber. Publisher B on 192.168.1.2 can reach A, but publisher C on 192.168.2.2 cannot — C's DATA packets are sent to 192.168.1.1 instead of the intended 192.168.2.1. CycloneDDS does not correctly route unicast data to the right interface when a participant has multiple addresses, causing silent message loss for one subnet.

**35. ROS 2 command line introspection tools don't work when AllowMulticast is set to false**  
2022-03-03 · GitHub ros2/rmw_cyclonedds#376 · [link](https://github.com/ros2/rmw_cyclonedds/issues/376)  

Setting AllowMulticast=false in a CycloneDDS XML config (needed to avoid disrupting internet connectivity) causes ros2 node list and ros2 topic echo to report empty results, even though nodes can communicate directly with each other. Introspection tools rely on multicast discovery internally, so disabling multicast silently breaks all ros2 CLI visibility without any error message.

**36. Bridge Remote DDS Networks With a DDS Router**  
2022-02-03 · Husarnet blog · [link](https://husarnet.com/blog/ros2-dds-router)  

The previous Discovery Server solution exposed all ROS 2 topics from all robots across the fleet with no access control — one compromised robot could read/write all fleet communication. Standard DDS has no built-in inter-network topic filtering, creating both security and operational problems for remote multi-robot deployments. The DDS Router was developed to selectively bridge only chosen topics between isolated networks.

**37. ROS2 foxy Cyclone DDS multiple nodes - unicast configuration**  
2021-08-15 · GitHub eclipse-cyclonedds/cyclonedds#687 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/687)  

When multicast is disabled (required for Docker-to-Windows communication), attempting to start a second ROS 2 node on Windows fails with 'rtps_init: failed to create unicast sockets for domain 0 participant index 0 (ports 7410, 7411)'. Only one participant index is available in unicast mode by default, so the second process cannot bind. Enabling multicast fixes the port-binding issue but breaks Docker discovery.

**38. Connecting Remote Robots Using ROS2, Docker & VPN**  
2021-07-20 · Husarnet blog · [link](https://husarnet.com/blog/ros2-docker)  

Documents that DDS autodiscovery completely fails when ROS 2 nodes are in different networks behind NAT, because multicast cannot cross router boundaries and nodes lack public static IPs. Standard ROS 2 nodes can only discover each other when on the same LAN/Wi-Fi segment. Husarnet VPN with explicit IPv6 peer config and Cyclone DDS XML overrides is required to bridge separate networks.

**39. FastDDS Discovery Server does not work locally without a network connection**  
2021-06-29 · GitHub ros2/rmw_fastrtps#545 · [link](https://github.com/ros2/rmw_fastrtps/issues/545)  

When all network interfaces (Ethernet and WiFi) are disconnected, running `fastdds discovery -i 0` immediately exits with a cryptic error suggesting a missing installation rather than the actual cause: no network interface available. Affects Foxy, Galactic, and Rolling. Discovery Server requires at least one active NIC even for loopback-only use.

**40. Integrating ROS2 with Eclipse zenoh**  
2021-04-28 · Zenoh blog (ZettaScale / Eclipse) · [link](https://zenoh.io/blog/2021-04-28-ros2-integration/)  

Documents that standard ROS 2 discovery relies on UDP multicast which fails behind NATs and across the internet, making remote / WAN robot deployments impossible without per-vendor workarounds. The Zenoh bridge reduces discovery traffic by up to 99.97% and enables internet-scale robot-to-anything (R2X) communication that DDS cannot support natively.

**41. ROS_LOCALHOST_ONLY is not preventing cross-talking between machines**  
2021-04-20 · GitHub ros2/rmw_cyclonedds#370 · [link](https://github.com/ros2/rmw_cyclonedds/issues/370)  

Setting ROS_LOCALHOST_ONLY=1 on ROS 2 Rolling with CycloneDDS fails to isolate the node: remote machines' nodes still appear in ros2 node list and nodes can receive messages from other machines on the network. Using a unique ROS_DOMAIN_ID or disabling the NIC provides isolation; ROS_LOCALHOST_ONLY with CycloneDDS is simply non-functional.

**42. ROS_LOCALHOST_ONLY=1 does not prevent cross-talk with machines on same network**  
2021-04-20 · GitHub ros2/ros2#1131 · [link](https://github.com/ros2/ros2/issues/1131)  

Setting ROS_LOCALHOST_ONLY=1 in CycloneDDS RMW fails to restrict discovery to localhost; nodes from other machines still appear in ros2 node list, some with duplicate names. The environment variable is ignored by the DDS layer's multicast announcements. Using a unique ROS_DOMAIN_ID correctly isolates the system, confirming the localhost-only flag has no effect on the underlying DDS participant advertisement.

**43. Minimising ROS2 Discovery Traffic**  
2021-03-23 · ROS Discourse (discourse.openrobotics.org) · [link](https://discourse.openrobotics.org/t/minimising-ros2-discovery-traffic/19614)  

Thread documents the scale of DDS discovery overhead and the 97-99.9% traffic reduction achievable by switching to Zenoh. The DDS Simple Discovery Protocol generates continuously growing traffic as participant count increases (O(n) with multicast, O(n²) with unicast), making large multi-robot deployments impractical. The Fast DDS Discovery Server v2 is presented as the DDS-native alternative requiring only an environment variable, but Zenoh is shown to also eliminate the need for a centrally deployed service while maintaining peer-to-peer communication.

**44. Minimizing Discovery Overhead in ROS2**  
2021-03-23 · Zenoh Blog zenoh.io/blog/2021-03-23-discovery/ · [link](https://zenoh.io/blog/2021-03-23-discovery/)  

ZettaScale measured DDS discovery traffic during a TurtleBot3 SLAM+RViz2 session: 686 packets totalling 251,576 bytes due to the O(n^2) growth of RTPS discovery where every participant tracks every reader and writer of every other participant. The protocol was designed for wired networks with plentiful bandwidth; Zenoh reduced the same workload to 31 packets / 6,617 bytes (97.37% reduction) with basic configuration and to a single 82-byte packet (99.97%) combining resource generalization and warm start.

**45. ROS2 Fast DDS Discovery — no topics listed**  
2021-03-02 · ROS Answers (answers.ros.org / robotics.stackexchange.com) · [link](https://answers.ros.org/question/373074/ros2-fastdds-discovery-no-topics-listed/)  

When using Fast DDS Discovery Server (ROS_DISCOVERY_SERVER=127.0.0.1:11811), talker and listener nodes communicate successfully but 'ros2 topic list' and 'ros2 node list' return only /parameter_events and /rosout. CLI tools and rqt are not connected to the Discovery Server as super-clients and see none of the application topics, making the standard ROS2 toolchain non-functional in Discovery Server mode.

**46. ros2 topic empty with Fast DDS discovery server**  
2021-03-02 · ROS Answers (answers.ros.org / robotics.stackexchange.com) · [link](https://answers.ros.org/question/373070/ros2-topic-empty-with-fastdds-discovery-server/)  

After following the official Fast DDS Discovery Server tutorial, the talker/listener demo works but 'ros2 topic list' returns empty output and rqt shows no nodes. The problem is a stale daemon cache: the daemon is not configured as a super-client to the Discovery Server and caches no-topic state. Running with --no-daemon or restarting the daemon restores topic visibility.

**47. Restarting nodes causes other nodes to "disappear"**  
2021-02-12 · GitHub ros2/rmw_fastrtps#509 · [link](https://github.com/ros2/rmw_fastrtps/issues/509)  

After repeatedly restarting a launch file (e.g., slam_launch.py) with Fast-RTPS, nodes and topics including /clock become undiscoverable via ros2 CLI and subscriber callbacks stop firing, even though publish() continues executing without error. The discovery database becomes corrupted or stale and does not recover, but the issue does not occur when using Connext DDS, isolating it to the Fast-RTPS RMW implementation.

**48. Reconsider default ip interface**  
2021-01-08 · GitHub eclipse-cyclonedds/cyclonedds#485 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/485)  

Cyclone frequently emits 'using network interface en0 (udp/192.168.1.120) selected arbitrarily from: en0, en1' warnings when a machine has multiple interfaces, because CycloneDDS only publishes one unicast address during discovery instead of all reachable addresses. The result is that peers on the non-selected interface can not connect, and users have no deterministic way to predict which interface will be chosen. The issue was closed after a configuration improvement was added but the arbitrary-selection behavior remained a long-standing complaint.

**49. ros2cli tools for topics, services and actions not functional when using Discovery Server**  
2021-01-07 · GitHub ros2/rmw_fastrtps#499 · [link](https://github.com/ros2/rmw_fastrtps/issues/499)  

After setting up Fast DDS Discovery Server, talker/listener communicate correctly, but `ros2 topic list`, `ros2 node info`, and `ros2 topic echo` return empty results or fail. The ros2 daemon used by CLI tools does not automatically use the ROS_DISCOVERY_SERVER setting, requiring extra XML configuration to make introspection work. Filed against Foxy.

**50. New Discovery Server**  
2020-11-17 · ROS Discourse (discourse.openrobotics.org) · [link](https://discourse.openrobotics.org/t/new-discovery-server/17383)  

eProsima announces the Fast DDS Discovery Server v2, motivated by the failures of the standard RTPS Simple Discovery Protocol (SDP): SDP generates discovery traffic 93% higher than the new server, relies entirely on multicast (unreliable on WiFi), and at deployments of 50-200+ nodes, discovery traffic itself drowns out actual data messages. Users report that 'discovery traffic was so high that actual messages were being drowned out due to the discovery traffic.' ROS 2's unique topology of creating many internal topics per node uniquely exposes these DDS scalability limits not found in traditional DDS commercial applications.

**51. Improve service discovery — race condition between server and client endpoint discovery**  
2020-05-28 · GitHub ros2/rmw_fastrtps#392 · [link](https://github.com/ros2/rmw_fastrtps/issues/392)  

Service discovery in rmw_fastrtps relies on independent topic-based discovery of request/reply endpoints, which creates a race condition where a client may send a request before the server's reader is matched, dropping it silently. The OMG DDS-RPC 1.0 spec defines an Enhanced Service Mapping to prevent this, but rmw_fastrtps uses a non-standard blocking workaround with side-effects on `rmw_send_response`.

**52. Restrict DDS middleware network traffic to localhost**  
2019-10-08 · GitHub ros2/ros2#798 · [link](https://github.com/ros2/ros2/issues/798)  

By default DDS selects all available network interfaces and uses multicast, causing parallel CI tests to inadvertently interfere with each other and creating unnecessary complexity for single-machine setups. Domain ID isolation helps but imposes a maintenance burden and the RTPS spec limits the usable range to 0-232. The issue proposes a ROS_LOCALHOST_ONLY environment variable to force loopback-only communication.

**53. Graph guard condition not triggered when new nodes are discovered**  
2019-09-20 · GitHub ros2/rmw_fastrtps#321 · [link](https://github.com/ros2/rmw_fastrtps/issues/321)  

The participant info callback in rmw_fastrtps never fires the node graph guard condition when a new node is discovered, making it impossible to use event-driven `wait_for_node_to_appear` patterns. Applications must poll `get_node_names()` in a loop instead of being notified, causing high CPU use and unreliable startup sequencing.

**54. Discovery: too slow and high network usage**  
2019-05-23 · GitHub ros2/rmw_fastrtps#281 · [link](https://github.com/ros2/rmw_fastrtps/issues/281)  

With approximately 20 ROS 2 nodes (23 publishers, 35 subscriptions), the Endpoint Discovery Phase hangs indefinitely after all nodes discover each other. Network upload spikes to ~50 KB/s during the discovery storm. Reproduced on Fast-RTPS 1.7.0 and 1.8.0; the issue was traced to a commit changing discovery behavior.

**55. Max value for ROS_DOMAIN_ID**  
2019-03-13 · GitHub ros2/rmw_fastrtps#261 · [link](https://github.com/ros2/rmw_fastrtps/issues/261)  

The admissible ROS_DOMAIN_ID range is 0-232 due to how RTPS maps domain IDs to UDP port numbers, yet Fast-RTPS at the time did not validate this and silently accepted invalid values above 232. Competing implementations like OpenSplice and Connext properly rejected out-of-range values with runtime errors, while Fast-RTPS failed in unpredictable ways, leaving users confused about why high domain IDs caused silent communication failures.

**56. ROS2 nodes can not see one another via network**  
2018-09-27 · ROS Answers (answers.ros.org / robotics.stackexchange.com) · [link](https://answers.ros.org/question/304373/ros2-nodes-can-not-see-one-another-via-network/)  

An early ROS2 cross-machine discovery report where users carrying over ROS1 habits set ROS_MASTER_URI and ROS_HOSTNAME (which are ignored in ROS2) and find no inter-machine communication. The actual mechanism is DDS UDP multicast, which has known reliability problems on WiFi. Restarting the ROS2 daemon on both sides restores visibility, exposing fragile discovery caching behavior present since early ROS2 distributions.

**57. Docker containers on the same buildfarm host interfering with each other**  
2018-04-18 · GitHub ros2/ci#149 · [link](https://github.com/ros2/ci/issues/149)  

When the ROS 2 ARM build farm consolidated from separate AWS machines to parallel Docker executors on a single native host, containers began discovering each other's DDS participants because they shared the default Docker bridge network without distinct ROS_DOMAIN_IDs. Tests detected unexpected nodes from concurrent bridge builds, causing false failures. DDS multicast on the shared bridge network allowed unrelated CI containers to cross-pollinate discovery traffic.

**58. Endpoint re-discovery can fail with un-equal lease times**  
n/a · GitHub eProsima/Fast-DDS#155 · [link](https://github.com/eProsima/Fast-DDS/issues/155)  

If a publisher and subscriber are configured with different liveliness lease durations (e.g. 60 s vs 10 s) and the network is interrupted for a period shorter than the longer lease time, EDP data is never re-exchanged after reconnect. Each participant resumes heartbeats and the one with the longer lease concludes no disconnect happened, so DATA(w)/DATA(r) are never retransmitted and data flow never resumes.

**59. Issue - Publisher/Subscriber not matching [3986]**  
n/a · GitHub eProsima/Fast-DDS#339 · [link](https://github.com/eProsima/Fast-DDS/issues/339)  

With ~5 publishers and 5 subscribers all starting in parallel on one host (each with its own participant), one random publisher/subscriber pair intermittently fails to match — roughly once every 20 restarts. Disabling IPv6 reduced but did not eliminate the failure, pointing to a timing race in SPDP/EDP announcement during concurrent participant creation.

**60. Nodes don't reconnect to TCP discovery server [13706]**  
n/a · GitHub eProsima/Fast-DDS#2299 · [link](https://github.com/eProsima/Fast-DDS/issues/2299)  

In a ROS 2 Galactic setup using TCP discovery server, stopping and restarting a talker node results in permanent communication failure: the restarted node never matches the listener again. The issue reproduces reliably inside Docker and was observed with Fast-DDS 2.3.4.

**61. Nodes hang after discovery server restart [13704]**  
n/a · GitHub eProsima/Fast-DDS#2289 · [link](https://github.com/eProsima/Fast-DDS/issues/2289)  

When using a TCP discovery server with ROS 2 Galactic, stopping and restarting the discovery server causes ROS nodes to hang indefinitely on SIGINT (Ctrl+C) instead of shutting down cleanly. The node receives the signal but never exits, blocking CI pipelines and robotic system restarts.

**62. Fast-DDS fails to transmit messages between containers on same Kubernetes pod [10123]**  
n/a · GitHub eProsima/Fast-DDS#1633 · [link](https://github.com/eProsima/Fast-DDS/issues/1633)  

Two Docker containers in the same Kubernetes pod share the same external IP and independently start as PID 2, producing identical Fast-DDS participant GUIDs (derived from MD5 of IP + PID). Participant matching either fails outright or silently switches to intra-process delivery even though the processes are in separate containers, so no messages are ever transferred.

---

## Shared Memory (Iceoryx/SHM segfaults, /dev/shm, same-host fails)

*52 items*

**63. Variable-size types deliver zero samples over PSMX (iceoryx) and pin a core at 100% CPU**  
2026-06-02 · GitHub ros2/rmw_cyclonedds#585 · [link](https://github.com/ros2/rmw_cyclonedds/issues/585)  

With PSMX/iceoryx shared-memory enabled, variable-length message types (e.g. std_msgs/String) receive zero samples and peg one CPU core at 100%. Fixed-size types work fine. The bug is a CDR header accounting mismatch: the buffer-size function allocates space for the payload only, the serializer writes a 4-byte CDR header too, overflowing the buffer; the deserializer then gets a truncated stream, logs 'CDR deserialization: truncated input', and the executor spins retrying the stuck sample infinitely.

**64. Data-sharing VOLATILE DataReader can loop forever in ReaderPool::init_shared_segment()**  
2026-03-21 · GitHub eProsima/Fast-DDS#6338 · [link](https://github.com/eProsima/Fast-DDS/issues/6338)  

With Fast-DDS data-sharing (SHM zero-copy), a late-joining VOLATILE DataReader can spin at 100% CPU indefinitely inside ReaderPool::init_shared_segment() when the matched DataWriter is actively publishing at high rate. The fast-forward loop re-reads the writer's live end() position on every iteration; if the reader thread is preempted, it chases a moving target and never terminates, blocking all discovery and transport threads that are waiting on the PDP mutex.

**65. Spurious 'Buffer is being invalidated, segment_size may be insufficient' warning with SHM transport**  
2025-12-02 · GitHub eProsima/Fast-DDS#6206 · [link](https://github.com/eProsima/Fast-DDS/issues/6206)  

With Fast-DDS SHM transport, after normal connection establishment and data exchange between two nodes, the log repeatedly emits 'Buffer is being invalidated, segment_size may be insufficient' even when no data loss is observed. The warning appears to be a false positive triggered by internal buffer lifecycle management, but there is no documentation or API to distinguish a genuine size problem from a benign invalidation event, causing confusion for operators monitoring production systems.

**66. umask setting has not taken effect for some SHM files, blocking cross-user access**  
2025-11-10 · GitHub eProsima/Fast-DDS#6162 · [link](https://github.com/eProsima/Fast-DDS/issues/6162)  

Even when a ROS 2 talker node is launched with umask 0000, some /dev/shm fastrtps_* files are still created with restrictive permissions (not world-writable), preventing a subscriber running as a different user from accessing them. The umask is applied inconsistently across different file types in the SHM transport implementation (segment files vs. port files vs. semaphores), leaving no reliable way to grant cross-user SHM access without manually chmod-ing every file after startup.

**67. SHM transport: Failed init_port fastrtps_port (mutex timeout race)**  
2025-10-22 · GitHub eProsima/Fast-DDS#6117 · [link](https://github.com/eProsima/Fast-DDS/issues/6117)  

Under heavy system load or slow hardware, Fast-DDS SHM port initialisation hits a hardcoded 2-second mutex timeout and then silently destroys and re-creates the SHM port mutex files ('breaks the mutex schema'), rather than failing cleanly. On systems with intensive discovery (many participants) or slow I/O, 2 seconds is insufficient; the silent recreation corrupts the locking invariant and other processes holding the old mutex reference get undefined behaviour. The timeout is not configurable.

**68. Set useBuiltinTransports to false causes Segmentation fault on shutdown**  
2025-10-21 · GitHub eProsima/Fast-DDS#6114 · [link](https://github.com/eProsima/Fast-DDS/issues/6114)  

With Fast-DDS v3.2.0 on Ubuntu 20.04 (ARM64), configuring a participant with useBuiltinTransports=false and only SHM transport (100 MB segment, 2 MB max message) causes a segmentation fault when the process exits via Ctrl-C. The crash is in SharedMemGlobal.hpp remove_port(): a port in watched_ports_ has already been freed before remove_port() is called, resulting in a double-free. Switching to UDP or keeping builtin transports enabled avoids the crash.

**69. Data Race in DataSharingPayloadPool: Inconsistent has_been_removed() State**  
2025-04-14 · GitHub eProsima/Fast-DDS#5762 · [link](https://github.com/eProsima/Fast-DDS/issues/5762)  

In the DataSharingPayloadPool (Fast-DDS SHM zero-copy path), a data race exists between release_payload() and advance_till_first_non_removed(): a PayloadNode that has_been_removed()=true in release_payload() is observed as has_been_removed()=false by the concurrent advance function, which then resets the flag. The unsynchronised flag access (no atomic or mutex) can cause use-after-free or incorrect payload delivery to readers sharing the same memory segment.

**70. [22903] discovery_server example does not work with SHM**  
2025-02-27 · GitHub eProsima/Fast-DDS#5670 · [link](https://github.com/eProsima/Fast-DDS/issues/5670)  

In Fast-DDS v3.1.2 on Ubuntu 22.04 and ARM, running the built-in discovery_server example with '--transport shm' causes discovery to fail entirely: the subscriber and publisher cannot find each other, while the same example works without the --transport flag. The SHM transport and discovery-server mode are incompatible because discovery-server uses unicast metatraffic locators that are not mapped to SHM locators, leaving SHM participants undiscovered.

**71. [22647] Shared Memory Object Deleted When Calling system() in QNX Environment**  
2025-01-14 · GitHub eProsima/Fast-DDS#5574 · [link](https://github.com/eProsima/Fast-DDS/issues/5574)  

On QNX 7.1 (aarch64) with Fast-DDS 2.12.2, calling system() inside a FastDDS-based publisher application causes the SHM segment file to disappear from /dev/shmem/, after which subscribers stop receiving published data. The problem is a QNX POSIX-incompatibility: system() forks a process that inherits and then closes the SHM file descriptors, which on QNX unlinks the segment. This makes the default SHM transport unreliable in any application that uses system() or popen().

**72. [22594] Killing one subscriber with kill -9 affects other subscribers; data not received after restart**  
2024-12-09 · GitHub eProsima/Fast-DDS#5469 · [link](https://github.com/eProsima/Fast-DDS/issues/5469)  

With Fast-DDS 2.14.3 on Ubuntu 22.04, a publisher writes ~3 MB RGB frames at 30 Hz over SHM to multiple subscribers. When one subscriber is kill -9'd under CPU/IO stress (stress-ng), the remaining subscribers stop receiving data and the restarted subscriber also fails to receive, even though the writer API returns OK. The SHM segment file is sometimes deleted or becomes inaccessible, and the DataWriter has no mechanism to recover or re-create the SHM state.

**73. Full shared memory files cleanup after application crash**  
2024-10-31 · GitHub eProsima/Fast-DDS (Discussion #5373) · [link](https://github.com/eProsima/Fast-DDS/discussions/5373)  

On an embedded Linux system using Fast-DDS with SHM+UDP transport, after a program runtime error the files fastrtps_portXXXX and sem.fastrtps_portXXXX_mutex persist in /dev/shm even after running 'fastdds shm clean'. The resource-constrained environment makes the leftover files critical, and the user is unsure whether manually deleting them is safe or whether they affect subsequent publisher/subscriber sessions.

**74. Buffer invalidation in Fast-DDS SHM transport**  
2024-10-28 · GitHub eProsima/Fast-DDS (Discussion #5363) · [link](https://github.com/eProsima/Fast-DDS/discussions/5363)  

A user configuring SHM transport with maxmessagesize equal to segment_size (128 MB) receives repeated 'Buffer is being invalidated, segment_size may be insufficient' warnings during operation despite observing no data loss. Maintainers could not fully resolve why invalidation warnings appear without causing loss in the reported config. The discussion closed without a definitive root cause, leaving users uncertain whether the warning is safe to ignore.

**75. [21830] Minimum fragment size in SHM of Fast-DDS**  
2024-10-02 · GitHub eProsima/Fast-DDS (Discussion #5291) · [link](https://github.com/eProsima/Fast-DDS/discussions/5291)  

A user attempting to set maxmessagesize to 123 bytes for an SHM transport while transferring 1 GB of data encountered RTPSWriter errors. Even when raised to the documented minimum of 512 bytes errors persisted. The root cause identified by maintainers is that segment_size must be larger than the total data payload; a segment_size close to or smaller than the data causes the write operation to overwrite the buffer during a single send, causing silent data loss. Fixes were tracked in PRs #5464 and #5473.

**76. Occasional SHM Transport issue**  
2024-09-19 · GitHub eProsima/Fast-DDS#5244 · [link](https://github.com/eProsima/Fast-DDS/issues/5244)  

On a Yocto Kirkstone / ROS 2 Humble embedded platform, publisher and subscriber nodes on the same device intermittently fail to exchange data via SHM while cross-device communication over UDP works fine. EDP matching completes (the publisher logs a reader) but the subscriber receives nothing. The issue is non-deterministic and appears under heavy discovery load, suggesting a timing or port-state race in the SHM transport layer.

**77. ROS2 nodes can't communicate between Docker containers**  
2024-08-01 · GitHub eProsima/Fast-DDS#5396 · [link](https://github.com/eProsima/Fast-DDS/issues/5396)  

ROS 2 Humble nodes in separate Docker containers (Fast-DDS 2.6.8, --net=host --ipc=host) can list each other's topics but data never flows when subscribing. The topic listing succeeds via PDP multicast, but the unicast EDP + SHM data path breaks because SHM locators from one container are not reachable by the other container's Fast-DDS instance.

**78. Shared memory performance loss compared with standalone iceoryx**  
2024-08-01 · GitHub eclipse-cyclonedds/cyclonedds#2155 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/2155)  

Benchmarks using the APEX performance_test package show that CycloneDDS with SHM enabled (iceoryx PSMX) has higher latency than using standalone iceoryx directly, and latency increases as the subscriber count grows (tested 1–32 subs at 30 Hz, 8 MB payload). The loaned-samples path and plain SHM path both degrade with scale, suggesting routing overhead or lock contention in the CycloneDDS-iceoryx integration that does not exist in native iceoryx.

**79. FastDDS SHM permission issue**  
2024-07-25 · GitHub eProsima/Fast-DDS (Discussion #5104) · [link](https://github.com/eProsima/Fast-DDS/discussions/5104)  

Fast-DDS 2.9.1 creates fixed-name /dev/shm files for PDP discovery (e.g. fastrtps_port7400 for domain 0). When the process umask is 002, these files are created with permissions 664 (group-owner only), so DomainParticipants running under a different GID get 'Permission denied' and PDP multicast discovery fails. The issue is structural: SHM file ownership is baked in at creation time with no API to override permissions.

**80. fastdds 2.14.0: shared memory mode cannot communicate after restarting the process**  
2024-07-11 · GitHub eProsima/Fast-DDS#5053 · [link](https://github.com/eProsima/Fast-DDS/issues/5053)  

On Windows 10 with Fast-DDS 2.14.0, if a subscriber console window is frozen (mouse click hangs it) and then closed, restarting the subscriber shows successful topic matching but no data is ever received. The issue is specific to SHM transport; switching to UDP makes restart recovery work correctly. Stale SHM port state from the frozen/abnormally closed process is not cleaned up, leaving the restarted subscriber unable to attach.

**81. Shared-memory with containers**  
2024-06-26 · GitHub ros2/rmw_zenoh#213 · [link](https://github.com/ros2/rmw_zenoh/issues/213)  

With rmw_zenoh, publisher and subscriber in separate rootless Podman containers (configured with --ipc=host and /dev/shm bind-mounted) do not use shared memory transport; instead hundreds of megabytes flow over the loopback interface. The heuristic Zenoh uses to detect 'same physical host' for SHM eligibility is confused by containerization, defaulting to network transport even when IPC namespace sharing is correctly configured.

**82. SHM transport delay is too large**  
2024-04-28 · GitHub eProsima/Fast-DDS#4739 · [link](https://github.com/eProsima/Fast-DDS/issues/4739)  

With Fast-DDS v2.11.2 and default SHM+UDP transport, a process with two DataWriters (one writing 1 MB messages, one writing 10-byte messages) shows unexpectedly high latency for the small messages when the large write is submitted first. The SHM transport serialises all writes through a shared port and a single large payload blocks delivery of subsequent small payloads, making SHM latency unpredictable for mixed-size workloads common in ROS 2 sensor pipelines.

**83. [22368] Zerocopy data reader loses data**  
2024-04-22 · GitHub eProsima/Fast-DDS#4715 · [link](https://github.com/eProsima/Fast-DDS/issues/4715)  

A Fast-DDS 2.11.2 zero-copy (data-sharing) reader dropped ~90 out of ~190 messages despite the history pool having 530 available slots and the pool not being full. The writer addresses and reader addresses partially differed, suggesting the shared-memory loan mechanism was not correctly tracking ownership. The bug was reproducible only with the data-sharing transport path, not with UDP.

**84. Messages get dropped when larger than 0.5 MB - using shared memory - QoS is Best_effort**  
2024-01-22 · GitHub ros2/rmw_fastrtps#739 · [link](https://github.com/ros2/rmw_fastrtps/issues/739)  

Transferring Ouster lidar point cloud data (~10 MB per scan) via ros2bag, ros2 topic echo, and topic hz showed dropped messages that worsened with higher publish frequency or larger payload size. SHM was active and showed only ~1.5 MB used out of 64 MB available, ruling out segment exhaustion. Increasing socket buffers had no effect because the path used SHM, not UDP, yet drops continued.

**85. Got 'unknown source entity, ignore.' when only use shared memory**  
2023-11-17 · GitHub eclipse-cyclonedds/cyclonedds#1881 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/1881)  

When running CycloneDDS with shared memory (iceoryx) across separate Docker containers with a shared /dev and /tmp volume, the listener logs 'unknown source entity, ignore.' and never receives data from the talker. The entities are visible in RouDi introspection but CycloneDDS cannot match the publisher and subscriber across container boundaries even with correct network interface config.

**86. Segfault in MultiThreadedExecutor using rmw_fastrtps_cpp**  
2023-10-25 · GitHub ros2/rmw_fastrtps#728 · [link](https://github.com/ros2/rmw_fastrtps/issues/728)  

On ROS 2 Humble (aarch64/Ubuntu 22.04 in Docker), a node using `rclcpp::executors::MultiThreadedExecutor` intermittently segfaults inside `rmw_fastrtps_shared_cpp::__rmw_wait()` at rmw_wait.cpp:220 with address-not-mapped error. The crash appears non-deterministic and has been confirmed on multiple deployments using MoveIt Studio.

**87. Composable node randomly not loading when shared memory is used**  
2023-10-06 · GitHub ros2/rmw_cyclonedds#472 · [link](https://github.com/ros2/rmw_cyclonedds/issues/472)  

With Iceoryx-backed shared memory enabled in rmw_cyclonedds_cpp on ROS 2 Humble, composable nodes randomly fail to load into component containers. In a system with 90 expected composable nodes, the actual count unpredictably drops below 181 total nodes. Disabling shared memory makes loading deterministic again. The failure is non-reproducible at a fixed point, indicating a race condition in SHM resource registration at startup.

**88. Possible memory leak in Cyclone/Iceoryx subscriber history queue**  
2023-10-02 · GitHub ros2/rmw_cyclonedds#471 · [link](https://github.com/ros2/rmw_cyclonedds/issues/471)  

Using CycloneDDS with iceoryx SHM transport, a subscriber with history depth 250 slowly accumulates unreleased memory chunks when multiple publishers start and stop. The error 'TOO_MANY_CHUNKS_HELD_IN_PARALLEL – could not take sample' appears after initial silent drops. Spawning and closing unrelated nodes causes the held chunk count to creep upward, pointing to a reference-counting or cleanup bug in the iceoryx pool.

**89. Windows 10 shared memory related crash/hang**  
2023-09-21 · GitHub ros2/rmw_fastrtps#713 · [link](https://github.com/ros2/rmw_fastrtps/issues/713)  

headlee reported that ROS 2 Humble nodes on Windows 10 crash or hang on launch when FastRTPS shared memory transport is enabled. The failure occurs because a shared memory directory conflict ('C:\ProgramData\eprosima\fastrtps_interprocess already exists') prevents participant creation, cascading to publisher finalization failure and process termination with exit code 3221226505. Running 'fastdds.bat shm clean' provides a temporary workaround but the condition recurs.

**90. Running discovery process as root breaks existing processes running as normal user when SHM is used**  
2023-08-17 · GitHub eProsima/Fast-DDS#3475 · [link](https://github.com/eProsima/Fast-DDS/issues/3475)  

On ROS 2 Humble, running any Fast-DDS process as root (e.g. `ros2 topic hz /foo` in a privileged Docker container) corrupts the shared-memory discovery state for pre-existing non-root processes. After the root process runs, other non-root participants see 'topic does not appear to be published yet' and never rediscover each other.

**91. FastDDS nodes cannot communicate if another user ran a node in the past**  
2023-05-24 · GitHub eProsima/Fast-DDS#3535 · [link](https://github.com/eProsima/Fast-DDS/issues/3535)  

On ROS 2 Humble, if Fast-DDS SHM port files in /dev/shm were previously created by a process running as user 1000, new processes running as user 1001 are blocked from writing to those files (permission denied), causing discovery to appear to succeed but no actual data to be exchanged. The stale files persist across process restarts and are not cleaned up, requiring 'fastdds shm clean' or manual deletion. This scenario is common in CI/CD where containers run as varying UIDs.

**92. FastDDS doesn't report an error if it fails to setup reliable communication through SHM transport**  
2023-05-24 · GitHub eProsima/Fast-DDS#3536 · [link](https://github.com/eProsima/Fast-DDS/issues/3536)  

When Fast-DDS (Humble, 2.6.4) fails to establish the SHM communication channel (e.g. due to a permission error from a prior different-user run), it silently falls back or degrades without logging any error visible to the application. Nodes discover each other and appear healthy but messages between certain pairs are never delivered. This makes the failure mode invisible: a DevOps engineer tested as one user, deployed as a service user, and lost significant debugging time before finding the silent SHM failure.

**93. Memory leak slowly and chunk size increase with multiple nodes**  
2023-04-20 · GitHub ros2/rmw_cyclonedds#452 · [link](https://github.com/ros2/rmw_cyclonedds/issues/452)  

Lannister-Xiaolin reported progressive memory growth when multiple nodes share a CycloneDDS zero-copy image pipeline: one publisher with two downstream point-cloud subscriber nodes. Under high CPU load the chunk and memory usage jumps and never recovers after the load subsides, indicating a resource recycling failure in CycloneDDS's memory pool. The issue was observed on Ubuntu 20.04 ARM64 with ROS 2 Galactic.

**94. Can't use shared memory transport with initialPeersList or discovery server**  
2023-03-15 · GitHub ros2/rmw_fastrtps#676 · [link](https://github.com/ros2/rmw_fastrtps/issues/676)  

When configuring Fast-DDS in ROS 2 (Humble, Docker) to use SHM for intra-host communication and UDP for inter-host communication, enabling either initialPeersList or a discovery server breaks SHM transport entirely: participants discover each other but no data flows over SHM. The root cause is that if any transport other than SHM is present for metatraffic, Fast-DDS routes discovery over UDP and the SHM locators are not exchanged, leaving intra-host SHM non-functional.

**95. Transient_Local is not work with Shared memory**  
2023-02-28 · GitHub eclipse-cyclonedds/cyclonedds#1584 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/1584)  

When CycloneDDS shared memory (iceoryx) is enabled with Transient_Local durability, a subscriber that starts after the publisher has already sent data never receives any historical samples. This is a silent QoS incompatibility: SHM transport in CycloneDDS does not implement Transient_Local semantics, so the durability setting is silently ignored and late-joiners lose all previously published data.

**96. SHM error Function open_port_internal**  
2023-01-18 · GitHub ros2/rmw_fastrtps#660 · [link](https://github.com/ros2/rmw_fastrtps/issues/660)  

Two Docker containers attempting SHM-based Fast-DDS communication fail with '[RTPS_TRANSPORT_SHM Error] Failed init_port fastrtps_port14162: open_and_lock_file failed' when one container runs FastDDS 2.1.2 and the other 2.1.1. A minor version mismatch between containers prevents the shared memory port from initializing, blocking all inter-container communication without a clear diagnostic error.

**97. Cyclone DDS with iox-roudi missing messages**  
2022-10-24 · ROS Answers (answers.ros.org / robotics.stackexchange.com) · [link](https://answers.ros.org/question/408463/cyclone-dds-with-iox-roudi-missing-messages/)  

After enabling Cyclone DDS shared-memory transport via iox-roudi for /cmd_vel, messages stop flowing after a few seconds. Iceoryx introspection reveals chunk usage spiking from 16 to 273 chunks at the moment messages stop, suggesting roudi's shared memory queue saturates even with only one subscriber. The issue does not occur without shared memory configuration, exposing a backpressure/queue-limit bug in the Cyclone+iceoryx SHM path.

**98. Shared memory not working with transient_local durability**  
2022-08-10 · GitHub ros2/rmw_cyclonedds#401 · [link](https://github.com/ros2/rmw_cyclonedds/issues/401)  

afrixs found that enabling CycloneDDS shared memory transport silently breaks transient_local durability: late-joining subscribers to /tf_static receive no messages despite the publisher having sent them. The identical setup works correctly with shared memory disabled. This violates the documented behavior that transient_local preserves messages for late joiners, and the DDS documentation claims the combination is supported.

**99. [Shared Memory] Subscriber won't reconnect after crash under specific circumstances**  
2022-07-06 · GitHub eProsima/Fast-DDS#2811 · [link](https://github.com/eProsima/Fast-DDS/issues/2811)  

After a Fast-DDS subscriber using SHM transport is killed abnormally (via GDB quit or SIGKILL), restarting the subscriber results in successful EDP matching being reported but zero messages received. The publisher logs show it still has the old reader matched. Stale SHM port state left by the crashed process prevents the new subscriber from attaching to the publisher's shared memory segment; running 'fastdds shm clean' before restart works around the issue.

**100. shared memory with iceoryx appears to use non-compatible topics**  
2022-07-05 · GitHub eclipse-cyclonedds/cyclonedds#1326 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/1326)  

When iceoryx SHM is enabled in a ROS 2 Nav2 deployment, the /parameter_events topic (which is variable-length and thus not SHM-compatible) gets routed through iceoryx anyway, causing repeated 'TOO_MANY_CHUNKS_HELD_IN_PARALLEL' errors and eventually a fatal 'could not create service: failed to create reader' crash that aborts the entire navigation stack. The history depth mismatch between the subscriber's KeepAll(100) request and iceoryx's chunk capacity of 1 is the proximate cause.

**101. Shared mem partition fills up after many runs: how to garbage collect?**  
2022-06-29 · GitHub eProsima/Fast-DDS#2790 · [link](https://github.com/eProsima/Fast-DDS/issues/2790)  

After restarting a Fast-DDS participant many times (e.g. in a containerized robot bringup loop using ipc:host), /dev/shm fills up with fastrtps_* segment files even when /dev/shm is sized at 32 GB. Once the partition is full, new participants fail with 'Failed to create segment: No such file or directory' and SHM transport is disabled. Fast-DDS has no automatic garbage collection; the only recovery is 'fastdds shm clean' or a machine reboot.

**102. ros2 galactic fast dds shared memory — how to use it?**  
2022-03-30 · ROS Answers (answers.ros.org / robotics.stackexchange.com) · [link](https://answers.ros.org/question/398211/)  

A user publishing 15 MB images experiences 80 ms end-to-end latency and tries to enable Fast DDS SHM transport via XML configuration. The attempt triggers 'eprosima::fastcdr::exception::NotEnoughtMemoryException' due to undersized segment configuration. The user also questions whether ROS2 is actually using Fast DDS SHM by default, revealing that SHM is not transparently enabled and requires non-obvious XML tuning.

**103. how to change the shared memory segment size of fastrtps**  
2022-01-24 · GitHub ros2/rmw_fastrtps#576 · [link](https://github.com/ros2/rmw_fastrtps/issues/576)  

ROS 2 Foxy enabled SHM transport by default, but the default shared-memory segment size was only 512 KB. Any message larger than ~510 KB caused sudden large drop rates with no error logged. The user found no documented API to increase the segment size through rmw_fastrtps configuration.

**104. Crashing with iceoryx sharedmemory enabled**  
2021-11-10 · GitHub eclipse-cyclonedds/cyclonedds#1026 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/1026)  

On NVIDIA Jetson Xavier running ROS 2 Galactic, CycloneDDS with iceoryx SHM crashes after several hours of operation with 'MEPOO__MEMPOOL_GETCHUNK_POOL_IS_RUNNING_OUT_OF_CHUNKS' — all 32768 mempool chunks are exhausted. A secondary failure mode is the iceoryx_mgmt shared memory segment not existing at startup, causing an assertion failure and core dump. The default mempool configuration is undersized for multi-node robotics workloads and there is no built-in backpressure or chunk reclaim mechanism.

**105. Repeated restart of the program will lead to memory leakage**  
2021-10-22 · GitHub eProsima/Fast-DDS#2287 · [link](https://github.com/eProsima/Fast-DDS/issues/2287)  

On ARM-based Linux, repeatedly launching and kill -9-ing a Fast-DDS DDSHelloWorldExample application every 2 seconds causes a continuous /dev/shm memory leak of 20–30 MB every 2 minutes. Because SIGKILL prevents the library's shutdown path from running, shared memory segments accumulate without bound in /dev/shm and are never garbage collected, eventually exhausting the tmpfs partition.

**106. Zero-copy DDS Performance Comparison for RMW Providers in ROS 2**  
2021-10-15 · ROS Discourse · [link](https://discourse.openrobotics.org/t/zero-copy-dds-performance-comparison-for-rmw-providers-in-ros-2/22696)  

Benchmark of Fast DDS data-sharing vs. CycloneDDS+Iceoryx zero-copy on Linux. Cyclone+Iceoryx crashed with memory pool failures at >1,000 msg/s shortly after launch; increasing Iceoryx chunks to 50,000 did not resolve it; the Iceoryx daemon crashed multiple times during testing, creating a single point of failure that disabled all zero-copy delivery. Maximum throughput for Cyclone+Iceoryx was very poor relative to Fast DDS, especially at large data sizes.

**107. Excessive CPU usage on Mac using Shared Memory [12607]**  
2021-09-28 · GitHub eProsima/Fast-DDS#2237 · [link](https://github.com/eProsima/Fast-DDS/issues/2237)  

The micrortps_agent exhibits 300–700 % CPU utilization on macOS when FastDDS shared-memory transport is enabled, versus ~40 % with UDP. Time profiler analysis traces the overhead to Boost's busy-wait polling loop on macOS. Even the 40 % baseline was considered excessive for the modest ~200 Hz message rate involved.

**108. fastdds shm clean crashes**  
2021-07-15 · GitHub eProsima/Fast-DDS#2071 · [link](https://github.com/eProsima/Fast-DDS/issues/2071)  

The 'fastdds shm clean' CLI tool that is supposed to remove zombie shared-memory segments and ports crashes with AttributeError ('Clean' object has no attribute 'segments_in_use' / 'ports_in_use') on CentOS 7 with Python 2.7, 3.4, and 3.6. The tool fails before it can clean anything, leaving stale /dev/shm files from crashed processes permanently. A manual attribute rename workaround was found but no official fix was merged at the time.

**109. Publisher and subscriber nodes don't hear each other if one of them runs in a Docker container**  
2021-02-12 · GitHub eProsima/Fast-DDS#1755 · [link](https://github.com/eProsima/Fast-DDS/issues/1755)  

A ROS 2 Foxy listener node on the host and a talker inside a Docker container (Ubuntu 18.04 base, --net host) never exchange data when both use Fast-DDS, but work immediately if either node switches to CycloneDDS. The failure is caused by Fast-DDS detecting the container and host as the same machine (via net-interface hash) and selecting SHM transport, while the IPC namespace is not shared (--ipc=host not set), so SHM communication silently fails.

**110. ROS2 Foxy Communication across multiple local users**  
2021-02-10 · GitHub eProsima/Fast-DDS#1750 · [link](https://github.com/eProsima/Fast-DDS/issues/1750)  

On a single ROS 2 Foxy machine with a network interface active, nodes launched under different Unix user accounts cannot see each other's topics even on localhost, while nodes launched as the same user can. When the network cable is removed topics become visible across users. The root cause is that Fast-DDS SHM port files in /dev/shm are owned by one user's UID/GID and are not world-writable, blocking cross-user access.

**111. 2021 Eclipse Cyclone DDS ROS Middleware Evaluation Report with iceoryx and Zenoh**  
2021-01-01 · OSRF TSC-RMW-Reports · [link](https://osrf.github.io/TSC-RMW-Reports/humble/eclipse-cyclonedds-report.html)  

OSRF formal evaluation found Cyclone DDS + iceoryx was the only implementation to pass the LoanedMessage/shared memory API tests — all other middleware implementations failed to receive any messages through the shared memory path. Tests with 4–8 MB messages at 500 Hz fail under default OS network buffer settings on both Windows 10 and Ubuntu. Service discovery reliability issues reported: 'Several users have reported instances where services never appear, or they never get responses.'

**112. Unable to use SharedMemTransport as the sole transport layer**  
2020-12-27 · GitHub eProsima/Fast-DDS#1665 · [link](https://github.com/eProsima/Fast-DDS/issues/1665)  

When SharedMemTransport is configured as the only transport layer (useBuiltinTransports=false, no UDP fallback) the DomainParticipant creation call hangs indefinitely on Windows 10 with Fast-DDS v2.1.0. The official HelloWorldExampleSharedMem example itself exhibits the hang as soon as the UDP transport line is removed. This forces users to always include a UDP transport alongside SHM, defeating zero-copy-only deployments.

**113. [RTPS_TRANSPORT_SHM Error] Failed to create segment [10057]**  
2020-11-27 · GitHub eProsima/Fast-DDS#1606 · [link](https://github.com/eProsima/Fast-DDS/issues/1606)  

On an embedded device running ROS 2 Foxy with Fast-DDS, the SHM transport fails to create the shared memory segment at startup with 'No such file or directory', logging RTPS_TRANSPORT_SHM Error and RTPS_MSG_OUT Error. The process falls back to UDP but the SHM transport is effectively unavailable, likely due to constrained flash/RAM resources on the device. There is no clean error path to distinguish resource exhaustion from a misconfiguration.

**114. [21899] SHM-only transport (host id) is dependent from the real IPv4 addresses**  
n/a · GitHub eProsima/Fast-DDS#5313 · [link](https://github.com/eProsima/Fast-DDS/issues/5313)  

When one participant starts before a network interface gets an IPv4 address and a second starts after, each computes a different `host_id` (one uses a fallback, one uses the IPv4 hash). Fast-DDS then considers them to be on different hosts, filters all SHM locators, and discovery never completes — even though both processes are on the same machine.

---

## QoS Silent No-Match (incompatible QoS → no data, no error)

*36 items*

**115. QoS compatibility is too strict, should be more user-friendly and flexible**  
2024-05-10 · GitHub ros2/ros2#1562 · [link](https://github.com/ros2/ros2/issues/1562)  

fujitatomoya raised that ROS 2's strict DDS QoS matching silently prevents endpoint creation: if a publisher offers 'volatile' durability but a subscriber requests 'transient_local', no connection is established and no messages flow, with no clear user-facing error. The user must predetermine QoS compatibility before node initialization or face silent endpoint creation failure. The issue proposes a fallback to the publisher's offered QoS level with a warning rather than outright rejection.

**116. Help with QoS Compatibility Issue in ZED ROS2 Wrapper and Custom Node**  
2024-01-31 · Stereolabs community forum · [link](https://community.stereolabs.com/t/help-with-qos-compatibility-issue-in-zed-ros2-wrapper-and-custom-node/4483)  

A user's custom node filtering and republishing ZED 2i PointCloud2 data received 'New publisher discovered on topic /filtered_pointcloud, offering incompatible QoS. No messages will be sent to it. Last incompatible policy: DURABILITY_QOS_POLICY'. The ZED wrapper and the custom republisher node used different DURABILITY settings, silently blocking all data flow to RViz.

**117. [WARN] New subscription discovered on topic '/scan', requesting incompatible QoS**  
2024-01-12 · GitHub ros2/rviz#1122 · [link](https://github.com/ros2/rviz/issues/1122)  

A user with SensorDataQoS (BEST_EFFORT, VOLATILE) on both publisher and RViz received the warning 'New subscription discovered on topic /scan, requesting incompatible QoS. No messages will be sent to it.' even though ros2 topic info showed matching profiles. The warning appeared only on Humble, not Foxy or Galactic, suggesting a version-specific regression in QoS negotiation rather than a real policy mismatch.

**118. Lifespan not working with transient_local subscriber**  
2023-09-13 · GitHub ros2/rmw_cyclonedds#473 · [link](https://github.com/ros2/rmw_cyclonedds/issues/473)  

Setting a 2-second Lifespan QoS on a transient_local subscriber with rmw_cyclonedds on ROS 2 Iron has no effect: the callback fires for all historical messages regardless of age. ros2 topic info --verbose shows 'Lifespan: Infinite' instead of the configured value, confirming the QoS is silently dropped at the DDS layer rather than being applied.

**119. Intra-process Type adaptation failing silently if types mismatch**  
2023-08-29 · GitHub ros2/rclcpp#2291 · [link](https://github.com/ros2/rclcpp/issues/2291)  

When intra-process communication is enabled and a publisher uses a custom adapted type while a subscriber uses the raw ROS message type (or vice versa), communication silently fails with no warning, error, or exception. The problem is undetectable without external instrumentation, and the rclcpp type adaptation layer lacks validation to catch incompatible type pairs before establishing communication. Requested fix is to surface diagnostics via ros2 doctor or exception.

**120. Last incompatible policy: RELIABILITY_QOS_POLICY - Ros2 bag problem**  
2023-06-22 · ROS Answers (archived) · [link](https://answers.ros.org/question/416740/last-incompatible-policy-reliability_qos_policy-ros2-bag-problem/)  

A user replaying a rosbag of 79,228 IMU messages configured their Python consumer with RELIABLE reliability to avoid message loss, but received 'New publisher discovered on topic sensors/imu, offering incompatible QoS. No messages will be received from it.' because the rosbag player published with BEST_EFFORT. Switching back to BEST_EFFORT caused ~5% message loss. There was no configuration path that delivered all messages reliably.

**121. QoS Best Effort Reliability Issue**  
2023-06-01 · GitHub ros2/ros2#1434 · [link](https://github.com/ros2/ros2/issues/1434)  

A BEST_EFFORT subscriber should drop messages independently and never throttle the publisher, but in practice a slow best-effort subscriber (e.g., RViz over degraded WiFi) causes the publisher to slow down identically to a RELIABLE subscriber. DDS flow control negotiates with all subscribers uniformly, defeating the intent of best-effort for non-critical monitoring and stalling the robot's internal communication.

**122. ROS2 QoS Reliability Issue**  
2023-05-30 · ROS Discourse (discourse.openrobotics.org) · [link](https://discourse.openrobotics.org/t/ros2-qos-reliability-issue/31700)  

A slow remote subscriber (e.g., RViz over degraded WiFi) connected to a publisher causes the publisher to throttle its output rate for ALL subscribers — including fast local shared-memory subscribers — defeating the purpose of per-subscriber QoS policies. A nav2 node receiving camera frames at 30Hz slows down to match the WiFi subscriber's bandwidth limit, regardless of whether internal subscribers use 'best_effort' policy. This bug is reproduced across FastDDS and CycloneDDS on ROS 2 Foxy, Galactic, Humble, and Rolling, and was escalated as GitHub issue ros2/ros2#1434.

**123. ROS2 Galactic support — QoS RELIABILITY_QOS_POLICY incompatibility on /ouster/points**  
2023-05-21 · GitHub ouster-lidar/ouster-ros discussion #135 · [link](https://github.com/ouster-lidar/ouster-ros/discussions/135)  

When visualising Ouster LiDAR point clouds in RViz2 under ROS2 Galactic, users received 'New publisher discovered on topic /ouster/points, offering incompatible QoS. No messages will be sent to it. Last incompatible policy: RELIABILITY_QOS_POLICY'. The driver published with BEST_EFFORT; RViz2 defaulted to RELIABLE. Approximately 90 degrees of the LiDAR scan was also missing due to an unrelated azimuth window misconfiguration.

**124. Humble ros2 bag play qos_override doesn't change topic durability**  
2023-01-31 · GitHub ros2/rosbag2#1237 · [link](https://github.com/ros2/rosbag2/issues/1237)  

A user attempted to replay a bag with /tf_static (originally TRANSIENT_LOCAL) using a qos_override YAML file specifying transient_local durability, but 'ros2 topic info --verbose' confirmed the played-back topic remained VOLATILE, causing rviz2 to reject all transforms with a durability QoS incompatibility warning. The override mechanism silently ignored the durability setting.

**125. Can ROS2 services still be expected to be 'flakey'?**  
2023-01-26 · ROS Answers (answers.ros.org / robotics.stackexchange.com) · [link](https://answers.ros.org/question/411860/can-ros2-services-still-be-expected-to-be-flakey/)  

Service clients intermittently time out despite the service server logging that it successfully processed and sent the response. The problem affects both CycloneDDS and FastDDS RMW implementations. The server confirms 'response sent' but the client callback is never triggered, indicating a DDS reliable-delivery or service-request/response matching defect that makes ROS2 services unreliable at low occurrence rates.

**126. Ros2 Humble/Rolling - rviz2 wont accept my tf_static message from bag**  
2022-10-15 · GitHub ros2/rviz#916 · [link](https://github.com/ros2/rviz/issues/916)  

When playing back a bag recording that contains /tf_static messages, rviz2 in Humble/Rolling issues the warning 'New subscription discovered on topic /tf_static, requesting incompatible QoS. No messages will be sent to it. Last incompatible policy: DURABILITY_QOS_POLICY'. The bag player publishes with VOLATILE while /tf_static requires TRANSIENT_LOCAL; the same bag worked fine in Galactic due to less-strict enforcement.

**127. camera_calibration: QoS incompatibility ROS2 galactic**  
2022-08-29 · GitHub ros-perception/image_pipeline#770 · [link](https://github.com/ros-perception/image_pipeline/issues/770)  

The camera_calibration node subscribed to image topics with RELIABLE reliability, but the camera driver published with BEST_EFFORT (SensorDataQoS). The result was 'New publisher discovered on topic image, offering incompatible QoS. No messages will be received from it. Last incompatible policy: RELIABILITY'. The calibration tool silently received nothing and gave no actionable feedback to the user.

**128. Content Filtering Topics Support**  
2022-06-24 · GitHub ros2/rmw_cyclonedds#397 · [link](https://github.com/ros2/rmw_cyclonedds/issues/397)  

CycloneDDS's rmw layer does not implement Content-Filtered Topics (CFT), a standard DDS feature present in other RMW implementations. An empty stub was added to pass the interface but CFT predicates are silently ignored, meaning subscribers receive all samples regardless of configured filters — a silent QoS non-implementation without error.

**129. Race condition in discovery of QoS prevents recording topics published with non-default QoS**  
2022-03-03 · GitHub ros2/rosbag2#967 · [link](https://github.com/ros2/rosbag2/issues/967)  

When ros2 bag record detects a topic via a DDS DataReader before the matching DataWriter is discovered, it subscribes with default QoS instead of the publisher's actual QoS. This causes loud incompatibility warnings for reliability mismatches and silent data loss when durability differs (e.g., TRANSIENT_LOCAL publisher vs. VOLATILE subscriber). The offered_qos_profiles metadata field in the bag can also be corrupted to an empty string.

**130. ROS2 Galactic and Cyclone DDS with AWS machines - hidden topics and nodes**  
2022-03-01 · GitHub eclipse-cyclonedds/cyclonedds#1170 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/1170)  

Running ROS 2 Galactic across two AWS EC2 instances with CycloneDDS results in intermittent failure where 'ros2 topic list' and 'ros2 node list' return nothing even though nodes are running. Simultaneously, Nav2 logs 'New subscription discovered on topic /scan, requesting incompatible QoS. No messages will be sent to it. Last incompatible policy: RELIABILITY_QOS_POLICY'. The QoS incompatibility silently suppresses delivery with no user-visible warning beyond the log line.

**131. static_transform_publisher with transient_local doesn't work**  
2021-12-16 · GitHub ros2/geometry2#487 · [link](https://github.com/ros2/geometry2/issues/487)  

In ROS2 Galactic with CycloneDDS on loopback networking, echoing /tf_static with matching TRANSIENT_LOCAL QoS caused the program to hang indefinitely waiting for messages. The published transform was never received despite correct QoS settings, pointing to an interaction between CycloneDDS loopback-only mode and transient_local publisher/subscriber matching.

**132. User setting for QoS depth is ignored while using Client/Service**  
2021-09-28 · GitHub ros2/rmw_cyclonedds#339 · [link](https://github.com/ros2/rmw_cyclonedds/issues/339)  

rmw_cyclonedds hard-codes DDS_HISTORY_KEEP_ALL with DDS_LENGTH_UNLIMITED for service request/reply topics, silently overriding any user-configured QoS depth. This means service history queues can grow without bound regardless of depth settings, and user-specified queue limits have no effect on ROS services.

**133. Confusing warning message about incompatible QoS**  
2021-07-30 · GitHub ros2/rosbag2#830 · [link](https://github.com/ros2/rosbag2/issues/830)  

When recording from a BEST_EFFORT publisher with 'ros2 bag record --all', rosbag2 emits a misleading warning claiming the bag is requesting RELIABLE while the publisher offers BEST_EFFORT. In practice messages are still recorded, but the warning alarms users into thinking data is being lost. The disconnect between the warning severity and actual behaviour causes widespread confusion about whether the system is working correctly.

**134. Overriding QoS Profiles Doesn't Work**  
2021-06-30 · GitHub ros2/rosbag2#802 · [link](https://github.com/ros2/rosbag2/issues/802)  

The rosbag2 QoS override feature (YAML file passed via --qos-profile-overrides-path) failed to apply the specified BEST_EFFORT reliability setting; rosbag2 continued subscribing as RELIABLE and generated warnings about no messages being recorded from the BEST_EFFORT publisher. Users who relied on the documented workaround found it non-functional, resulting in silently empty recordings.

**135. Incorrect QoS compatibility warning**  
2021-05-20 · GitHub ros2/rosbag2#772 · [link](https://github.com/ros2/rosbag2/issues/772)  

A use-after-free bug in recorder.cpp caused rosbag2 to read stale memory for the actual_qos of a subscription, producing a spurious 'offering VOLATILE durability but rosbag2 subscribed requesting TRANSIENT_LOCAL' warning. The timing-dependent bug appeared more often in release builds and caused users to believe data was being dropped when it was actually being recorded normally.

**136. ros2 bag play does not work with rmw_connextdds**  
2021-04-23 · GitHub ros2/rosbag2#756 · [link](https://github.com/ros2/rosbag2/issues/756)  

When using rmw_connextdds as the RMW in Galactic/Rolling, ros2 bag play fails to create any publishers because the DDS writer creation is rejected with 'inconsistent QoS policy: period' and 'inconsistent QoS policy: deadline' errors. The root cause is that Connext DDS rejects the QoS values persisted from the bag metadata as internally inconsistent, silently blocking all playback.

**137. ROS2 Foxy incompatible QoS policies — no warnings emitted**  
2021-02-07 · ROS Answers (answers.ros.org / robotics.stackexchange.com) · [link](https://answers.ros.org/question/371257/ros2-foxy-incompatible-qos-policies-no-warnings/)  

A publisher configured with best-effort QoS and a subscriber with reliable QoS silently receive no messages with no diagnostic output. When the user tried to register an incompatible-QoS event callback via event_callbacks.incompatible_qos_callback, rmw_fastrtps threw UnsupportedEventTypeException, meaning the feature was not supported, leaving developers with no way to detect the mismatch programmatically in Foxy.

**138. Ros2 Galactic Nav2 — /scan offering incompatible QoS**  
2021-02-07 · ROS Answers (answers.ros.org / robotics.stackexchange.com) — redirected to robotics.stackexchange.com/questions/100796 · [link](https://answers.ros.org/question/392676/ros2-galactic-nav2-scan-offering-incompatible-qos/)  

Nav2's costmap subscribed to /scan with reliable reliability while the lidar driver published using sensor_data QoS (best-effort). This QoS mismatch caused a '[WARN] New subscription discovered on topic /scan, requesting incompatible QoS — No messages will be sent to it' warning and no sensor data reached the costmap. Fix is to change RViz and subscriber nodes to best-effort reliability, matching the sensor driver's QoS profile.

**139. QoS mismatch between gazebo_ros_camera and gazebo_ros_video**  
2021-02-01 · GitHub ros-simulation/gazebo_ros_pkgs#1218 · [link](https://github.com/ros-simulation/gazebo_ros_pkgs/issues/1218)  

The Gazebo camera plugin published with BEST_EFFORT reliability while the video plugin subscribed expecting RELIABLE, preventing message transmission entirely. Camera output was invisible in both RViz and rqt, and texture rendering in the simulator failed. The fix required aligning the video plugin's subscriber QoS with the camera plugin's publisher QoS.

**140. QoS compatibility failing silently (ROS2)**  
2021-01-22 · GitHub RobotWebTools/rosbridge_suite#551 · [link](https://github.com/RobotWebTools/rosbridge_suite/issues/551)  

rosbridge_server subscribes to topics using RELIABLE reliability by default. When a publisher advertises a topic with BEST_EFFORT reliability, the DDS QoS negotiation silently fails: no error is logged, the subscription appears created, but no data ever arrives in the callback. The root cause is that a BEST_EFFORT publisher cannot satisfy a RELIABLE subscriber's request, and rosbridge exposes no mechanism to configure subscriber QoS.

**141. ros2 bag play uses incorrect qos for static transform publisher**  
2020-12-15 · ROS Answers (archived) · [link](https://answers.ros.org/question/367760/)  

Recording and replaying /tf_static with 'ros2 bag record -a' produced a DURABILITY_QOS_POLICY incompatibility warning in RViz at playback time. The bag recorder did not preserve the TRANSIENT_LOCAL durability that static_transform_publisher requires; the bag player replayed as VOLATILE, making the transform listener reject all messages and causing the robot model to vanish in RViz.

**142. ros2 transient_local durability (late joiners policy) does not work when using ros2 topic echo**  
2020-06-01 · ROS Answers (answers.ros.org / robotics.stackexchange.com) · [link](https://answers.ros.org/question/353755/ros2-transient_local-durability-late-joiners-policy-does-not-work-when-using-ros2-topic-echo/)  

A publisher with TRANSIENT_LOCAL durability that emits one message at startup cannot be read by a late-joining 'ros2 topic echo --qos-durability=transient_local'. The retained message is accessible by a custom subscriber node joining late, but the CLI tool fails to retrieve it. The bug stems from ros2cli not properly configuring its transient_local subscriber to match the DDS DataWriter's QoS before attaching.

**143. ros2topic test_echo_pub can't handle new QoS mismatch warnings**  
2020-04-20 · GitHub ros2/ros2cli#492 · [link](https://github.com/ros2/ros2cli/issues/492)  

When incompatible QoS settings are detected, Cyclone DDS emits a warning (RELIABILITY_QOS_POLICY mismatch) but Fast-RTPS did not support this feature at the time, making warning behaviour inconsistent across vendors. A test that expected no output broke because Cyclone started emitting 'Incompatible QoS Policy detected' messages, revealing that the feature was middleware-dependent.

**144. rti DDS 5.3.1 and ROS2 eloquent, topic not received**  
2020-04-01 · RTI Community Forum · [link](https://community.rti.com/forum-topic/rti-dds-531-and-ros2-eloquent-topic-not-received)  

A developer bridging a native RTI DDS 5.3.1 application into ROS2 Eloquent via a routing-service plugin found that the ROS2 subscriber callback was never triggered even though the RTI Admin Console showed a successful topic match. The problem combined a transport locator incompatibility (510-mode flag mismatch) with a USER_QOS_PROFILES.xml error about inconsistent history.depth vs resource_limits.max_samples, silently blocking all message flow.

**145. Using the correct QoS profile to subscribe to an existing topic**  
2020-03-18 · GitHub ros2/ros2_documentation#569 · [link](https://github.com/ros2/ros2_documentation/issues/569)  

Users subscribing to a TurtleBot3 lidar topic with default QoS parameters received no data and no error. The sensor driver publishes with the sensor_data profile (BEST_EFFORT), while rclcpp/rclpy default to RELIABLE, causing a silent QoS no-match. There were no code examples in the official tutorials showing non-default profiles, making the failure extremely hard to diagnose.

**146. Adding support for ON_REQUESTED_INCOMPATIBLE_QOS and ON_OFFERED_INCOMPATIBLE_QOS event callbacks in ROS2**  
2019-11-16 · GitHub ros2/ros2#822 · [link](https://github.com/ros2/ros2/issues/822)  

Publishers and subscribers can be created with incompatible QoS policies without any user notification, silently resulting in zero data delivery. For example, a subscription with a smaller deadline than the publisher's will never receive messages. The DDS spec provides on_requested_incompatible_qos and on_offered_incompatible_qos callbacks for exactly this case, but ROS 2 had not surfaced them, making these mismatches extremely difficult to debug.

**147. [ROS2] QOS profile options and subscribing problem**  
2019-01-17 · GitHub ros-visualization/rqt#187 · [link](https://github.com/ros-visualization/rqt/issues/187)  

rqt_image_view and other rqt GUI plugins subscribed with fixed default QoS, making it impossible to view topics from publishers using non-default profiles such as sensor_data (BEST_EFFORT). The mismatch caused silently empty displays. The issue requested per-plugin QoS configuration support so that GUI tools could match whatever profile a publisher was using.

**148. ROS2: topic subscribed but not received**  
2018-09-05 · ROS Answers (answers.ros.org / robotics.stackexchange.com) · [link](https://answers.ros.org/question/302451/ros2-topic-subscribed-but-not-received/)  

A PointCloud2 device driver publishing at 30 Hz with KEEP_LAST history depth=1 and best-effort reliability works initially in RViz but messages disappear when the 3D view is manipulated. 'ros2 topic echo' receives nothing despite the publisher confirming active subscribers. Root cause is insufficient history buffer depth (depth=1 causes dropped messages under any processing load); setting depth > 1 stabilizes delivery.

**149. Error when publishing more than 5000 messages and using 'keep all' history**  
2016-11-29 · GitHub ros2/rmw_fastrtps#68 · [link](https://github.com/ros2/rmw_fastrtps/issues/68)  

FastDDS aborts after publishing exactly 5,000 messages with KEEP_ALL QoS and no active subscriber, printing 'Maximum number of allowed reserved caches reached' and 'failed to publish message: cannot publish data'. The cache pool is exhausted because messages are buffered indefinitely without a subscriber to consume them. Switching to KEEP_LAST avoids the crash but changes delivery semantics.

**150. FastRTPS doesn't send message if publisher and subscriber don't have the same qos_profile**  
2016-06-30 · GitHub ros2/rclcpp#232 · [link](https://github.com/ros2/rclcpp/issues/232)  

One of the earliest reported ROS2 QoS silent-failure cases: a publisher using rmw_qos_profile_sensor_data and a subscriber using rmw_qos_profile_default exchanged no messages at all under Fast-RTPS. The reporter was uncertain whether this was expected DDS behaviour or a bug, demonstrating that the incompatibility-means-no-data contract was not yet documented.

---

## Multicast / WiFi (blocked, floods, dropouts)

*34 items*

**151. Configuring Fast DDS Discovery Server to use TCP to bypass firewall UDP flood protection**  
2026-02-04 · GitHub turtlebot/turtlebot4#673 · [link](https://github.com/turtlebot/turtlebot4/issues/673)  

yjcrocks reported that launching TurtleBot4 navigation on a university WiFi network caused the university firewall to detect FastDDS's UDP traffic burst as a DDoS attack: all UDP connections were dropped, WAN internet connectivity was lost for the robot, the host machine, and all other devices on the router. Only same-subnet TCP connections survived. The user needed to reconfigure the entire Fast-DDS Discovery Server from UDP to TCP to avoid triggering flood-protection systems.

**152. Configuring Cyclone DDS for Wifi + Ethernet connection on an Enterprise Network (for ROS2)**  
2025-11-05 · GitHub gist robosam2003 · [link](https://gist.github.com/robosam2003/d5fcfaf4bfd55298d86c1460cb7fc60c)  

On university and corporate networks, DDS multicast is blocked by security policy, making default ROS 2 node discovery completely non-functional. The gist documents that FastDDS unicast mode is also unreliable and recommends switching to Cyclone DDS with a static peer list. A 2025 comment adds that omitting the loopback address from the peer list causes additional intra-device discovery flakiness.

**153. Optimizing ROS 2 Communication for Wireless Robotic Systems**  
2025-08-15 · arXiv 2508.11366 · [link](https://arxiv.org/html/2508.11366v1)  

The first systematic network-layer analysis of ROS 2 DDS over wireless links identifies three root causes: excessive IP fragmentation (44 fragments per 64KB RTPS message at 1500B MTU), inefficient retransmission timing (default 0.33Hz triggering 160Mb/s instantaneous bursts for 65KB messages at 30Hz), and congestive buffer bursts (400-sample history cache generating 400×231KB burst after a 10-second link outage, collapsing throughput to 5Hz). At 1% packet error rate with 330KB payloads, message delivery collapses to 9%.

**154. Announcing ROS 2 Easy Mode**  
2025-02-11 · ROS Discourse (discourse.openrobotics.org) · [link](https://discourse.openrobotics.org/t/announcing-ros-2-easy-mode/41974)  

eProsima announces ROS 2 Easy Mode for FastDDS to address four documented failure scenarios: lossy WiFi networks, network congestion, large data (video/LiDAR), and discovery failures in environments without multicast support. A December 2025 follow-up reports that even with Easy Mode, sending data across multiple network interfaces causes the same packet to be sent to all interface addresses simultaneously — triplicating traffic with three interfaces — because Easy Mode ignores ROS_STATIC_PEERS and ROS_AUTOMATIC_DISCOVERY_RANGE variables.

**155. Forget Packet Loss & Discovery Hassles: Meet ROS 2 Easy Mode!**  
2025-02-10 · eProsima news / Vulcanexus · [link](https://www.eprosima.com/news/forget-packet-loss-forget-discovery-hassles-meet-ros-2-easymode)  

eProsima's announcement of ROS 2 Easy Mode explicitly acknowledges that default DDS Simple Discovery Protocol is unsuitable for many deployments: packet loss in Wi-Fi environments, multicast discovery traffic causing network load, unreliable discovery in scenarios where multicast is blocked, and poor congestion control under heavy load. Easy Mode replaces multicast with TCP-based server-to-server discovery to address these systemic DDS weaknesses.

**156. Bad network performance with multiple robots in ROS2**  
2024-06-14 · GitHub eProsima/Fast-DDS discussion#4948 · [link](https://github.com/eProsima/Fast-DDS/discussions/4948)  

Adding a second robot to a ROS 2 fleet using FastDDS causes the network to grind to a halt 90% of the time, with messages arriving 10-15 seconds late or not at all. With hundreds of topics and ~10 nodes per robot, default DDS multicast discovery floods the network, occasionally crashing routers. The user's workaround of separate DOMAIN_IDs bridged with domain_bridge felt like a hack; the recommended fix is switching to Discovery Server to reduce multicast traffic.

**157. FastDDS in low-bandwidth ROS2 environment**  
2024-05-22 · GitHub eProsima/Fast-DDS#4832 · [link](https://github.com/eProsima/Fast-DDS/discussions/4832)  

A mobile robot using FastDDS over weak WiFi signal (<-80dBm) experiences the main publish thread blocking for multiple seconds because SYNC publish mode waits for slow WLAN uploads. Even switching to ASYNC mode the send queue fills when RViz2 subscribes remotely alongside local nodes, preventing any topic from being transmitted. The user needed per-interface transport descriptors to isolate robot-internal from WLAN-facing communication.

**158. DDS Tuning for ROS 2**  
2024-05-17 · breq.dev blog · [link](https://breq.dev/2024/05/17/dds)  

A robotics team found that switching UI views generated excessive multicast UDP discovery traffic that saturated a bandwidth-constrained radio link. Large DDS packets exceeding 1500-byte MTU were fragmented; under congestion, fragment drops made large messages unrecoverable. Point cloud streaming 'completely broke down' in ROS2 compared to ROS1 due to UDP lacking TCP's congestion control. Switching from FastDDS to CycloneDDS 'largely fixed discovery traffic problems'; eventually migrated to Zenoh for a further 97% discovery traffic reduction.

**159. FR: Add support for ROS2**  
2024-05-02 · GitHub tailscale/tailscale#11972 · [link](https://github.com/tailscale/tailscale/issues/11972)  

ROS 2 DDS discovery relies on UDP multicast for the initial peer-discovery phase (SPDP), which Tailscale's virtual network does not support. This makes ROS 2 completely incompatible with Tailscale VPN for multi-robot fleet deployments over the internet. The reporter warns 'No robotics companies will use Tailscale for deployments' without multicast support, pointing to DDS's architectural dependence on LAN-multicast as the blocker.

**160. ROS2 communication stopped between nodes on same machine when network is down**  
2024-02-29 · GitHub ros2/rmw_cyclonedds#483 · [link](https://github.com/ros2/rmw_cyclonedds/issues/483)  

Two ROS 2 Iron nodes in Docker containers on the same host (--network=host) stop communicating the moment the host's WiFi disconnects. CycloneDDS logs ddsi_upd_conn_write errors referencing the WiFi IP rather than falling back to loopback. All traffic was local; the external NIC going down should not affect intra-host communication.

**161. Losing Connection after sending a few ros2 commands**  
2023-11-29 · GitHub iRobotEducation/create3_docs#466 · [link](https://github.com/iRobotEducation/create3_docs/discussions/466)  

After ~5 ROS 2 action commands (rotate_angle), the Create3 becomes unresponsive with 'Waiting for an action server to become available', though topic list still shows topics present. The failure is reproducible and traced to multicast-based ROS 2 discovery being broken by the home router's network configuration; disabling multicast and configuring unicast XML profile resolves the issue entirely.

**162. Multiple Turtlebot4 setup with Discovery Server**  
2023-08-07 · GitHub turtlebot/turtlebot4#244 · [link](https://github.com/turtlebot/turtlebot4/issues/244)  

With three TurtleBot4 robots on WiFi using Simple Discovery, the Create3 units repeatedly lose WiFi connectivity and exhibit poor motion performance. The problem is severe enough that DDS multicast storms are suspected, requiring each robot to be given separate DOMAIN_IDs and configuring Discovery Server to reduce the multicast load—a complex configuration task that the user found prohibitively difficult.

**163. Investigation into alternative middleware solutions**  
2023-07-31 · ROS Discourse discourse.openrobotics.org/t/investigation-into-alternative-middleware-solutions/32642 · [link](https://discourse.openrobotics.org/t/investigation-into-alternative-middleware-solutions/32642)  

Open Robotics (clalancette) opened formal investigation into replacing DDS, citing: multicast UDP disabled in many office/corporate environments causing discovery failures; WiFi image streaming blocks the publisher thread because DDS lacks asynchronous publishing; large image frames drop nearly every packet on WiFi without TCP retransmission; and configuration complexity overwhelming educators and hobbyists.

**164. ROS2 with multiple machines doesn't work properly with point clouds**  
2023-06-20 · ROS Answers archive · [link](https://answers.ros.org/question/416682/ros2-with-multiple-machines-doesnt-work-properly-with-point-clouds/)  

When a development PC subscribes to LiDAR point cloud data from an embedded system over WiFi, the high data volume overwhelms the wireless link and causes backpressure that drops upstream /lidar_packets before they reach the assembler node—resulting in incomplete point clouds rather than simple subscriber-side frame drops. The internal robot pipeline is corrupted by a remote WiFi subscriber's inability to keep up.

**165. Frequent discovery DATABASE_ERROR during WiFi brownouts and roaming**  
2023-05-28 · GitHub eProsima/Fast-DDS#3544 · [link](https://github.com/eProsima/Fast-DDS/issues/3544)  

On a fleet of autonomous mobile robots using WiFi 6 with AP roaming, the Fast-DDS Discovery Server intermittently emits `[DISCOVERY_DATABASE Error] Reader/Writer has no associated participant` messages after a robot switches APs. After these errors, certain publisher/subscriber pairs never re-match and data flow does not recover without restarting the ROS nodes.

**166. Losing DDS connection with remote hosts**  
2023-04-10 · GitHub eclipse-cyclonedds/cyclonedds#1648 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/1648)  

In a ROS 2 Galactic robot+control-station system communicating over WiFi, when the wireless link temporarily drops (due to range or interference), CycloneDDS loses the remote peer and never reconnects even after the link is restored. Wireshark shows METATRAFFIC is still being sent but user data packets stop flowing. The only fix is manually restarting the affected node, indicating CycloneDDS does not re-establish peer state after WiFi-level disconnection.

**167. Driving Autoware with Zenoh**  
2023-02-02 · Autoware Foundation blog · [link](https://autoware.org/driving-autoware-with-zenoh/)  

Autoware team documents three concrete DDS limitations: (1) ROS 2/DDS cannot cross the internet without dedicated per-vendor support; (2) DDS requires multicast capability, making it incompatible with 4G/5G networks used in field deployments; (3) DDS multicast discovery packets become a problem on WiFi by generating too many packets. The zenoh-bridge-dds is proposed to convert DDS packets to Zenoh for cross-network remote control.

**168. ROS2 WIFI Multicast Multi Robot and IGMP Snooping**  
2022-11-25 · ROS Discourse (discourse.openrobotics.org) · [link](https://discourse.openrobotics.org/t/ros2-wifi-multicast-multi-robot-and-igmp-snooping/28516)  

User operating three drones over WiFi with ROS 2 Foxy and FastDDS experiences intermittent complete signal dropout lasting up to one second, causing drone crashes; the problem is reproducible by switching any drone from ethernet to WiFi. Root cause is multicast discovery traffic being degraded by the router's inability to handle multicast packets at scale — the author notes 'multicast over wifi has the potential to fragment packets and cause a mini DDos'. Enabling IGMP snooping on an Asus GT-AX6000 nearly eliminates dropouts; the Fast DDS Discovery Server is recommended as the structural fix.

**169. ros2 wireless alternatives**  
2022-10-20 · ROS Answers archive · [link](https://answers.ros.org/question/408296/ros2-wireless-alternatives/)  

A student cannot deploy ROS 2 on a college campus because the campus WiFi blocks multicast as a security measure, making all DDS-based discovery non-functional. Even setting up a private wireless access point failed because the laptop refused to connect without internet access. The post explicitly asks for alternatives to DDS for wireless ROS 2 deployment.

**170. Configuration Cyclone to allow multi subnet/interface communications**  
2022-09-08 · GitHub eclipse-cyclonedds/cyclonedds#1422 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/1422)  

A ROS 2 Galactic system where three PCs communicate over Ethernet (192.168.2.0/24) cannot be reached by a fourth PC connected via WiFi (192.168.1.0/24) even though all machines are on both networks. Adding all peer IP addresses via the Discovery/Peers config has no effect. The root cause is that Cyclone only uses one interface at a time, so machines connected only via the non-primary interface are invisible regardless of peer config.

**171. ROS2 Multicast works but nodes can't communicate or see each other over multiple machines**  
2022-08-24 · ROS Answers (answers.ros.org / robotics.stackexchange.com) · [link](https://answers.ros.org/question/405451/ros2-multicast-works-but-nodes-cant-communicate-or-see-each-other-over-multiple-machines/)  

On ROS2 Humble across Ubuntu 22.04 and Raspberry Pi 4 over WiFi, dedicated multicast test tools confirm bidirectional multicast, same ROS_DOMAIN_ID is set, and firewall allows multicast — yet talker/listener nodes only discover peers on the same machine, not cross-machine. The paradoxical resolution (static IP + UFW subnet rule fixes ROS2 comms but breaks the multicast tool) suggests DDS unicast fallback or port-specific routing issues distinct from raw multicast reachability.

**172. Unhandled Terminate w/ Services Over Wifi Network**  
2022-04-05 · GitHub ros2/ros2#1253 · [link](https://github.com/ros2/ros2/issues/1253)  

When issuing 25 concurrent service calls from a WiFi-connected machine, Fast-RTPS intermittently crashes with 'client will not receive response', while CycloneDDS is unaffected. The failure does not reproduce on the same host or over wired connections, pointing to WiFi latency and packet loss exposing a reliability gap in Fast-RTPS service handling.

**173. DDS not working while ethernet has a device plugged in**  
2021-10-18 · GitHub ros2/ros2#1203 · [link](https://github.com/ros2/ros2/issues/1203)  

When a Velodyne LiDAR is connected via Ethernet to a Raspberry Pi that is also on WiFi, plugging in the sensor after node startup causes all topics and nodes to become invisible to other computers on the same ROS domain. DDS multicast discovery appears to break when a new network interface (Ethernet) becomes active mid-run, as it interferes with the multicast routing already in use on the WiFi interface. Nodes must be launched before the sensor is plugged in as a workaround.

**174. Indy Autonomous Challenge (IAC): Experiences from the Trenches**  
2021-09-28 · Zenoh blog (ZettaScale / Eclipse) · [link](https://zenoh.io/blog/2021-09-28-iac-experiences-from-the-trenches/)  

Race-team field report: excessive DDS discovery packets overwhelmed the wireless infrastructure at the Indy Autonomous Challenge. TCP-based transport caused congestion-control delays that postponed fresh telemetry data. High-frequency ROS 2 topics saturated wireless bandwidth. The team had to restrict DDS to localhost, switch transports to UDP, and add traffic-pacing mechanisms as workarounds.

**175. ROS2 foxy Eclipse Cyclone DDS communication between two Docker Ubuntu containers**  
2021-08-01 · GitHub eclipse-cyclonedds/cyclonedds#680 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/680)  

Fixing Docker-to-Windows host CycloneDDS communication (by disabling multicast) breaks container-to-container communication. Re-enabling multicast restores container-to-container comms but breaks Windows connectivity again. There is no single configuration that handles both simultaneously; users are forced to choose one or the other, making multi-endpoint Docker deployments impossible without a discovery server.

**176. Multiple Interface: multicast will be all disabled while some interfaces don't support multicast**  
2021-05-31 · GitHub eclipse-cyclonedds/cyclonedds#819 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/819)  

When a user configures CycloneDDS to use both loopback (lo) and a wireless interface (wlp0s20f3) together, if the loopback is flagged as not multicast-capable, CycloneDDS disables multicast for ALL listed interfaces. The log shows: 'selected interface lo is not multicast-capable: disabling multicast', breaking cross-machine discovery even though the wireless interface supports multicast. Workarounds require either enabling multicast on loopback, using AssumeMulticastCapable, or switching to unicast peer config.

**177. Bad performance of ROS2 via Wifi**  
2020-09-01 · ROS Answers archive · [link](https://answers.ros.org/question/362065/)  

When a WiFi-connected workstation subscribes to laser scanner data from a ROS2 node, the local publisher's rate drops from 30Hz to 20Hz and the remote receives only 2–3Hz in bursts every 5 seconds. The issue persists across both FastRTPS and Cyclone DDS with Best_Effort/Volatile QoS set. Adding a remote WiFi subscriber degrades the local publisher even though communication is internal, likely due to the hidden-node WiFi problem under ~500kB/s load.

**178. Edge Robotics with Eclipse zenoh and ROS 2**  
2020-09-01 · Eclipse Foundation Newsletter · [link](https://www.eclipse.org/community/eclipse_newsletter/2020/september/1.php)  

Eclipse Foundation article identifies DDS's core wireless limitations for edge robotics: multicast on Wi-Fi networks is problematic; DDS was not designed for wireless or to scale across WAN; discovery traffic and reliability protocol are poorly suited to distributed edge environments. Zenoh is positioned as the wire-most-efficient protocol enabling internet-scale robot data streaming that DDS cannot support.

**179. ROS2 Default Behavior (Wifi)**  
2020-03-31 · ROS Discourse (discourse.openrobotics.org) · [link](https://discourse.openrobotics.org/t/ros2-default-behavior-wifi/13460)  

Poster argues that ROS 2's multicast-based DDS discovery is fundamentally inadequate for corporate WiFi out of the box, with standard demonstrations failing on typical office networks when transmitting HD video and point clouds from mobile robots. Users must manually configure XML files, learn about discovery servers, or switch to unicast — a steep barrier for roboticists without DDS networking expertise. The post establishes that 'working out of the box in corporate wifi in steady state setting reasonably well is the minimum viable product' for ROS 2, implying the default DDS configuration fails this bar entirely.

**180. RasperryPi3 Wireless communication problem**  
2019-09-10 · GitHub ros2/rmw_fastrtps#315 · [link](https://github.com/ros2/rmw_fastrtps/issues/315)  

A Raspberry Pi 3 connected via WiFi fails to receive messages from an Ethernet-connected x86 workstation 90–99% of the time with Fast-RTPS; occasionally after 3–5 minutes a few messages trickle through intermittently. The exact cause (WiFi jitter, slow RPi CPU missing DDS timeouts, or multicast limitations) was not definitively resolved but clearly linked to the wireless path.

**181. Issues disabling multicast for WIFI use case [12505]**  
n/a · GitHub eProsima/Fast-DDS#2201 · [link](https://github.com/eProsima/Fast-DDS/issues/2201)  

A user running Fast-DDS between a Jetson AGX (AP) and a laptop (client) over WiFi tried to disable multicast by configuring metatrafficUnicastLocatorList and initialPeersList, but received no RTPS or UDP traffic at all. Despite ARP showing the devices can reach each other, the locator configuration was insufficient to make unicast-only Simple Discovery work over WiFi.

**182. Multicast-only discovery/subscription**  
n/a · GitHub eProsima/Fast-DDS#2934 · [link](https://github.com/eProsima/Fast-DDS/issues/2934)  

Two hosts in different subnets connected through a router (an iRobot Create3 with a closed-source OS) where unicast routing is impossible. Even with `smcroute` forwarding multicast in both directions, Fast-DDS performs unicast during EDP, making cross-subnet discovery fail. `ros2 topic list` never shows topics from the remote subnet.

**183. Subscriber fails to receive message sometimes in different machines, however a discovery node will instantly make the subscriber work again [6119]**  
n/a · GitHub eProsima/Fast-DDS#581 · [link](https://github.com/eProsima/Fast-DDS/issues/581)  

A subscriber on WiFi occasionally receives nothing from a publisher on a second machine in the same subnet. Intermittently, the subscriber starts silently missing all messages after startup. Introducing a third unrelated participant (a 'monitor' node) instantly causes the subscriber to start receiving data again, suggesting EDP state is incompletely propagated during Simple Discovery's initial announcement window.

**184. Messages not received after participant rediscovery**  
n/a · GitHub eProsima/Fast-DDS#514 · [link](https://github.com/eProsima/Fast-DDS/issues/514)  

A WiFi reader/writer pair using BEST_EFFORT QoS at 6 kB/s intermittently loses the writer participant due to interference. Even after the writer is re-discovered (DISCOVERED_PARTICIPANT fires), the reader never resumes receiving data. Introducing an unrelated third participant with only a discovery listener inexplicably restores data flow, indicating a latent EDP re-exchange failure after reconnect.

---

## Cross-Vendor / Inter-Distro Interop

*32 items*

**185. Cross-RMW service interoperability: ListParameters request from rmw_cyclonedds_cpp client can be misdeserialized and trigger OOM on rmw_fastrtps_cpp server**  
2026-04-02 · GitHub ros2/rmw_cyclonedds#577 · [link](https://github.com/ros2/rmw_cyclonedds/issues/577)  

A rmw_cyclonedds client calling ListParameters on a rmw_fastrtps server causes the server to allocate tens of gigabytes and be OOM-killed. CycloneDDS injects a 16-byte GUID+sequence-number header before the CDR payload in service requests; FastRTPS interprets this header as the ROS data, reads a bogus sequence length, and tries to allocate a corresponding enormous buffer. The incompatibility is acknowledged in CycloneDDS source code comments as 'probably incompatible'.

**186. XTypes compliance mismatch — RTI Connext default CDR encoding non-compliant with OMG XTypes 1.3**  
2025-06-12 · RTI Community knowledge base · [link](https://community.rti.com/kb/xtypes-compliance-mismatch)  

By default, RTI Connext data serialization is not fully compliant with the OMG Extended CDR encoding as specified in XTypes 1.3. This prevents interoperability with strictly-compliant DDS vendors. When the XTypes compliance mask differs between the Core Libraries and Code Generator settings, Connext emits 'Inconsistent XTypes Compliance options for this type' and refuses to register the type, requiring developers to manually align the -xTypesComplianceMask parameter across both components.

**187. Incompatibility between distributions**  
2025-05-14 · ROS Discourse discourse.openrobotics.org/t/incompatability-between-distributions/43747 · [link](https://discourse.openrobotics.org/t/incompatability-between-distributions/43747)  

Conpleks raised that Humble and Jazzy nodes cannot reliably communicate despite identical message definitions: ROS 2 Iron introduced Type Description Distribution changing serialization byte-format with additional metadata, GID storage changed from 8 to 16 bytes, and FastDDS changed from version 2.6.x to 2.14.x between the two distros. Core maintainer Katherine Scott stated explicitly: 'The recommendation is that users NOT MIX NODES BETWEEN ROS DISTROS. Will it work? Maybe? There are zero tests or guarantees.'

**188. DDS design vs reality — services and actions incompatible across vendors despite pub/sub working**  
2024-09-18 · ROS Discourse (openrobotics) · [link](https://discourse.openrobotics.org/t/difference-between-dds-design-and-reality/39669)  

Users building heterogeneous fleets where certification requirements forced different DDS vendor choices found that ROS2 services and actions silently fail across vendor boundaries even though pub/sub topics work. The DDS standard promises broad interoperability but the RMW service/action mapping is vendor-specific, meaning a CycloneDDS client cannot call a Connext service. Projects locked to a vendor by C++ standard or certification requirements cannot integrate with components using other vendors.

**189. CycloneDDS and eProsima Micro XRCE-DDS Communication in ROS2**  
2024-08-05 · GitHub eclipse-cyclonedds/cyclonedds#2062 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/2062)  

In a Nav2 + MicroROS setup (STM32 microcontroller), critical stop-command twist messages occasionally fail to reach the microcontroller, causing the robot to continue rotating uncontrollably. The developer suspects the cross-vendor incompatibility between CycloneDDS (Nav2 side) and eProsima Micro XRCE-DDS (microcontroller side) as the root cause of dropped messages in the 10 ms publish loop.

**190. Rosbag2 does not work at all with CycloneDDS on Jazzy**  
2024-05-06 · GitHub ros2/rosbag2#1638 · [link](https://github.com/ros2/rosbag2/issues/1638)  

On ROS 2 Jazzy with rmw_cyclonedds_cpp, rosbag2 records bags where all timestamps are set to January 1, 1970 (epoch 0), making playback produce no output. The issue affects both Jazzy and Rolling. Switching to a different RMW resolves the problem, indicating a CycloneDDS-specific timestamp initialization or propagation bug in the Jazzy release.

**191. Ubuntu Host (Humble) and Docker (Iron) communication issue — Failed to parse type hash for topic + xmlrpc.client.ResponseError**  
2024-04-11 · GitHub ros2/rmw_cyclonedds#487 · [link](https://github.com/ros2/rmw_cyclonedds/issues/487)  

Running ROS 2 Humble on a host and ROS 2 Iron in a Docker container with the same rmw_cyclonedds_cpp causes repeated 'Failed to parse type hash for topic ros_discovery_info' warnings and ResponseError from rclpy.type_hash.TypeHash when echoing topics. Topics appear in listings but transmit no data, and the failure is version-specific — a cross-ROS-release type-hash format incompatibility in CycloneDDS's participant metadata.

**192. OpenDDS and Eprosima FastDDS (ROS2)**  
2023-11-30 · GitHub OpenDDS/OpenDDS Wiki · [link](https://github.com/OpenDDS/OpenDDS/wiki/OpenDDS-and-Eprosima-FastDDS-(ROS2))  

OpenDDS and FastDDS interoperability in ROS2 requires explicit configuration: users must set DataWriterQos data representation to XCDR_DATA_REPRESENTATION (XCDRv1), use RtpsDiscovery and rtps_udp transport, and align topic key definitions. Without this, failures occur due to incompatible data formats during fragmentation and mismatched topic key interpretations. FastDDS defaults to XCDR2 representations that OpenDDS does not accept out of the box.

**193. ROS2 Inter-distro Communication Issue: Eloquent to Humble with Fast DDS**  
2023-09-14 · ROS Discourse (discourse.openrobotics.org) · [link](https://discourse.openrobotics.org/t/ros2-inter-distro-communication-issue-eloquent-to-humble-with-fast-dds/33550)  

User running ROS 2 Humble on a PC cannot receive any topics published by a robot running ROS 2 Eloquent, despite both using eProsima Fast DDS on the same domain ID and same network. A maintainer confirms that inter-distro communication is officially unsupported in ROS 2 and represents a known architectural limitation rather than a configuration error. This leaves teams upgrading fleets incrementally with no supported discovery/communication path between old and new ROS 2 distributions.

**194. can not deserialize TypeInformation, which serialized in XCDR**  
2023-02-16 · GitHub eclipse-cyclonedds/cyclonedds#1572 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/1572)  

CycloneDDS announces support for both XCDR and XCDR2 in SEDP discovery messages, but its deser_type_information function hardcodes CDR version 2 only. When a remote DDS implementation (e.g., FastDDS or RTI Connext) sends TypeInformation serialized in XCDR v1, CycloneDDS silently fails to parse it despite having advertised compatibility, breaking typed discovery and type-hash-based matching for cross-vendor deployments.

**195. Can ROS2 services still be expected to be 'flakey'?**  
2023-01-26 · ROS Answers archive · [link](https://answers.ros.org/question/411860/)  

ROS2 Humble service clients intermittently fail to receive responses when multiple clients call the same service concurrently. After several thousand successful exchanges the system 'stutters' and clients begin timing out with 'Failed to call service. Stopping execution now'. Both FastDDS and CycloneDDS exhibit the failure, showing it is a cross-vendor RMW-layer problem in the request/reply pattern rather than a single-vendor bug.

**196. Cross-Distro/Vendor communication in ROS 2**  
2023-01-25 · GitHub ros2/ros2_documentation#3288 · [link](https://github.com/ros2/ros2_documentation/issues/3288)  

ROS 2 officially does not support cross-distro or cross-vendor communication, yet this is undocumented in official docs, causing widespread user confusion. The issue was raised after the Middleware Working Group meeting by Tomoya Fujita. Community forums show recurring failures where users unknowingly run Eloquent on a robot and Humble on a PC using the same FastDDS domain and find no data flowing, being told inter-distro communication is 'an anti-pattern'.

**197. ROS2 to OpenDDS Communication**  
2022-11-08 · ROS Answers (answers.ros.org / robotics.stackexchange.com) · [link](https://answers.ros.org/question/409158/)  

A developer bridges ROS2 turtlesim (FastDDS) with a custom OpenDDS RTPS implementation. Topics appear in monitoring tools, DDS data types are recognized, and QoS settings match ROS2 specs — but actual message payloads are never received. The failure persists despite correct topic naming (rt/ prefix) and QoS alignment, pointing to a deeper serialization or type-encoding incompatibility between FastDDS and OpenDDS at the RTPS layer.

**198. DDS to ROS2 messages**  
2022-09-16 · ROS Answers archive · [link](https://answers.ros.org/question/406529/dds-to-ros2-messages/)  

Native DDS applications fail to receive messages from ROS2 nodes due to multiple interop barriers: ROS2 prefixes all topic names with 'rt/', auto-generated IDL has extra nesting that does not match vendor IDL tools, default QoS values conflict, and CycloneDDS rejects @appendable type annotations requiring XCDR2. Each vendor (CycloneDDS, Connext, FastDDS) has different QoS defaults, so there is no single IDL or QoS configuration that works across all three simultaneously.

**199. ROS2 interoperability with native DDS**  
2022-08-12 · ROS Answers (answers.ros.org / robotics.stackexchange.com) · [link](https://answers.ros.org/question/404995/ros2-interoperability-with-native-dds/)  

Attempting to connect a native CycloneDDS application to ROS2 Galactic requires the rt/ topic prefix, CDR-encoded message types, and compatible QoS. Even after matching these, messages are not received because CycloneDDS rejects the @appendable IDL annotation (used by ROS2 for XTypes) since it requires XCDR2; using @final instead resolves the issue. This undocumented IDL-annotation incompatibility blocks native DDS ↔ ROS2 interop without trial-and-error.

**200. ROS2 interoperability with native DDS**  
2022-08-12 · ROS Answers archive · [link](https://answers.ros.org/question/404995/)  

A user trying to bridge a native CycloneDDS application to ROS2 Galactic finds that even after aligning topic names (rt/ prefix) and QoS, ros2 topic echo shows no messages despite the publisher being detected. Root cause: CycloneDDS refuses to decode data serialized with the @appendable XTypes extensibility annotation because it does not support XCDR2 data representation. Switching to @final and adjusting QoS resolved the issue.

**201. Experiences with ROS 2 on our robots – Hamburg Bit-Bots**  
2022-07-25 · Hamburg Bit-Bots Blog · [link](https://bit-bots.de/en/2022/07/experiences-with-ros-2-on-our-robots/)  

A robot team reports two distinct cross-vendor issues from real ROS2 deployment: FastDDS intermittently fails to list nodes/topics after node restarts, and C++ callbacks stop arriving entirely on ROS2 Rolling with FastDDS (severe enough to block nav2's Rolling release). Switching to CycloneDDS fixed the callback problem but introduced a new failure — CycloneDDS locks to a single network interface at startup and breaks when that interface becomes unavailable (e.g., cable unplugged), requiring a bridge workaround.

**202. QoS issue when subscribing to OpenDDS publisher**  
2022-06-01 · GitHub eclipse-cyclonedds/cyclonedds#1544 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/1544)  

When an OpenDDS publisher uses KEEP_ALL history QoS, CycloneDDS mis-decodes the OpenDDS QoS wire encoding and interprets it as KEEP_LAST, leading to failed communication between OpenDDS publisher and CycloneDDS subscriber. The cyclonedds ls introspection tool confirms the wrong QoS is decoded. This is a cross-vendor serialization/parsing incompatibility in the RTPS discovery QoS inline parameters.

**203. Publisher GID in message info of taken message does not match GID of source publisher**  
2022-03-05 · GitHub ros2/rmw_cyclonedds#377 · [link](https://github.com/ros2/rmw_cyclonedds/issues/377)  

In rmw_cyclonedds_cpp, the publisher Global ID (GID) recorded inside a taken message's info struct does not match the GID returned when the publisher was created. This makes it impossible to correctly correlate received messages back to their source publisher. The bug is CycloneDDS-specific — rmw_fastrtps_cpp returns matching GIDs — and undermines any cross-vendor publisher tracking or message provenance logic.

**204. Running ROS2 demo nodes with CycloneDDS and FastRTPS**  
2022-01-18 · GitHub eclipse-cyclonedds/cyclonedds#1110 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/1110)  

In a mixed-RMW setup with FastRTPS as publisher and CycloneDDS as subscriber, the CycloneDDS listener misses the first 4 messages when the listener starts before the talker. This was filed to investigate broader cross-DDS discovery timing issues: the listener is running and matched, but initial samples are dropped due to a discovery race condition between the two RMW layers.

**205. Message compatibility to native DDS**  
2021-08-31 · GitHub ros2/rmw_connextdds#64 · [link](https://github.com/ros2/rmw_connextdds/issues/64)  

ROS2 Galactic removed the vendor-specific IDL files from /opt/ros/galactic/share/PKG/msg/dds_connext that previously enabled RTI Admin Console and native Connext DDS apps to match ROS2 message types. After the removal, the generated IDL files no longer match the wire format seen in the RTI toolchain, breaking native Connext applications' interoperability with ROS2. Users must fall back to Foxy message types or legacy compatibility flags.

**206. ros2 and rti-connext-dds keyed mismatch**  
2021-06-27 · ROS Answers (answers.ros.org / robotics.stackexchange.com) · [link](https://answers.ros.org/question/381215/ros2-and-rti-connext-dds-keyed-mismatch/)  

A native RTI DDS 6.0.1 routing service plugin attempts to bridge DDS topics to ROS2 Foxy, but the ROS2 subscriber callback is never invoked. RTI Admin Console reports a 'keyed mismatch' in its Match Analyses even though the developer defined key fields in the IDL. The incompatibility between how ROS2's rmw_connextdds treats keyed types vs. native RTI type definitions silently prevents all data flow.

**207. Unable to communicate between a FastDDS application and ROS2 using CycloneDDS when ROS_LOCALHOST_ONLY is set to 1**  
2021-06-07 · GitHub ros2/rmw_cyclonedds#318 · [link](https://github.com/ros2/rmw_cyclonedds/issues/318)  

A standalone FastDDS application configured for localhost-only UDP transport cannot communicate with ROS 2 nodes using rmw_cyclonedds_cpp when ROS_LOCALHOST_ONLY=1. The identical setup works with rmw_fastrtps_cpp. CycloneDDS's localhost-only implementation creates an RTPS environment incompatible with FastDDS's loopback transport whitelist configuration.

**208. Numerous test failures for Nav2 when using rmw_connextdds**  
2021-03-31 · GitHub ros2/rmw_connextdds#21 · [link](https://github.com/ros2/rmw_connextdds/issues/21)  

Running Nav2 with rmw_connextdds as the RMW produced three distinct failure classes absent with fastrtps/cyclonedds: nanoseconds-overflow warnings in the DDS timestamp layer, a type-support identifier mismatch ('rosidl_typesupport_cpp is not supported by this library'), and a concurrency error ('cannot delete and wait on the same object') during object lifecycle teardown. The failures were specific to the Connext RMW and did not reproduce with other DDS implementations.

**209. QoS profiles recorded from Fast-DDS are unplayable in Cyclone (and vice-versa)**  
2021-02-15 · GitHub ros2/rosbag2#656 · [link](https://github.com/ros2/rosbag2/issues/656)  

RMW implementations encode infinity-duration QoS values differently; these vendor-specific constants are stored verbatim in rosbag2 metadata. Bags recorded with FastDDS cannot be played back on CycloneDDS and vice versa, producing 'DEADLINE invalid' errors. This creates a hidden interoperability dependency that silently breaks bag playback when operators switch DDS implementations.

**210. Service interoperability with FastRTPS**  
2020-05-12 · GitHub ros2/rmw_cyclonedds#184 · [link](https://github.com/ros2/rmw_cyclonedds/issues/184)  

When a CycloneDDS service client calls a FastRTPS service server, the server receives corrupted request data (random integers instead of the correct values) and the client times out receiving no response. The failure is directional and RPC-specific: pub/sub across the two vendors works, same-vendor service calls work, but cross-vendor RPC is broken due to incompatible request/reply metadata serialization between the two implementations.

**211. CDR wstring serialization is nonstandard**  
2019-09-23 · GitHub ros2/rmw_cyclonedds#43 · [link](https://github.com/ros2/rmw_cyclonedds/issues/43)  

CycloneDDS violates the DDS-XTypes v1.2 specification for wide string (wstring) CDR serialization: the spec mandates UTF-16 encoding without BOM or NUL terminator, but CycloneDDS uses a nonstandard encoding that is compatible with Fast-RTPS but not with Connext DDS. Fixing the serialization to match the spec would break cross-vendor compatibility with Fast-RTPS while restoring it with Connext, forcing a painful choice between spec compliance and existing deployed behavior.

**212. RTI Connext 6.0.0 ROS2 compilation error — template type_code mismatch**  
2019-04-26 · RTI Community forum · [link](https://community.rti.com/forum-topic/rti-connext-600-ros2-compilation-error)  

Upgrading from Connext 5.3.1 to 6.0.0 broke the ROS2 build with the error ''type_code' is not a class template struct type_code<ConnextStaticSerializedData>' in connext_static_serialized_data.h. Both RTI and Open Robotics acknowledged the incompatibility; the only workaround was reverting to Connext 5.3.1. This blocked users from using the then-current Connext release with ROS2.

**213. Using different typesupports on same node and topic ends in segfault**  
2019-03-21 · GitHub ros2/rmw_fastrtps#265 · [link](https://github.com/ros2/rmw_fastrtps/issues/265)  

When a single node creates both a C typesupport publisher (via rosout) and a C++ typesupport subscriber on the same topic (e.g., `/rosout`), rmw_fastrtps segfaults in the type-registration path. The crash is non-deterministic and depends on commented-out code elsewhere in the process, suggesting a race or corruption in the shared type registry.

**214. Fast-RTPS cross vendor tests are failing frequently on Windows**  
2018-12-13 · GitHub ros2/rmw_fastrtps#246 · [link](https://github.com/ros2/rmw_fastrtps/issues/246)  

When running a talker with RTI Connext and a listener with Fast-RTPS on Windows (or vice versa), the listener receives nothing from the talker. Communication works when both endpoints use the same vendor. Identified as the root cause of frequent failures in the test_communication cross-vendor CI suite. No technical root cause is stated beyond the simple inter-vendor mismatch.

**215. ROS2 + DDS: A Field Guide to Interoperability**  
2018-09-27 · RTI blog · [link](https://www.rti.com/blog/ros2-dds-a-field-guide-to-interoperability)  

RTI documents that the ROS 2 RMW layer introduces a serialized type representation for all 'rt/*' topics causing QoS mismatches when standard DDS applications connect without suppressing typecode announcements via XML overrides. ROS 2 encodes topic namespaces in DDS topic names and partitions in non-obvious ways. ROS 2 uses unbounded sequences/strings, requiring explicit 'Unbounded Support' in DDS code generators to interoperate, all of which require expert XML configuration not exposed in ROS 2 tools.

**216. RTI Connext and CycloneDDS services not interoperable — RMW_CONNEXT_CYCLONE_COMPATIBILITY_MODE required**  
n/a · rmw_connextdds Runtime Configuration Docs · [link](https://rmw-connextdds.readthedocs.io/en/latest/user/runtime-cfg.html)  

By default, ROS2 applications using rmw_connextdds cannot communicate with rmw_cyclonedds_cpp via ROS2 clients and services (only publishers/subscribers work). The root cause is that CycloneDDS uses a custom non-standard mapping for propagating request metadata between clients and services, diverging from the DDS-RPC specification that Connext follows. A special environment variable RMW_CONNEXT_CYCLONE_COMPATIBILITY_MODE=y forces Connext to abandon DDS-RPC compliance to match CycloneDDS's proprietary approach.

---

## Large Data / Fragmentation (images, point clouds, 262 kB ceiling)

*29 items*

**217. FastDDS High Latency using Large Data**  
2025-03-05 · GitHub eProsima/Fast-DDS#5686 · [link](https://github.com/eProsima/Fast-DDS/issues/5686)  

Transferring ~4 MB image frames between Fast-DDS 2.13.3 participants over SHM (with --ipc=shareable Docker containers) achieved ~20 ms latency — worse than Zenoh (~10 ms) and ZeroMQ, despite Fast-DDS explicitly leveraging shared memory. Testing both the large_data_builtin_transports_options and video_publisher_qos profiles yielded similar poor results, suggesting the LARGE_DATA mode or SHM path had unresolved overhead for multi-MB messages.

**218. Unusual performance when working with large messages**  
2024-11-15 · GitHub eclipse-cyclonedds/cyclonedds#2139 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/2139)  

With CycloneDDS in ROS 2, a single subscriber to a ~5 MB pointcloud topic published at 10 Hz receives only 10–20 % of messages; at higher frequencies the single subscriber receives 0 %. Counterintuitively, adding a second subscriber causes both to receive nearly all messages. FastDDS handles the same scenario without degradation, making CycloneDDS's resource scheduling the suspected cause.

**219. Inconsistent Network Bandwidth Consumption in ROS2 Image Transmission**  
2024-04-19 · GitHub ros2/ros2#1544 · [link](https://github.com/ros2/ros2/issues/1544)  

Transmitting 15 MB RGB images at 2 fps (expected: ~30 MB/s) actually consumed ~57 MB/s — nearly double — when using regular subscriptions or 'ros2 topic echo'. Diagnostic tools 'ros2 topic bw' and 'ros2 topic hz' correctly reported ~30 MB/s, suggesting fragmentation overhead or redundant DDS message copies in the FastRTPS data path inflate bandwidth for large messages in multi-subscriber fleet scenarios.

**220. [22208] The SocketTransportDescriptor::min_send_buffer_size() method returns too large value when exceeded net.core.wmem_max**  
2024-04-14 · GitHub eProsima/Fast-DDS#4684 · [link](https://github.com/eProsima/Fast-DDS/issues/4684)  

SocketTransportDescriptor's sendBufferSize and receiveBufferSize fields and the min_send_buffer_size() API reported values larger than the actual OS socket buffers when the requested size exceeded net.core.wmem_max or net.core.rmem_max. This caused users to believe large-data buffers were correctly configured while messages were still being dropped due to the smaller actual socket buffer.

**221. ROS 2 and Large Data transfer on lossy networks**  
2024-03-12 · ROS Discourse discourse.openrobotics.org/t/ros-2-and-large-data-transfer-on-lossy-networks/36598 · [link](https://discourse.openrobotics.org/t/ros-2-and-large-data-transfer-on-lossy-networks/36598)  

LMoreno (eProsima) documented that transferring large data over WiFi with DDS requires complex per-vendor configuration: UDP transport lacks built-in flow control so entire RTPS packets are discarded under congestion; messages above ~65 KB fragment and partial fragment loss silently drops the entire sample; and TCP fallback requires matching configuration on both sender and receiver since endpoints cannot auto-negotiate transport type, meaning a misconfigured peer simply receives nothing.

**222. Messages dropped while multiple publishers are publishing, help with XML profile**  
2024-03-05 · GitHub ros2/rmw_fastrtps#747 · [link](https://github.com/ros2/rmw_fastrtps/issues/747)  

Recording 35 concurrent publishers (Ouster lidar at 20 Hz, Intel RealSense at 30 Hz, 200 Hz IMU) with FastDDS drops ~15,481 messages. CycloneDDS requires 250 MB receive buffers to handle the same load; FastDDS's default sendSocketBufferSize of 1 MB and listenSocketBufferSize of 4 MB are far smaller, causing drops under high concurrent-publisher load.

**223. Message sizes greater than around 262 kB drop out and don't get received**  
2022-10-28 · GitHub eProsima/Fast-DDS#3053 · [link](https://github.com/eProsima/Fast-DDS/issues/3053)  

calvertdw reported that Fast-DDS silently drops UDP messages larger than approximately 262 kB after briefly transmitting for a second or two, regardless of socket buffer size configuration. Affected use cases include 4K video (1 MB payloads) and Realsense L515 colored point clouds (4 MB). The 262 kB threshold corresponds to the Linux kernel's default ipfrag_high_thresh, where fragment reassembly buffers fill and subsequent fragments are silently discarded. Issue remained open without milestone.

**224. Lost large messages across subnets**  
2022-09-20 · ROS Answers (answers.ros.org / robotics.stackexchange.com) · [link](https://answers.ros.org/question/406653/lost-large-messages-across-subnets/)  

640x480 RGB images (large payload) fail to arrive when transmitted across subnet boundaries, while 160x120 images and small topics (tf, camera_info) work fine. The responder suspects IP fragmentation failures: when intermediate network devices do not properly reassemble fragmented UDP datagrams, DDS-level fragmentation of messages exceeding ~64 KB results in silent data loss. The issue remained unresolved after standard buffer tuning.

**225. On-Going Issues with Large Topics**  
2022-07-01 · GitHub ros2/ros2#1289 · [link](https://github.com/ros2/ros2/issues/1289)  

A high-profile tracking issue opened by a ROS 2 maintainer documenting that working with camera, depth camera, and 3D LiDAR topics remained fundamentally broken in ROS 2 by the second LTS release. A benchmark paper showed messages larger than ~1 MB caused disproportionate problems. DDS vendors treated it as an edge case despite it being required for nearly every robotic application. Teams at Samsung could not build sensor processing pipelines without manual node composition.

**226. ROS2-Galactic DDS Problems**  
2022-05-19 · ROS Discourse · [link](https://discourse.openrobotics.org/t/ros2-galactic-dds-problems/25654)  

A student team experienced 2–3 second end-to-end camera latency and constant stream freezes when transmitting uncompressed 480p video over 1 Gb Ethernet using ROS 2 Galactic with DDS, with 0.03 % packet loss on bare Ubuntu but 10–20 % on VM or IoT hardware. The underlying cause was identified as uncompressed image transport generating excessive DDS fragmentation and CPU load.

**227. Very slow publishing of large messages**  
2022-02-08 · GitHub ros2/ros2#1242 · [link](https://github.com/ros2/ros2/issues/1242)  

Publishing sensor_msgs/Image frames (640×480 BGR8, ~900 KB) with Fast-DDS over Ethernet between two Debian machines resulted in very slow or stalled delivery. The pipeline used a background thread which may have interacted with the Fast-DDS async writer queue. Investigation pointed to the RTPS fragment send loop stalling when the socket send buffer was full, with no back-pressure mechanism.

**228. Communication between nodes fails for non small-sized images ros2 topics**  
2021-11-14 · GitHub ros2/rmw_fastrtps#570 · [link](https://github.com/ros2/rmw_fastrtps/issues/570)  

Using ROS 2 Galactic with Fast-DDS Discovery Server on two Windows 10 machines, small (100×100 pixel) camera images were received normally but larger images were never delivered to the subscriber. Resizing the image below the fragmentation threshold restored communication, confirming the problem was message-size-dependent. Several buffer-size workarounds from related issues did not resolve it.

**229. 2021 ROS Middleware Evaluation Report**  
2021-10-14 · OSRF TSC-RMW-Reports osrf.github.io/TSC-RMW-Reports/humble/ · [link](https://osrf.github.io/TSC-RMW-Reports/humble/)  

Official TSC evaluation found both CycloneDDS and FastDDS drop messages at large payload sizes: CycloneDDS starts dropping at 2 MB, FastDDS async mode at 1 MB. CycloneDDS latency roughly doubles per size step beyond 1 MB. Both implementations require 'additional configuration' for WiFi — neither works reliably out-of-the-box over wireless. Service replies are broadcast to all clients causing scalability degradation. CPU and memory consumption increased compared to 2020 baselines with unclear root cause.

**230. occasional long message delay in remote node subscription of pointcloud**  
2020-09-29 · GitHub ros2/rmw_fastrtps#454 · [link](https://github.com/ros2/rmw_fastrtps/issues/454)  

A 96×128 point cloud (16 bytes/point) published at 15 Hz was received at 15 Hz on the local machine but on a remote Ethernet-connected machine experienced ~10-second blackout periods every few minutes where no point clouds arrived. The issue was attributed to UDP IP-fragment loss on the network link causing the Linux reassembly buffer to fill, blocking all further fragments for up to 30 s.

**231. poor performance with pub/sub of pointclouds between computers**  
2020-09-29 · GitHub ros2/rmw_cyclonedds#251 · [link](https://github.com/ros2/rmw_cyclonedds/issues/251)  

On ROS 2 Foxy with CycloneDDS, a 96×128 16-byte-per-point pointcloud published at 15 Hz (~3 MB/s) arrives at only 0.1–0.3 Hz on a remote machine over a ~30 MB/s Ethernet link. FastRTPS gives better results but still produces occasional 10-second message gaps. The same data flows fine locally at 15 Hz, isolating the problem to CycloneDDS's inter-host transport.

**232. Rosbag2 silently stops recording image and pointcloud2 topics**  
2020-08-08 · GitHub ros2/rosbag2#498 · [link](https://github.com/ros2/rosbag2/issues/498)  

rosbag2 on ROS 2 Foxy with Fast-DDS silently stopped receiving image and PointCloud2 topics after 10–60 seconds of recording while smaller topics continued uninterrupted. rviz2 confirmed the publishers were still active. Drops started and stopped synchronously across all large topics, suggesting a shared DDS transport resource (likely IP fragment reassembly buffer) was periodically exhausted and then recovered after ~10–15 minutes.

**233. Large messages are being dropped sometimes**  
2020-06-15 · GitHub ros2/ros2#946 · [link](https://github.com/ros2/ros2/issues/946)  

Sending 10 MB string messages at 10 Hz in ROS 2 Foxy resulted in dropped or very-late messages with Fast-RTPS; the same test with CycloneDDS also showed drops. On the same machine (loopback), some messages took seconds to arrive or never arrived. The reporter bisected the issue to large fragmented RTPS DATA submessages overwhelming the receive queue.

**234. Send big data, subscriber not responses [8367]**  
2020-05-09 · GitHub eProsima/Fast-DDS#1205 · [link](https://github.com/eProsima/Fast-DDS/issues/1205)  

A user tried to send 1–2 MB payloads using an unbounded sequence<octet> in FastRTPS 1.10 with ASYNCHRONOUS_PUBLISH_MODE. Once the payload exceeded 128 bytes the subscriber received nothing. The fix required setting receiveBufferSize to at least the size of a full sample and raising net.core.rmem_max accordingly — neither requirement was surfaced by any error message.

**235. Reduce how eager CycloneDDS is in retransmits**  
2020-04-10 · GitHub eclipse-cyclonedds/cyclonedds#484 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/484)  

Publishing 9.8 MB pointcloud data with CycloneDDS can stall for up to 100 seconds due to overly aggressive retransmission when socket receive buffers are insufficient. CycloneDDS lacks adaptive retransmission logic, forcing users to pre-configure up to 20 MB receive buffers (requiring root access) to avoid the stall, and knowing the maximum payload size in advance.

**236. Very slow message delivery in some cases with Fast-RTPS 1.9.x [8914]**  
2020-04-01 · GitHub eProsima/Fast-DDS#1120 · [link](https://github.com/eProsima/Fast-DDS/issues/1120)  

A ROS 2 Foxy point cloud pipeline (trigger → image filter → disparity → point cloud node) that should complete in under 500 ms took anywhere from 500 ms to 30 s, or never completed under load. Timing traces showed messages stalling inside FastRTPS on the inter-node hop carrying the large point cloud payload. The issue was attributed to the bursty nature of large RTPS fragment bursts overwhelming the UDP receive path.

**237. make the maximum message size of UDP packets configurable [7416]**  
2020-01-23 · GitHub eProsima/Fast-DDS#976 · [link](https://github.com/eProsima/Fast-DDS/issues/976)  

The maximum UDP packet size was hard-coded in TransportInterface.h, meaning every RTPS datagram was sent as a single large UDP packet that the OS then IP-fragmented at the MTU boundary (~1500 B). On lossy links even a single lost IP fragment caused the whole RTPS message to be discarded. The request was to make this configurable so users could send sub-MTU packets, trading overhead for reliability.

**238. [solved e90e4b7] eProsima Fast-RTPS Segmentation Fault**  
2018-01-19 · GitHub eProsima/Fast-DDS#181 · [link](https://github.com/eProsima/Fast-DDS/issues/181)  

A segfault occurred inside the fragmentation send path (StatefulWriter::send_any_unsent_changes → RTPSMessageGroup::add_data_frag) when a CacheChange scoped copy went out of scope before memmove completed, resulting in a use-after-free. The bug was triggered specifically when a large change required fragmentation and security was enabled. The crash happened in both security and non-security builds under ROS 2 Ardent.

**239. "Buffer too small" when sending service response that requires fragmentation**  
2017-06-29 · GitHub eProsima/Fast-DDS#117 · [link](https://github.com/eProsima/Fast-DDS/issues/117)  

ROS 2 service calls with large responses triggered '[RTPS_WRITER Error] Cannot add RTPS submessage to the CDRMessage. Buffer too small' and 'Error sending fragment (1, 69)'. The request side (fragmentation in the request) worked fine but the response publisher configured with ASYNCHRONOUS_PUBLISH_MODE failed. The issue exposed that service response publishers were not correctly sized for fragmented payloads.

**240. msg field exceeds the maximum length**  
2017-05-11 · GitHub eProsima/Fast-DDS#103 · [link](https://github.com/eProsima/Fast-DDS/issues/103)  

A user publishing a 720-byte message received an undocumented error 'msg field exceeds the maximum length'. Neither the maximum message size nor how to increase it was documented anywhere in the FastRTPS API or user guide at the time. The issue highlighted that even sub-1 KB payloads could silently hit hidden size limits.

**241. Errors sending large data (video frames up to 150 KB)**  
2017-03-14 · GitHub eProsima/Fast-DDS#83 · [link](https://github.com/eProsima/Fast-DDS/issues/83)  

Sending video frames of up to 150 KB with ASYNCHRONOUS_PUBLISH_MODE: the first several frames transmit successfully, then almost every other frame fails. The data_msg_length field showed a value near ULONG_MAX, indicating a buffer-sizing bug in the fragmentation path. The publisher had sendSocketBufferSize set to 201000 but fragmentation still miscomputed message sizes.

**242. Best effort + fragmentation + OS X = no images**  
2017-03-01 · GitHub ros2/rmw_fastrtps#93 · [link](https://github.com/ros2/rmw_fastrtps/issues/93)  

Publishing 320×240 RGB images at 30 fps with best-effort reliability on macOS received zero images on the same machine, while Linux worked fine. The kernel UDP buffer was overwhelmed by bursty fragment bursts with no flow controller. Counterintuitively, increasing publish frequency to 100+ fps improved reception rate (partial frames), and enabling a throughput controller fixed reception at the cost of limiting data rate.

**243. Maximum message payload size**  
2016-02-19 · GitHub eProsima/Fast-DDS#20 · [link](https://github.com/eProsima/Fast-DDS/issues/20)  

Early FastRTPS had a hard ceiling near 64000 bytes enforced at type registration ('Current version only supports types of sizes < 64000'), a 'buffer too small' error at ~16000 bytes, and a 'Message too long' send error at ~8650 bytes on macOS. The 65536-byte uint16_t overflow caused a compiler warning. Multiple distinct size ceilings existed across platforms with no documented workaround.

**244. DDS Middleware and Network Tuning (ZED Camera ROS 2)**  
n/a · Stereolabs ROS 2 documentation · [link](https://www.stereolabs.com/docs/ros2/dds-and-network-tuning)  

Stereolabs explicitly warns: 'If you don't apply these settings, ROS 2 nodes will fail to receive and send large data like point clouds or images.' Default Linux IP fragment reassembly buffer is only 4 MB and times out after 30 seconds; kernel receive buffer defaults to 4 MB. Cyclone DDS out-of-box settings do not accommodate ZED camera image and point cloud sizes. System-wide kernel tuning (rmem_max to 2 GB, ipfrag_high_thresh to 128 MB) is mandatory before any data flows.

**245. DDS settings for ROS 2 and Autoware**  
n/a · Autoware Foundation documentation · [link](https://autowarefoundation.github.io/autoware-documentation/main/installation/additional-settings-for-developers/network-configuration/dds-settings/)  

Autoware documentation warns: 'If you don't tune these settings, Autoware will fail to receive large data like point clouds or images.' Default OS receive buffer is only 208 KiB, far below what autonomous driving sensor fusion requires. IP fragmentation defaults (30-second timeout, 256 KiB threshold) cause packet loss with large LiDAR point cloud transmissions. Mandatory prescriptions: increase buffers to 2 GiB, reduce fragment timeout, configure Cyclone DDS socket receive buffer and watermarks.

---

## DDS-Security / SROS2

*22 items*

**246. [23025] Discovery Matching fails when discovery_protection_kind=ENCRYPT and topic-level protection are both enabled**  
2025-04-08 · GitHub eProsima/Fast-DDS#5753 · [link](https://github.com/eProsima/Fast-DDS/issues/5753)  

When both `discovery_protection_kind=ENCRYPT` in governance.xml and per-topic protection (data + metadata ENCRYPT) are enabled in Fast-DDS DDS Security, publisher and subscriber silently fail to match. No `on_publication_matched` or `on_subscription_matched` callbacks fire. Disabling discovery protection makes matching work, indicating a bug in how encrypted SEDP packets interact with topic-level security filters.

**247. ROS2 Humble security settings and certificates — discovery fails after enabling Fast-DDS security with MicroXRCEAgent**  
2025-03-13 · GitHub eProsima/Fast-DDS#5707 · [link](https://github.com/eProsima/Fast-DDS/issues/5707)  

A developer configuring ROS2 Humble with Fast-DDS PKI-DH authentication, Access-Permissions control, and AES-GCM-GMAC encryption sees MicroXRCEAgent start correctly in one terminal, but discovery fails in a second terminal sourcing the same security configuration when running 'ros2 doctor --report'. Signed S/MIME governance and permissions files were verified correct; the issue appears to be inter-participant discovery or certificate validation failure between simultaneous secure connections. Issue remains open with no resolution.

**248. Security vulnerabilities due to incomplete privilege inheritance in ROS2 and ROS1**  
2024-08-07 · GitHub ros2/ros2#1589 · [link](https://github.com/ros2/ros2/issues/1589)  

ROS 2 nodes can create new contexts via rclcpp::Context or rclpy.context.Context without inheriting parent settings such as Domain ID, Namespace, or SROS2 security configuration. An attacker controlling a node can instantiate a blank context to disable SROS2 enforcement, then launch new child nodes that freely access unprotected topics or reassign leaked enclave credentials. The issue remained open with 'more-information-needed' status.

**249. ROS_SECURITY_ENCLAVE_OVERRIDE does not effectively work — ros2 node list / topic list show only system topics**  
2024-05-08 · GitHub ros2/sros2#306 · [link](https://github.com/ros2/sros2/issues/306)  

The ROS_SECURITY_ENCLAVE_OVERRIDE environment variable, introduced to allow ros2cli debugging in secured networks, intermittently fails: 'ros2 node list' and 'ros2 topic list' return only /parameter_events and /rosout instead of application nodes and the /chatter topic. The bug reproduces with both Fast-DDS and CycloneDDS and also occurs with --no-daemon, effectively preventing operators from inspecting a running secured ROS 2 graph.

**250. Bind security enclaves to ros2cli commands for debug purpose — node list returns empty with security enabled**  
2024-04-17 · GitHub ros2/sros2#293 · [link](https://github.com/ros2/sros2/issues/293)  

When SROS2 security is active, 'ros2 node list' returns empty output because the cli tool does not participate in any security enclave; developers cannot inspect the node graph or use any introspection tools in secured deployments. The issue requests a --enclave flag on ros2cli commands to grant temporary access; the gap effectively forces users to either disable security for debugging or accept complete observability blindness. Closed via PR #295.

**251. Disconnect Vulnerability in RTPS Packets Used by SROS2 (CVE-2023-50257)**  
2024-02-19 · GitHub eProsima/Fast-DDS Security Advisory GHSA-v5r6-8mvh-cp98 · [link](https://github.com/eProsima/Fast-DDS/security/advisories/GHSA-v5r6-8mvh-cp98)  

Unencrypted disconnect data (p[UD]) and GUID values in RTPS packets allow a network attacker to capture Publisher Participant IDs and forge malicious disconnect packets sent to the multicast address 239.255.0.1:7400, forcibly severing all subscriber connections. The SecurityManager does not reinitialize after receiving spoofed packets with duplicate GUIDs, leaving cached security tokens blocking reconnection. Affects Fast-DDS before versions 2.13.0/2.12.2/2.11.3/2.10.3/2.6.7 across all tested ROS 2 distributions (Humble, Galactic, Foxy, Iron). CVSS score 9.6 Critical.

**252. Enabling ROS 2 Security with domain_bridge crashes with 'couldn\'t find all security files'**  
2023-06-13 · GitHub ros2/domain_bridge#74 · [link](https://github.com/ros2/domain_bridge/issues/74)  

When domain_bridge is configured to bridge a secured ROS 2 domain (sros2 enabled) with an unsecured domain, the bridge crashes at startup with 'couldn\'t find all security files! at ./src/participant.cpp:274' followed by 'rcl node\'s rmw handle is invalid'. The same enclave configuration works with regular nodes, but domain_bridge never implemented full security support — the design doc had a 'TODO' for two years before this report. Keystores being domain-ID-dependent further complicates multi-domain secured setups.

**253. Security Interop: CycloneDDS <-> FastDDS. FastDDS subscriber and Cyclone publisher not working**  
2023-01-23 · GitHub eclipse-cyclonedds/cyclonedds#1547 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/1547)  

DDS Security interop between CycloneDDS and FastDDS is asymmetric: FastDDS publisher + CycloneDDS subscriber works, but CycloneDDS publisher + FastDDS subscriber fails. The FastDDS side logs 'Failed to convert octet sequence to ASN1 integer', indicating incompatible security handshake message encoding between the two vendors despite using identical certificates and governance files.

**254. Chain of trust issues with a single CA certificate**  
2022-12-13 · GitHub ros2/sros2#282 · [link](https://github.com/ros2/sros2/issues/282)  

SROS2 keystores use a single CA symlinked as both Identity CA and Permissions CA. A compromised node can therefore create unauthorized permissions.xml content, sign it with its own enclave certificate (which is signed by that same Identity CA), and distribute forged permission documents that other participants accept as valid. The enclave certificate requires the digitalSignature flag for DDS-Security, making certificate-flag workarounds impractical; the recommended fix is to separate Identity CA and Permissions CA into distinct certificates.

**255. On the (In)Security of Secure ROS2 — four software-level vulnerabilities including permission non-revocation**  
2022-09-15 · ACM CCS 2022 paper (sites.google.com/view/secure-sros2) · [link](https://sites.google.com/view/secure-sros2)  

Researchers identified four software-level vulnerabilities in SROS2. A key design flaw (V1) is that permission revocation requires the target node to voluntarily restart its DDS publisher/subscriber services; an adversarial node can refuse this restart and continue publishing/subscribing to topics whose access is supposed to be revoked. Certificate revocation via CRL was integrated into ROS2 rolling as the primary mitigation after the paper's acceptance at ACM CCS 2022.

**256. Cybersecurity in the ROS 2 communication middleware, targeting the top 6 DDS implementations**  
2021-11-26 · ROS Discourse (discourse.openrobotics.org) · [link](https://discourse.openrobotics.org/t/cybersecurity-in-the-ros-2-communication-middleware-targeting-the-top-6-dds-implementations/23254)  

Researchers disclosed CVEs across all six major DDS implementations used by ROS 2: buffer overflows in RTI Connext (CVE-2021-38435) and GurumDDS, denial-of-service via crafted RTPS packets in OpenDDS and Fast-DDS (CVE-2021-38447, CVE-2021-38425), write-what-where conditions in CycloneDDS\'s XML parser (CVE-2021-38441, CVE-2021-38443), and a reflection/amplification attack using PID_METATRAFFIC_MULTICAST_LOCATOR manipulation. The community discussion highlighted the need for a dedicated RTPS dissector tool maintained by the ROS 2 Security Working Group.

**257. Cryptography Error when running talker_listener on two machines with security enabled**  
2021-05-05 · GitHub ros2/sros2#263 · [link](https://github.com/ros2/sros2/issues/263)  

After successfully configuring SROS2 on a single machine and copying the keystore to a second machine, the listener receives nothing despite the talker publishing. The listener logs 'Received Writer Cryptography message but not found local reader' and 'Received Reader Cryptography message but not found local writer', indicating a cryptographic key or certificate mismatch between distributed keystores. Root cause was not conclusively identified in the issue thread.

**258. Unable to run SROS with CycloneDDS on ROS 2 foxy**  
2020-10-21 · GitHub ros2/ros2#1051 · [link](https://github.com/ros2/ros2/issues/1051)  

The official ROS2 Foxy Debian package for CycloneDDS omits the DDS-Security authentication library (`dds_security_auth`), causing all SROS-secured nodes to fail at startup with 'Could not load Authentication library: dds_security_auth: cannot open shared object file'. The security plugin and DDS participant creation both fail, making the binary package non-functional for any security use case. Building from source was the only workaround.

**259. generate_artifacts not creating ros_discovery_info topic permissions under CycloneDDS rmw**  
2020-09-16 · GitHub ros2/sros2#242 · [link](https://github.com/ros2/sros2/issues/242)  

The ros2 security generate_artifacts command omits publish/subscribe permissions for the ros_discovery_info topic when RMW_IMPLEMENTATION=rmw_cyclonedds_cpp. Without these permissions, nodes fail with 'failed to create topic' under SROS2 enforce mode. Generating artifacts with FastRTPS first and then running CycloneDDS works around the missing permission entries.

**260. Option for smaller or lossy permissions — default enclaves exceed 64 KB causing DDS handshake failure**  
2020-07-28 · GitHub ros2/sros2#228 · [link](https://github.com/ros2/sros2/issues/228)  

SROS2's default artifact generation produces per-node permission files that list every action server/client topic explicitly; with a moderate number of node profiles, the signed permissions.p7s file exceeds the 64 KB RTPS packet property size limit, causing the DDS Security handshake to fail silently. The proposed mitigation is to allow lossy POSIX wildcard compression of permission rules (e.g. 'rq/talker/*' instead of individual entries) during the XSL transform phase.

**261. SROS2 leaks node information, regardless of rtps_protection_kind setup**  
2019-12-06 · GitHub ros2/sros2#172 · [link](https://github.com/ros2/sros2/issues/172)  

Despite enabling SROS2 encryption via rtps_protection_kind, ROS 2 node details remain visible on the network and can be retrieved with standard ros2cli tools. The leak is always reproducible; even after regenerating keys and rebuilding, node metadata is disclosed. CVE-2019-19627 was assigned; the only documented mitigation is using static endpoints and avoiding dynamic discovery, rather than a proper fix.

**262. SROS2 fails to use the DOMAIN_ID — nodes crash with non-zero ROS_DOMAIN_ID**  
2019-11-05 · GitHub ros2/sros2#169 · [link](https://github.com/ros2/sros2/issues/169)  

When security is enabled and ROS_DOMAIN_ID is set to a non-default value (e.g. 10), nodes fail to initialize with 'Not found topic access rule for topic rt/rosout' because the security credentials were generated without the correct domain ID. Both C++ and Python nodes crash with 'create_publisher() could not create publisher'. The workaround is to export ROS_DOMAIN_ID before running the keystore/key creation commands.

**263. fix certificate start date to work regardless of the timezone — nodes fail with 'certificate is not yet valid'**  
2019-08-01 · GitHub ros2/sros2 Pull Request #148 · [link](https://github.com/ros2/sros2/pull/148)  

When a machine generating SROS2 certificates has a local clock ahead of UTC, the generated certificate's 'not before' validity timestamp is set in the future relative to UTC-based validation, causing node startup to fail. Fast-RTPS reports 'Error validating the local participant identity'; RTI Connext reports X509 error 9 'certificate is not yet valid'. The fix switches certificate generation to use UTC timestamps.

**264. Node crashes without showing an error message when security enabled on Windows**  
2019-05-19 · GitHub ros2/sros2#116 · [link](https://github.com/ros2/sros2/issues/116)  

On Windows Server 2019 with FastRTPS, running any non-talker/listener ROS 2 example (e.g. intra_process_demo) with SROS2 security enabled causes the process to exit silently with errorlevel -1. No diagnostic message is printed, making root-cause analysis impossible. The issue remained unresolved, reflecting both the security initialization problem and the absence of any error surfacing on Windows.

**265. Tutorial permissions not working — KeyError: 'nodes' and broken policy file URL in official SROS2 tutorial**  
2019-02-18 · GitHub ros2/sros2#79 · [link](https://github.com/ros2/sros2/issues/79)  

The official SROS2 Linux tutorial\'s access control section had a broken URL to the sample policy file, a YAML formatting error producing 'could not find expected ":"', and a missing 'services' key producing KeyError when generating permission files. All three errors occurred sequentially, preventing any user from completing the tutorial without first applying PRs #72 and #80. Reflects the high friction of SROS2 artifact management.

**266. Secure DDS interoperability — Fast-RTPS and RTI Connext cannot communicate with identical security files**  
2018-08-31 · GitHub eProsima/Fast-DDS#250 · [link](https://github.com/eProsima/Fast-DDS/issues/250)  

Using identical security configuration files, Fast-RTPS and RTI Connext fail to discover or connect to each other\'s topics when DDS Security is enabled, despite communicating fine without security. The implementations connect successfully with their own vendor peers, but cross-vendor secured communication is entirely blocked. No resolution was documented in the issue, indicating a fundamental interoperability gap in the DDS-Security standard implementations relevant to ROS 2 multi-vendor deployments.

**267. Bionic: FastRTPS security tests fail — OpenSSL certificate format mismatch**  
2018-04-30 · GitHub ros2/sros2#46 · [link](https://github.com/ros2/sros2/issues/46)  

On Ubuntu Bionic with OpenSSL 1.1.0, FastRTPS security tests fail with 'Invalid CA error'. Certificates generated by sros2 are in SSLv1 format while FastRTPS expects SSLv3-compatible certificates; tests generated by FastRTPS itself pass, but sros2-generated credentials do not. The fix required changing sros2\'s certificate generation to produce OpenSSL 1.1-compatible output, an incompatibility that caused a broad swath of ROS 2 security setup failures on that Ubuntu LTS.

---

## Configuration Complexity (XML tuning, hidden prerequisites)

*21 items*

**268. "I'm done manually tuning DDS parameters!" — hundreds of knobs, days of trial-and-error, suboptimal results**  
2026-04-30 · ROS Discourse (openrobotics) · [link](https://discourse.openrobotics.org/t/im-done-manually-tuning-dds-parameters/54415)  

A ROS2 user describes spending hours to days manually tweaking DDS XML parameters with no systematic guidance on where to start for latency or throughput goals. With over 20 interacting QoS policies and vendor-specific XML profiles, QoS conflicts emerge silently at runtime rather than at configuration time. The post prompted community discussion about AI-assisted tuning tools as a workaround for what users describe as an inherently unmanageable configuration surface.

**269. DDS in ROS 2: Consolidated User Insights**  
2025-12-09 · ROS Discourse (discourse.openrobotics.org) · [link](https://discourse.openrobotics.org/t/dds-in-ros-2-consolidated-user-insights/51340)  

A consolidation thread cataloging ongoing DDS problem areas in ROS 2: multicast-related discovery challenges, WiFi connectivity concerns, QoS policy conflicts requiring static analysis tooling (referenced 'QoS Guard' tool and 'Dependency Chain Analysis of ROS 2 DDS QoS Policies' paper), and IP fragmentation plus buffer burst bottlenecks over WiFi. Academic research (DGIST CSI Lab) is cited for mathematical latency models and XML WiFi optimization profiles needed to achieve stable 30Hz video even with packet loss, indicating these are not solved problems in default configurations.

**270. ROS2 Jazzy Jalisco binary install on Windows searches for paid RTI Connext instead of free Fast DDS**  
2025-08-15 · GitHub ros2/ros2#1716 · [link](https://github.com/ros2/ros2/issues/1716)  

A fresh Jazzy binary install on Windows triggers the RTI Connext environment-script warning every time local_setup.bat is sourced, preventing the bundled demo talker/listener from starting. The install should default silently to Fast DDS, but the setup infrastructure still probes for the commercial Connext middleware first, confusing new users who have no RTI license.

**271. rmw_create_node: failed to create domain, error Error**  
2025-04-04 · GitHub ros2/rmw_cyclonedds#537 · [link](https://github.com/ros2/rmw_cyclonedds/issues/537)  

A user running ROS 2 Humble in an arm64 Docker container consistently fails to create any node with 'rmw_create_node: failed to create domain, error Error'. Despite configuring loopback networking and granting Docker --privileged and --network=host, node creation fails with an opaque error that propagates no diagnostic from the CycloneDDS layer, making root-cause analysis impossible.

**272. Network Interface selection**  
2025-04-02 · GitHub eclipse-cyclonedds/cyclonedds#2201 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/2201)  

Specifying a concrete NetworkInterface name (eth0) in the CycloneDDS XML config inside a Docker --network=host container causes a cryptic 'failed to create domain, error Error' from rmw_cyclonedds_cpp at startup, with no actionable diagnostic. Omitting the Interfaces block entirely (letting CycloneDDS auto-select) works, but then users have no control over which interface is used, defeating the purpose of explicit configuration.

**273. Tackling ROS 2 Networking Challenges**  
2025-01-16 · Clearpath Robotics blog · [link](https://clearpathrobotics.com/blog/2025/01/tackling-ros-2-networking-challenges/)  

Clearpath Robotics summarizes the most common networking failure modes seen across their customer base: daemon awareness gaps causing inconsistent discovery, bandwidth saturation from high-frequency topics in multi-robot deployments, QoS mismatches silently breaking communication, and multi-network complexity (robots with multiple NICs). The article prompted a dedicated full-day ROSCon 2024 workshop, indicating these are widespread production issues rather than edge cases.

**274. ZettaScale designs Zenoh to transcend DDS for automotive, ROS communications**  
2024-11-18 · The Robot Report · [link](https://www.therobotreport.com/zettascale-designs-zenoh-to-transcend-dds-for-automotive-ros-communications/)  

ZettaScale CEO Angelo Corsaro explains that DDS was optimized for closed wired naval combat management systems assuming low packet loss and plentiful bandwidth, and breaks down when used outside that design space. DDS cannot fit on microcontrollers and the wire protocol was not designed for constrained networks, forcing automotive/robotics teams to implement two to three different protocols for data flow across their systems. Wireless environments are specifically identified as problematic.

**275. DDS middleware complaint**  
2024-09-30 · ROS Discourse (discourse.openrobotics.org) · [link](https://discourse.openrobotics.org/t/dds-middleware-complaint/39821)  

User reports the default FastDDS in ROS 2 is broken with a concurrency issue in the RMW implementation causing service and topic discovery to fail non-deterministically. Additional complaints: DDS requires UDP and is unstable when network adapters are controlled by firewalls that block UDP; DDS connects to the whole network by default requiring manual domain ID or XML configuration; CycloneDDS 'floods the network and takes a metric ass ton of system resources compared to ros_comm'. On Raspberry Pi 4, each node consumes nearly a full CPU core for RMW message conversion overhead.

**276. ros2 commands fail on ROS 2 Jazzy when RMW_IMPLEMENTATION=rmw_cyclonedds_cpp**  
2024-06-17 · GitHub eclipse-cyclonedds/cyclonedds#2043 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/2043)  

On ROS 2 Jazzy, setting RMW_IMPLEMENTATION=rmw_cyclonedds_cpp with a custom CycloneDDS URI (for IPv6/Husarnet peer discovery) causes all ros2 commands to crash immediately with '*** buffer overflow detected ***: terminated'. The same config works on Humble, Galactic, and Iron, and using rmw_fastrtps_cpp on Jazzy works fine, pointing to a Jazzy-specific regression in the dds_create_domain path.

**277. QoS override via XML file fails: 'Change payload size larger than the history payload size'**  
2024-06-04 · GitHub ros2/rmw_fastrtps#764 · [link](https://github.com/ros2/rmw_fastrtps/issues/764)  

When using `RMW_FASTRTPS_USE_QOS_FROM_XML=1` with a profiles XML on Humble (FastDDS 2.6.7), the reader history's payload size is computed from the XML-specified `historyQos.depth` (too small) rather than the actual message size, causing all incoming discovery messages to be silently rejected with `RTPS_READER_HISTORY Error: Change payload size ... larger than history payload size`. The user's goal was to switch System Default QoS from Best_effort (FastDDS default) to Reliable to fix regressions after migrating from CycloneDDS.

**278. Oh rmw_zenoh, come quickly!**  
2024-03-22 · ROS Discourse (discourse.openrobotics.org) · [link](https://discourse.openrobotics.org/t/oh-rmw-zenoh-come-quickly/36769)  

Community thread expressing urgent desire for rmw_zenoh to replace DDS. Specific reported failures: with FastDDS, services fail to connect reliably especially with multithreaded executors; with CycloneDDS, node discovery takes multiple seconds requiring periodic 'ros2 daemon stop/start'; cross-network/VPN configuration is described as impossible to set up with any DDS implementation. One user documents 20x CPU overhead increase porting from ROS 1 to ROS 2 due to RMW conversion overhead; multiple respondents note DDS troubleshooting requires deep networking expertise unavailable to most robotics developers.

**279. ROS 2 Router for Remote Robotics and Topic Filtering**  
2023-11-14 · Husarnet blog · [link](https://husarnet.com/blog/ros2router)  

Husarnet identifies that standard ROS 2 requires uniform DDS configuration across the entire fleet and cannot filter topics between networks, making remote multi-robot deployments operationally fragile. Local multicast discovery does not function across internet-separated networks. Without the router, sensitive command topics like /cmd_vel are inevitably exposed alongside monitoring data, and any cross-network communication requires manually maintained IPv6 peer lists in XML config files.

**280. RTI Connext DDS installation procedure needs to be reorganised — missing rmw-connextdds package step**  
2023-05-09 · GitHub ros2/ros2_documentation#3573 · [link](https://github.com/ros2/ros2_documentation/issues/3573)  

The official ROS2 documentation for Connext DDS installation only describes the binary install but omits that users must separately install ros-{DISTRO}-rmw-connextdds for the middleware layer to be functional. Instructions are scattered across multiple pages with no clear sequence, causing users to end up with a Connext installation that silently falls back to another RMW at runtime.

**281. SROS2: Usable Cyber Security Tools for ROS 2 — acknowledged limitations in granularity and lifecycle management**  
2022-08-04 · arXiv 2208.02615 / IROS 2022 (Vilches et al.) · [link](https://arxiv.org/abs/2208.02615)  

The paper introducing SROS2\'s toolchain formally acknowledges two principal shortcomings: (1) lack of configuration granularity — it is not possible to independently configure authentication vs. encryption for individual topics or nodes; and (2) poor security artifact lifecycle management — updating certificates or rotating keys has no automated tooling, leading practitioners to either skip security or operate with stale credentials. The authors conclude that without usability improvements, security adoption in robotics will be severely impaired.

**282. Unpredictable behavior on machines that have multiple NICs**  
2022-03-22 · GitHub ros2/rmw_fastrtps#593 · [link](https://github.com/ros2/rmw_fastrtps/issues/593)  

On machines with multiple NICs (e.g., WiFi + Ethernet), rmw_fastrtps behaves unpredictably when a FastRTPS profiles XML file restricts traffic to one interface: nodes without the profile see topics listed by `ros2 topic list` but receive zero data silently. Additionally, discovery server settings appear to persist across processes that did not set them ('sticky' discovery server effect), causing silent data loss.

**283. RTI Connext DDS environment script not found (ROS2 on Windows)**  
2021-05-28 · ROS Answers (archive) · [link](https://answers.ros.org/question/379164/rti-connext-dds-environment-script-not-found-ros2-on-windows/)  

On a clean ROS2 Galactic binary install for Windows, running local_setup.bat prints '[rti_connext_dds_cmake_module][warning] RTI Connext DDS environment script not found', then all subsequent ros2 commands fail with 'failed to create process'. The setup script looks for rtisetenv_x64Win64VS2017.bat at a hard-coded path; if the commercial Connext package is absent the entire ROS2 environment is broken until the user manually switches to RMW_IMPLEMENTATION=rmw_fastrtps_cpp.

**284. Discovery problems with the discovery server ;-)**  
2021-04-13 · ROS Discourse (discourse.openrobotics.org) · [link](https://discourse.openrobotics.org/t/discovery-problems-with-the-discovery-server/19900)  

After configuring FastDDS Discovery Server, 'ros2 topic list' returns empty results and 'ros2 node info' shows no topic data despite active publishers. The root cause is that CLI tools need SUPER_CLIENT configuration to observe all participants through the server. On ROS 2 Foxy, attempting to use SUPER_CLIENT produces XML parse errors ('Node discoveryProtocol with bad content') because Foxy ships libfastrtps v2.0.2 which does not support SUPER_CLIENT (added in v2.3+); the workaround requires configuring the ROS 2 daemon as a server, an approach requiring expert XML configuration knowledge.

**285. Multi ECU configuration**  
2021-03-25 · GitHub eclipse-cyclonedds/cyclonedds#729 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/729)  

An autonomous-driving ROS 2 system with three ECUs (Main PC, Logging PC, Edge PC) on separate subnets cannot use multicast with multiple NICs simultaneously; the user confirmed that multiple network specification was not supported at the time. The only working solution is to disable multicast entirely and enumerate every peer IP address by hand in the Cyclone XML config. This makes the configuration brittle and unscalable for a 50+ node system.

**286. [10885] Discovery Server not working on WAN**  
2021-03-20 · GitHub eProsima/Fast-DDS#1842 · [link](https://github.com/eProsima/Fast-DDS/issues/1842)  

A Fast-DDS Discovery Server setup that works on LAN fails over WAN: publisher and subscriber are discovered and matched, but actual data transfer between them does not occur. The configuration uses TCP transport with NAT port forwarding for all three components. The failure is in the data plane, not the discovery plane, pointing to a TCP locator or WAN address resolution problem that Discovery Server does not transparently handle across NAT boundaries.

**287. [ros2] Configure QoS information from DEFAULT_FASTRTPS_PROFILES.xml — QoS override silently not applied**  
2021-01-12 · GitHub ros2/rmw_fastrtps#501 · [link](https://github.com/ros2/rmw_fastrtps/issues/501)  

Setting `RMW_FASTRTPS_USE_QOS_FROM_XML=1` with a FASTRTPS_DEFAULT_PROFILES_FILE to override QoS parameters (e.g., reliability, history) does not apply the changes to ROS 2 nodes on Eloquent and Foxy: `get_actual_qos()` still returns the default QoS values, making it impossible to configure DDS QoS from XML for production deployments without recompiling.

**288. ROS2 expects RTI Connext on hard-coded installation path, ignores NDDSHOME**  
2019-09-04 · GitHub ros2/rmw_connext#383 · [link](https://github.com/ros2/rmw_connext/issues/383)  

The ament_cmake export files for rmw_connext embed the absolute path C:/Program Files/rti_connext_dds-5.3.1/include, causing CMake to emit 'package exports include directory which doesn't exist' warnings for any install not at that exact path. NDDSHOME is ignored. This broke builds for all users who installed Connext to a non-default location and required patching the exported CMake files.

---

## Docker / Kubernetes / Cloud

*19 items*

**289. Isaac Sim in Docker unreachable and ignores CycloneDDS config**  
2026-01-09 · GitHub isaac-sim/IsaacSim#407 · [link](https://github.com/isaac-sim/IsaacSim/issues/407)  

Isaac Sim 5.1.0 running in Docker on a GPU server ignores a custom cyclonedds.xml that successfully enables cross-subnet DDS communication in standard ROS 2 containers. Despite identical ROS_DOMAIN_ID, host networking, and Tailscale overlay connectivity, Isaac Sim topics are unreachable from a ROS 2 container on a different subnet. Isaac Sim's container image overrides or ignores the CYCLONEDDS_URI environment variable, preventing any cross-host DDS configuration.

**290. ROS 2 Humble node fails to register in Docker with host when both WiFi and Ethernet are active**  
2024-10-23 · GitHub ros2/rmw_fastrtps#786 · [link](https://github.com/ros2/rmw_fastrtps/issues/786)  

In a Docker container using `network_mode: host`, a ROS 2 Humble node publishes a topic that is visible via `ros2 topic echo` from the host, but `ros2 topic info -v` reports `_NODE_NAME_UNKNOWN_` / `_NODE_NAMESPACE_UNKNOWN_` — participant registration is broken. The failure only occurs when both WiFi and Ethernet are simultaneously active; single-interface operation works. Configuring an interface whitelist in the rmw_fastrtps XML profile does not help.

**291. ROS 2 / DDS flying in Cloud with Cilium / Kubernetes**  
2024-03-27 · ROS Discourse discourse.openrobotics.org/t/ros-2-dds-flying-in-cloud-with-cilium-kubernetes/36845 · [link](https://discourse.openrobotics.org/t/ros-2-dds-flying-in-cloud-with-cilium-kubernetes/36845)  

Tomoya Fujita documented that mainstream Kubernetes CNIs (Container Network Interfaces) do not support multicast, which DDS requires for SPDP participant discovery. WeaveNet supported multicast but its entire open-source project was end-of-life. The post prescribes Cilium with eBPF as the only viable CNI to run ROS 2 DDS in Kubernetes, revealing that standard cloud deployments require non-standard, specialist infrastructure purely because of DDS's multicast dependency.

**292. [Solved] FastDDS Discovery Server Config Pi5 ROS 2 in Docker**  
2024-02-17 · GitHub iRobotEducation/create3_docs discussion #549 · [link](https://github.com/iRobotEducation/create3_docs/discussions/549)  

A FastDDS Discovery Server running in a Docker container on Raspberry Pi 5 with --network=host fails to serve both the USB-connected Create 3 robot (192.168.186.x subnet) and the WiFi network (10.0.0.x subnet) simultaneously. The server requires three separate listening port configurations for each interface, and the Create 3 robot cannot find the discovery server because container-hosted server locators do not correctly advertise multi-interface reachability.

**293. ROS 2 Fast-DDS Discovery Server with Kubernetes**  
2024-02-14 · ROS Discourse (discourse.openrobotics.org) · [link](https://discourse.openrobotics.org/t/ros-2-fast-dds-discovery-server-with-kubernetes/36086)  

Deploying many ROS 2 applications in Kubernetes is problematic with FastDDS Discovery Server because the system requires static pre-configured discovery server IP addresses, but Kubernetes assigns cluster IPs only at container startup. This makes it impossible to pre-configure the discovery server address before deployment. The post describes a workaround using Kubernetes Headless Services with DNS so containers can dynamically resolve the discovery server address at startup rather than requiring static configuration.

**294. what/how k3s to do what k8s with Weavenet support multicasting for ROS 2?**  
2023-08-01 · GitHub k3s-io/k3s discussion #8088 · [link](https://github.com/k3s-io/k3s/discussions/8088)  

ROS 2 nodes deployed across multiple K3s worker nodes cannot communicate via DDS multicast. Single-node operation works but cross-node multicast fails. WeaveNet was identified as the only CNI supporting multicast for ROS 2, but its installation on K3s does not resolve the cross-node multicast problem. As of mid-2024, WeaveNet was archived and no actively maintained CNI provides drop-in multicast support for ROS 2 on K3s.

**295. How to run ROS across docker containers?**  
2023-01-23 · ROS Answers (answers.ros.org / robotics.stackexchange.com) · [link](https://answers.ros.org/question/411728/)  

Running ROS2 talker and listener in separate Docker containers fails even with --network host and firewall rules allowing multicast on 224.0.0.0/4. Nodes in the same container communicate fine; cross-container communication never works because Docker's default bridge network does not forward DDS multicast UDP discovery packets between containers, and even --network host exhibits problems in some configurations.

**296. ROS2 topics on Docker detected by host but can't subscribe**  
2022-09-16 · GitHub eProsima/Fast-DDS#2956 · [link](https://github.com/eProsima/Fast-DDS/issues/2956)  

On Ubuntu 22.04 ARM64 with ROS 2 Humble running the container as --net=host --ipc=host --pid=host, the host can list all topics and services from the containerized node but ros2 topic echo receives nothing. Fast-DDS default dual-transport (UDPv4 + SHM) causes an asymmetric discovery: the container advertises SHM locators the host cannot reach, so topic listing via discovery works but actual data delivery fails unidirectionally.

**297. Unable to Communicate between Ubuntu 20.04 Container and Ubuntu 20.04 Host**  
2022-08-20 · GitHub ros2/ros2#1318 · [link](https://github.com/ros2/ros2/issues/1318)  

A ROS 2 Foxy talker in a Docker container successfully publishes but the native-host listener receives no messages, even with --net=host --ipc=host. Containerized ROS 2 packages can only communicate with other Docker containers, not with the native host. The issue traces to Fast-DDS selecting shared memory transport based on network interface identity checks that do not correctly reflect container IPC namespace boundaries.

**298. Isolating DDS communication between Docker containers**  
2022-02-16 · RTI Community Forum · [link](https://community.rti.com/forum-topic/isolating-dds-communication-between-docker-containers)  

When hundreds of identical Docker containers share host networking (required for RTI license server access), all containers with the same domain ID exchange unwanted RTPS discovery traffic with each other. DDS port allocation is computed from domain ID and participant index, making it impossible to isolate containers without changing domain IDs. Operators needing to run many identical containers on one host cannot prevent mutual DDS discovery short of per-container domain ID assignment.

**299. Exploring ROS 2 Kubernetes configurations**  
2022-01-01 · Canonical Blog · [link](https://ubuntu.com/blog/exploring-ros-2-kubernetes-configurations)  

Follow-up Canonical article shows that even with Multus MacVLAN interfaces to fix multicast, containers sharing a pod still experience loopback interface collisions where only one container successfully registers its discovery ports. In tests, messages sent from one talker inside a pod never leave that container's network interface and reach the pod network. The article prescribes one-container-per-pod as the only safe rule.

**300. Nodes can't talk to host when running ROS2 in Docker on MacOS**  
2021-03-15 · Robotics StackExchange (ROS Answers archive) q/374042 · [link](https://answers.ros.org/question/374042/nodes-cant-talk-to-host-when-running-ros2-in-docker-on-macos/)  

Docker Desktop for Mac runs containers inside a Linux VM, making --network=host unavailable. ROS 2 containers on macOS cannot discover or exchange messages with nodes on the macOS host regardless of port exposure because DDS multicast is confined to the VM's network namespace. The only viable approaches are container-to-container communication via a bridge network or a VPN-plus-discovery-server workaround.

**301. ROS2 foxy eProsima Fast RTPS communication between Docker ubuntu container and Windows host**  
2021-01-20 · GitHub eProsima/Fast-DDS#1698 · [link](https://github.com/eProsima/Fast-DDS/issues/1698)  

A ROS 2 Foxy talker in a Linux Docker container cannot reach a listener on the Windows host via Fast-DDS. Messages published inside the container are not visible on the Windows side despite explicit initialPeersList pointing to host.docker.internal. The cross-platform Docker-to-host boundary prevents DDS discovery and message exchange.

**302. ROS2 foxy Eclipse Cyclone DDS communication between Docker ubuntu container and Windows host**  
2021-01-20 · GitHub eclipse-cyclonedds/cyclonedds#677 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/677)  

ROS 2 Foxy with rmw_cyclonedds_cpp cannot exchange messages between a Linux Docker container and a Windows 10 host: the talker runs in the container but the Windows listener receives nothing. The problem is that Docker's virtual network interface on macOS/Windows does not participate in DDSI-RTPS multicast with the host OS, making cross-platform container-to-host DDS discovery impossible without explicit unicast peer configuration.

**303. ROS 2 and Kubernetes Basics**  
2020-11-23 · Canonical Blog · [link](https://canonical.com/blog/exploring-ros-2-with-kubernetes)  

Canonical documents that Calico (MicroK8s default CNI) does not forward multicast traffic, so DDS RTPS peer discovery on UDP port 7400 never reaches other pods. Multiple ROS 2 containers in a single pod cannot coexist because they attempt to share the same loopback UDP ports and 'ROS 2 does not provide a method for managing ports used by RTPS'. RTPS also embeds IP addresses in discovery locators, so Kubernetes NAT breaks participant connectivity.

**304. ROS 2 on Kubernetes**  
2020-11-01 · ROS Discourse · [link](https://discourse.openrobotics.org/t/ros-2-on-kubernetes/17182)  

Discussion identifies three fundamental DDS-Kubernetes incompatibilities: (1) port binding conflicts when multiple ROS 2 containers in one pod all try to bind the same RTPS discovery port 7400, causing all but the first container to silently drop traffic; (2) multicast discovery inconsistency across CNI plugins; (3) RTPS discovery locators embed raw IP addresses which break under Kubernetes NAT and load-balancer service IPs, making RTPS incompatible with standard Kubernetes service routing.

**305. Communication issues with many Docker containers**  
2020-03-30 · GitHub ros2/rclpy#530 · [link](https://github.com/ros2/rclpy/issues/530)  

When scaling to roughly 20 rclpy listener containers with TF2, some listeners receive very few or no messages while others work normally. The problem was reproducible across Fast-RTPS, OpenSplice, and CycloneDDS, and only with rclpy subscribers (not rclcpp or rclpy publishers). The TF listener interaction with DDS discovery under containerized multi-subscriber load exposed a scalability failure specific to the Python RMW layer.

**306. listener cannot receive the data after restarting container talker node**  
2020-02-21 · GitHub ros2/rmw_fastrtps#349 · [link](https://github.com/ros2/rmw_fastrtps/issues/349)  

When a Docker container running a Fast-RTPS talker is killed and restarted, the host-side listener stops receiving messages even though the new talker publishes normally. The GUID Fast-DDS assigns is derived from the container's PID, which recycles to the same value after restart. The DDS reader treats the restarted publisher as the same entity and fails to reconnect, causing silent data loss after container restarts.

**307. Multicast fails within same pod, succeeds in different pods**  
2020-01-23 · Discuss Kubernetes · [link](https://discuss.kubernetes.io/t/multicast-fails-within-same-pod-succeeds-in-different-pods/9435)  

UDP multicast for DDS discovery reliably works between separate Kubernetes pods but fails inconsistently when publisher and subscriber containers share a single pod. The consumer container rarely receives any messages in the single-pod configuration despite identical code, yet the problem sometimes disappears transiently. The inconsistency stems from how loopback-based multicast is handled within a shared network namespace versus across pod boundaries under different CNI plugins.

---

## Performance / Latency / CPU Overhead

*19 items*

**308. Publishing large message blocks all callback groups**  
2026-03-03 · GitHub ros2/rmw_cyclonedds#559 · [link](https://github.com/ros2/rmw_cyclonedds/issues/559)  

Publishing a large message (>65 kB) from a timer callback on ROS 2 Kilted with rmw_cyclonedds blocks all subscription callbacks in other callback groups until the publish completes, even in a multithreaded executor. Pending subscription callbacks queue up and fire in a burst after the publish, causing jitter and deadline violations. The executor lock is held for the entire duration of dds_write on large payloads.

**309. Data Latency increases in low frequency data**  
2025-04-12 · GitHub eclipse-cyclonedds/cyclonedds#2256 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/2256)  

Publishing at 1 Hz produces significantly higher end-to-end latency than publishing the same payload at 100 Hz, reproducible with both custom code and ddsperf. The root cause is that CycloneDDS uses an internal aggregation/batching timer that fires on a fixed interval; at low publish rates the data sits in the send queue waiting for the next timer tick, adding tens of milliseconds of unnecessary delay not present at higher rates.

**310. rmw_zenoh binaries for Rolling, Jazzy and Humble**  
2025-01-03 · ROS Discourse discourse.openrobotics.org/t/rmw-zenoh-binaries-for-rolling-jazzy-and-humble/41395 · [link](https://discourse.openrobotics.org/t/rmw-zenoh-binaries-for-rolling-jazzy-and-humble/41395)  

Yadunund announced rmw_zenoh binaries and user testing revealed that with default FastRTPS, a 60 Hz camera stream measured only ~33 Hz via 'ros2 topic hz' appearing choppy in visualization. Bernd_Pfrommer noted this is a well-known limitation he has had to explain to first-time ROS 2 users repeatedly: high-bandwidth DDS streams cannot be reliably measured with rclpy-based tooling because message deserialization overhead throttles the tool itself. rmw_zenoh achieved the full 59.5 Hz.

**311. High latency / Lost messages: Pub/Sub 10B at high pub frequency**  
2024-06-07 · GitHub ros2/rmw_zenoh#198 · [link](https://github.com/ros2/rmw_zenoh/issues/198)  

With rmw_zenoh, publishing 10-byte messages at 2 kHz multi-process, messages arrive with latency spikes exceeding 1699 µs against a 400 µs budget (logged as 'msg 0 late'). The problem disappears in single-process intra-process mode, occurs on both Raspberry Pi 4B and x86, and manifests at lower frequencies too, pointing to inter-process synchronization overhead in the Zenoh RMW bridge.

**312. CycloneDDS Unnecessarily Sends Packets Through Network**  
2024-04-19 · GitHub ros2/rmw_cyclonedds#489 · [link](https://github.com/ros2/rmw_cyclonedds/issues/489)  

When a second subscriber joins a topic on the same local machine, CycloneDDS routes all ~70 Mbps of pub-sub traffic through the physical LAN interface rather than loopback, saturating the home router. The same multi-subscriber scenario with FastRTPS produces near-zero LAN traffic. Root cause is CycloneDDS not detecting that all readers are co-located and choosing loopback transport.

**313. Sometimes heavy CPU load with 60 publishers and one subscriber component**  
2024-03-19 · GitHub ros2/rmw_fastrtps#749 · [link](https://github.com/ros2/rmw_fastrtps/issues/749)  

A setup with 60 publishers in separate processes sending to a single component container running 60 subscribers on ROS 2 Humble sporadically causes the subscriber component to peg CPU. The root cause is tied to SHM shared memory not being cleared on Ctrl+C, leading to stale segment state on restart. The container also emits `failed to send response ... client will not receive response` warnings during lifecycle transitions under load.

**314. Frequency drops with additional subscriber**  
2023-07-04 · GitHub ros2/rmw_cyclonedds#461 · [link](https://github.com/ros2/rmw_cyclonedds/issues/461)  

Publishing 512×384 image messages at 10 Hz with rmw_cyclonedds 1.6.0 on Humble drops from 10 Hz to 7 Hz when a second subscriber is added. The rate degradation is proportional to subscriber count. This is a performance regression absent in other RMW implementations and indicates publisher-side throttling or lock contention in CycloneDDS when managing multiple reader endpoints.

**315. After a long period of operation, the topic communications slow down.**  
2023-03-06 · ROS Answers · [link](https://answers.ros.org/question/413135/)  

On ROS 2 Eloquent, topic reception delays grow from normal to seconds or tens of seconds after several hours of operation. Different topics are affected at different times; once delays appear they do not recover. The issue is consistent with DDS cache growth or network-state accumulation over time but lacks a confirmed root cause, requiring DDS tuning as a workaround.

**316. New Fast DDS Performance Testing**  
2023-01-31 · ROS Discourse · [link](https://discourse.openrobotics.org/t/new-fast-dds-performance-testing/29539)  

Discussion of an eProsima-published benchmark for Fast DDS. Community members contested the results: Cyclone DDS outperformed FastDDS for messages up to 64 kB in all graphs; one developer reported repeated 'Problem reserving CacheChange in reader' errors running FastDDS in production while CycloneDDS 'worked like a charm'; reviewers noted sub-100 µs latency figures reflect CPU wake-up overhead rather than middleware performance, making those numbers 'all but worthless' without CPU-frequency pinning.

**317. Topic message loss under high load on loopback, no multicast**  
2021-10-29 · GitHub ros2/rmw_cyclonedds#350 · [link](https://github.com/ros2/rmw_cyclonedds/issues/350)  

Four publishers sending 1280×960 RGB8 images at 30 Hz to eight subscribers (with RViz + bag recording) see subscriber reception drop to ≤29.5% of expected rate and bag files have missing packets, even with net.core.rmem_max=64 MB tuned. CycloneDDS fine-grained logging shows no retransmission events, yet application-layer drops are clear, suggesting silent loss in the receive path under CPU saturation.

**318. slow publishing and performance for custom messages with large arrays**  
2021-10-14 · GitHub ros2/rmw_cyclonedds#346 · [link](https://github.com/ros2/rmw_cyclonedds/issues/346)  

Publishing ~80 KB messages (5000-element arrays) on ROS 2 Galactic with CycloneDDS achieves only ~30 Hz subscription rate and forces 100% CPU on the publisher despite adequate hardware, while ROS 1 handles 100k-element messages without issue. The bottleneck appears to be serialization overhead in CycloneDDS's CDR path for large payloads, not network bandwidth.

**319. Latency Analysis of ROS2 Multi-Node Systems**  
2021-06-11 · arXiv:2101.02074 · [link](https://arxiv.org/abs/2101.02074)  

Academic measurement study finding that the ROS 2 communication stack adds up to 50 % latency overhead compared to using DDS directly. End-to-end latency in multi-node pipelines strongly depends on the chosen DDS middleware implementation, meaning application developers cannot predict pipeline latency without benchmarking their specific DDS vendor and configuration.

**320. Abnormal jitter in latency test**  
2021-06-08 · GitHub eclipse-cyclonedds/cyclonedds#912 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/912)  

Across multiple hardware platforms, CycloneDDS (master branch) shows extreme max-latency outliers (up to 5761 µs) while median latency stays normal (~40–72 µs). The jitter was not present in older releases, indicating a regression. The pattern appears in both the RoundTrip benchmark and ddsperf, making it a systematic regression in the RTPS stack's scheduling or socket handling rather than an application issue.

**321. High CPU usage when using DDS intra-process communication**  
2021-04-26 · GitHub ros2/rclcpp#1642 · [link](https://github.com/ros2/rclcpp/issues/1642)  

The rclcpp executor calls subscription->create_message() to copy messages even when DDS intra-process communication could pass them without copying. On a Raspberry Pi processing 8 MB messages at 10 Hz, this produces measurable memmove/memset overhead. The unnecessary copy doubles CPU consumption compared to rclcpp's own IPC mechanism.

**322. Reliable publisher/subscriber drops messages at high frequency in Release builds**  
2019-10-28 · GitHub ros2/rmw_fastrtps#338 · [link](https://github.com/ros2/rmw_fastrtps/issues/338)  

A reliable publisher sending 100,000 messages at 1 µs intervals loses messages when the binary is built in Release mode (not Debug). The bug affects Fast-RTPS on Ubuntu 18.04 (Eloquent) but also shows on OpenSplice under similar conditions. The issue persisted for over 40 comments and was reproduced independently by Denso and Apex.AI teams.

**323. FastRTPS drops messages under stress**  
2019-03-06 · GitHub ros2/rmw_fastrtps#258 · [link](https://github.com/ros2/rmw_fastrtps/issues/258)  

On ROS 2 Crystal with Fast-RTPS 1.7.0 and 1.7.1, reliable publisher/subscriber pairs lose messages when the system is under CPU load (e.g., `stress-ng`). The same test produces zero drops with RTI Connext Pro. Root cause was traced to Fast-RTPS internal buffers being starved under high CPU contention.

**324. Security and Performance Considerations in ROS 2: A Balancing Act — 1.55x latency overhead from DDS Security**  
2018-09-24 · arXiv 1809.09566 (Kim et al., CSIRO/Data61) · [link](https://arxiv.org/abs/1809.09566)  

Benchmark study measuring ROS 2 communication latency and throughput with and without DDS Security enabled. DDS Security extensions introduce a mean latency 1.55x higher than the no-security baseline; combining DDS Security with VPN raises mean latency to 4.19x the baseline. The performance hit is more significant than QoS parameter tuning, indicating that enabling SROS2 has a disproportionate cost on real-time robotic applications.

**325. unreasonable latency in a lossy network**  
2018-07-13 · GitHub ros2/ros2#540 · [link](https://github.com/ros2/ros2/issues/540)  

With 30 % simulated packet loss on loopback, FastDDS Reliable QoS causes a subscriber to receive no messages for ~5 seconds and then receive the entire backlog in a burst. The DDS retransmission mechanism queues unacknowledged messages and delivers them all at once on retry success, producing seconds-scale latency spikes rather than a gradual delivery degradation.

**326. Insufficient performance in the QoS demo using default parameters**  
2018-05-24 · GitHub ros2/rmw_fastrtps#202 · [link](https://github.com/ros2/rmw_fastrtps/issues/202)  

The Fast-RTPS image streaming demo (320x240 @ 30 fps, reliable, default QoS) shows "much more stuttering" compared to Connext and OpenSplice; at 640x480 it hangs for several seconds between bursts. The same pipeline runs flawlessly on Connext. Identified early as a systemic performance gap in Fast-RTPS under default ROS 2 QoS settings, driving initial concerns about Fast-RTPS as the default RMW.

---

## Scaling / Fleets / Many Nodes

*16 items*

**327. Fix [rmw_cyclonedds_cpp]: rmw_create_node: failed to create domain, error**  
2026-01-24 · GitHub autowarefoundation/autoware#6759 · [link](https://github.com/autowarefoundation/autoware/issues/6759)  

Autoware's planning simulation demo with ~138 ROS2 nodes and 70 composable components on Jazzy fails with CycloneDDS participant index exhaustion. Multiple component containers crash with 'Failed to find a free participant index for domain 0' and 'rmw_create_node: failed to create domain'. The fix requires adding `<ParticipantIndex>auto</ParticipantIndex><MaxAutoParticipantIndex>1000</MaxAutoParticipantIndex>` to the CycloneDDS XML — a non-obvious configuration not needed for smaller systems.

**328. How many DDS participants are currently used/allowed by RMW?**  
2025-09-09 · ROS Discourse (discourse.openrobotics.org) · [link](https://discourse.openrobotics.org/t/how-many-dds-participants-are-currently-used-allowed-by-rmw/49976)  

Thread uncovers an undocumented default 32-participant cap when using ROS_AUTOMATIC_DISCOVERY_RANGE=LOCALHOST: FastDDS uses maxInitialPeersRange=32 and CycloneDDS uses MaxAutoParticipantIndex=32 by default. When this limit is exceeded, nodes start without any error messages but silently fail to appear in 'ros2 node list', with their topics also invisible. FastDDS fails silently while CycloneDDS at least produces console warnings. The workaround requires either custom DDS XML configuration files or switching to Discovery Server mode.

**329. Discovery Server becomes unresponsive with a large number of participants**  
2025-04-17 · GitHub eProsima/Fast-DDS#5767 · [link](https://github.com/eProsima/Fast-DDS/issues/5767)  

When more than ~75-100 ROS 2 nodes are started nearly simultaneously, the FastDDS Discovery Server consumes over 100% CPU and becomes completely unresponsive — 'ros2 topic list' and 'ros2 topic echo' fail to return any results. Tested with Fast-DDS 2.14.1 on Ubuntu 20.04 with ROS 2 Jazzy using the default UDP/SHM transport configuration. The discovery server, intended as the scalability solution for large deployments, itself becomes a single point of failure at the scale it was designed to support.

**330. Running a subscriber on Humble with publishing from Jazzy/Rolling uses up all memory**  
2025-01-15 · GitHub ros2/rmw_fastrtps#797 · [link](https://github.com/ros2/rmw_fastrtps/issues/797)  

When Humble and Jazzy nodes coexist on the same network (even just running `ros2 topic list` from Jazzy), the Humble machine can exhaust all RAM and crash, caused by unbounded allocation of incoming discovery traffic that the Humble Fast-RTPS cannot parse or discard. Cross-distribution traffic is unsupported but should not cause an OOM crash.

**331. [21654] The discovery_server example is stuck or deadlock when many readers and writers are matching with tcpv4/reliability**  
2024-12-04 · GitHub eProsima/Fast-DDS#5235 · [link](https://github.com/eProsima/Fast-DDS/issues/5235)  

Creating 1000 topics with corresponding reliable DataWriters/DataReaders under a TCP Discovery Server causes Fast-DDS to deadlock during the mass-matching phase. The process cannot be killed with Ctrl+C and CPU usage stalls. The deadlock involves the TCP transport and reliable QoS writer-reader matching under high concurrency.

**332. Scalability issues with large number of nodes**  
2024-03-01 · ROS Discourse discourse.openrobotics.org/t/scalability-issues-with-large-number-of-nodes/36399 · [link](https://discourse.openrobotics.org/t/scalability-issues-with-large-number-of-nodes/36399)  

leander2189 reported a system with 80 nodes and 1,505 total DDS objects (198 clients, 636 services) on CycloneDDS where state machine service clients stop receiving responses or experience >60 second delays. The default Python executor pins a CPU core to 100% and rebuilds callback lists on every iteration. Creating or destroying service clients at runtime forces the executor to regenerate internal state, and static overhead from unused service clients fills the rcl waitset on every wait cycle.

**333. failed to create domain error when spawning many python nodes at once from launch file with cyclonedds**  
2024-01-23 · GitHub ros2/rclpy#1212 · [link](https://github.com/ros2/rclpy/issues/1212)  

Launching ~15–20 Python nodes simultaneously from a launch file on ROS 2 Iron with rmw_cyclonedds_cpp in a Podman container causes 4–5 random nodes to fail with 'Failed to find a free participant index for domain 5'. C++ nodes are unaffected. The participant index pool is exhausted under concurrent Python node initialization, leading to non-deterministic startup failures.

**334. [Iron][nav2] error: Failed to find a free participant index for domain 0**  
2023-06-01 · GitHub ros2/rmw_cyclonedds#458 · [link](https://github.com/ros2/rmw_cyclonedds/issues/458)  

Launching the full Nav2 navigation stack with rmw_cyclonedds_cpp on ROS 2 Iron causes lifecycle_manager, planner_server, controller_server and other nodes to fail with 'Failed to find a free participant index for domain 0'. The default MaxAutoParticipantIndex of 9 is too low for complex stacks; workaround is setting it to 100 in CYCLONEDDS_URI, but the failure is silent and confusing.

**335. The more participants are created, CPU usage and memory consumption are significantly increased even if no message is sent**  
2022-12-20 · GitHub eProsima/Fast-DDS#3163 · [link](https://github.com/eProsima/Fast-DDS/issues/3163)  

On two Raspberry Pi 4 boards with ROS 2 Rolling, simply creating publisher/subscriber connections (no data sent) caused CPU and memory to scale catastrophically: 1 topic = 0.2% CPU / 31 KB; 10 topics = 4% CPU / 268 KB; 30 topics = 99% CPU / 1.6 GB on the subscriber side. Beyond 200 participants, each new node/subscription creation slows to 1-2 seconds.

**336. ROS2 Galactic: Failed to find a free participant index for domain**  
2022-10-31 · ROS Answers archive · [link](https://answers.ros.org/question/408754/)  

Multiple nodes on ROS 2 Galactic with CycloneDDS fail at startup with 'Failed to find a free participant index for domain 0' when concurrent node count exceeds CycloneDDS's default MaxAutoParticipantIndex limit. The error prevents nodes from creating their RMW handles entirely. The workaround requires deep CycloneDDS XML knowledge to set the limit to 1000 and optionally disable multicast.

**337. Experiences with ROS 2 on our robots and what we learned on the way**  
2022-07-26 · ROS Discourse discourse.openrobotics.org/t/experiences-with-ros-2-on-our-robots-and-what-we-learned-on-the-way/26637 · [link](https://discourse.openrobotics.org/t/experiences-with-ros-2-on-our-robots-and-what-we-learned-on-the-way/26637)  

The Hamburg Bit-Bots RoboCup team (Flova) documented production ROS 2 deployment findings. Community responses confirmed that DDS multicast discovery is unlikely to ever reliably scale to 200+ node systems. Out-of-the-box performance for DDS and executors is 'much worse than ROS1'. One experienced developer concluded migration pros do not outweigh cons for existing large codebases. The thread generated significant community agreement from multiple organizations facing identical issues.

**338. Ok to enable multicast on lo interface?**  
2022-07-01 · GitHub eclipse-cyclonedds/cyclonedds#1400 · [link](https://github.com/eclipse-cyclonedds/cyclonedds/issues/1400)  

With ROS_LOCALHOST_ONLY=1, CycloneDDS restricts itself to the loopback (lo) interface, but Ubuntu's lo is not multicast-capable by default. CycloneDDS disables multicast and falls back to unicast discovery, which requires unique participant-index port numbers. The default MaxAutoParticipantIndex of 9 means only 9 ROS 2 processes can run simultaneously; launching a 10th fails with 'Failed to find a free participant index for domain 0'.

**339. Configure maximum DDS participants in a generic way**  
2022-06-23 · GitHub ros2/rmw#324 · [link](https://github.com/ros2/rmw/issues/324)  

Users hit the error "Failed to find a free participant index for domain #" when the default maximum DDS participant count is exceeded, requiring vendor-specific XML configuration files to raise the ceiling. There is no vendor-agnostic environment variable analogous to ROS_LOCALHOST_ONLY to set this limit, forcing new users to know which DDS implementation they are running and how to configure it, which is a major friction point.

**340. Scale Distributed Robot Fleets with Fast DDS and Husarnet**  
2021-12-21 · ROS Discourse (discourse.openrobotics.org) · [link](https://discourse.openrobotics.org/t/scale-distributed-robot-fleets-with-fast-dds-and-husarnet/23545)  

Thread documenting that the standard DDS Simple Discovery mechanism is a fundamental scalability limiter for Internet-connected robot fleets: adding each new robot requires all existing participants to exchange discovery information with it, and discovery storms can overload the network with dozens of robots. The proposed solution combines FastDDS Discovery Server with Husarnet VPN to work across multiple remote physical networks without manual XML DDS configuration. Discovery Server introduces a static IP address requirement that is incompatible with dynamically addressed deployments.

**341. Scalable Distributed Robot Fleet With Fast DDS Discovery Server**  
2021-12-21 · Husarnet blog · [link](https://husarnet.com/blog/ros2-dds-discovery-server)  

Standard DDS multicast/peer-to-peer discovery over WAN/VPN scales as O(n²) in traffic — adding each new robot multiplies discovery packets by the fleet size. The standard simple discovery mechanism also requires modifying XML config on every existing robot whenever a new one joins, then restarting all nodes simultaneously. This makes dynamic fleet expansion without downtime impossible with default DDS configuration.

**342. Reconsidering 1-to-1 mapping of ROS nodes to DDS participants**  
2019-07-31 · GitHub ros2/rmw#180 · [link](https://github.com/ros2/rmw/issues/180)  

The original ROS 2 design creates one DDS participant per ROS node, causing CPU and memory overhead that scales badly with node count; research showed compositing to one participant per process significantly reduces overhead. The architectural change is complicated by DDS-Security operating at participant level, meaning per-node security identities and access control policies are lost if nodes share a participant.

---

## Migration to Zenoh / Alternative Middleware

*7 items*

**343. Performance Comparison of ROS2 Middlewares for Multi-robot Mesh Networks in Planetary Exploration**  
2024-07-03 · arXiv:2407.03091 · [link](https://arxiv.org/abs/2407.03091)  

A comparative study of FastRTPS, CycloneDDS, and Zenoh in dynamic multi-robot mesh network topologies (representing planetary exploration scenarios) found that FastRTPS and CycloneDDS both exhibit higher latency, worse reachability, and higher CPU consumption than Zenoh in dynamic topologies. Zenoh was identified as the superior choice specifically due to DDS's poor adaptability to intermittent and changing network conditions across multiple robots.

**344. Revisit how and when to start the Zenoh router**  
2024-07-01 · GitHub ros2/rmw_zenoh#231 · [link](https://github.com/ros2/rmw_zenoh/issues/231)  

rmw_zenoh requires a running zenohd router daemon before any ROS 2 nodes can discover each other, because multicast scouting is disabled by default and peer gossip only works through the router. Unlike DDS-based RMWs that work out-of-the-box, this creates a roscore-like bootstrapping requirement. The issue catalogues the dilemma: automatic router spawning introduces race conditions on multi-node startup, cross-platform issues on Windows, and unclear lifetime management.

**345. Zenoh Experimental Support Lands in ROS 2**  
2024-06-12 · ZettaScale Technologies news · [link](https://www.zettascale.tech/news/zenoh-experimental-support-lands-in-ros-2/)  

ZettaScale announcement that Zenoh is now included experimentally in ROS 2 Jazzy, driven by two documented DDS failure modes: corporate networks that block the UDP multicast DDS requires, and the inability of DDS to support globe-spanning remote robotics without proprietary per-vendor extensions. OSRF's investigation confirmed Zenoh met key requirements for current and future robotics applications that DDS could not.

**346. ROS 2 Communication Stack: Exploring the Improvements Brought by Zenoh**  
2024-05-01 · Electronic Design / ZettaScale · [link](https://www.electronicdesign.com/technologies/communications/article/55039208/zettascale-ros-2-communication-stack-exploring-the-improvements-brought-by-zenoh)  

Documents the concrete DDS problems driving Zenoh adoption: multicast UDP discovery blocked or unreliable in corporate and cloud deployments; reliability protocol creates fully-connected participant graph causing quadratic traffic growth; large messages (images, point clouds) systematically dropped or slow; application startup failures under DDS load that disappear with Zenoh. OSRF community survey found users 'overwhelmingly favoured Zenoh' over other alternatives, and Zenoh became an official RMW in ROS2 Kilted Kaiju.

**347. Eclipse Zenoh Selected as the Alternate ROS 2 Middleware**  
2023-10-30 · Eclipse Foundation newsroom · [link](https://newsroom.eclipse.org/eclipse-newsletter/2023/october/eclipse-zenoh-selected-alternate-ros-2-middleware)  

Eclipse Foundation announcement of Zenoh as the officially selected ROS 2 alternative middleware, citing that DDS implementations failed in corporate networks where multicast UDP is blocked, and could not support globe-spanning remote robotics deployments. OSRF's community survey showed Zenoh was the top user-recommended alternative. Zenoh entered ROS 2 as an experimental RMW with the Jazzy release in May 2024.

**348. New Zenoh bridge for ROS 2**  
2023-10-16 · ROS Discourse (discourse.openrobotics.org) · [link](https://discourse.openrobotics.org/t/new-zenoh-bridge-for-ros-2/34163)  

eProsima and ZettaScale announce a new zenoh-bridge-ros2dds, explicitly motivated by 'issues occurring with DDS communication over wireless networks or at large scale' that the earlier zenoh-bridge-dds had already helped users overcome. The bridge translates between DDS RTPS wire format and Zenoh, allowing ROS 2 nodes to bypass DDS discovery entirely for cross-network or large-scale scenarios while maintaining compatibility with DDS-based ROS 2 nodes on the same host.

**349. ROS 2 Alternative middleware report**  
2023-09-27 · ROS Discourse (discourse.openrobotics.org) · [link](https://discourse.openrobotics.org/t/ros-2-alternative-middleware-report/33771)  

Official OSRF report documenting why DDS is insufficient as the sole ROS 2 middleware: community members reported 'network-wide crashes from DDS multicast packet storms' in office and customer environments, DDS 'didn't work out of the box on their networks' (especially academic and corporate managed networks), and it required 'expert application-specific DDS configuration' to function reliably. Zenoh was selected as the alternative middleware as it was overwhelmingly the most-recommended option by surveyed users, with DDS's multicast dependency on UDP being explicitly cited as the blocker for commercial deployments.

---
