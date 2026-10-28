#!/usr/bin/env python3
# packaging/linux/deb/rmw-service-info-check.py
# Service endpoint info through an installed ros-<distro>-rmw-zerodds-cpp
# package (Rolling: rmw_get_servers_info_by_service and
# rmw_get_clients_info_by_service).
#
# A child process is a node whose parameter services include
# /zerodds_svc_server/get_parameters; the parent process creates a client for
# that service and asks rcl for its servers and clients. It exits 0 only when
# it sees one server on the child's node and one client on its own, both with
# the service type rcl_interfaces/srv/GetParameters. The caller sets
# RMW_IMPLEMENTATION=rmw_zerodds_cpp and ROS_DOMAIN_ID.
#
# Usage: rmw-service-info-check.py
import subprocess
import sys
import time

import rclpy
from rcl_interfaces.srv import GetParameters

SERVER_NODE = "zerodds_svc_server"
CLIENT_NODE = "zerodds_svc_client"
SERVICE = f"/{SERVER_NODE}/get_parameters"
SERVICE_TYPE = "rcl_interfaces/srv/GetParameters"
DISCOVERY_S = 15.0
SERVER_S = 30.0


def run_server() -> int:
    rclpy.init()
    node = rclpy.create_node(SERVER_NODE)
    deadline = time.monotonic() + SERVER_S
    while time.monotonic() < deadline:
        rclpy.spin_once(node, timeout_sec=0.2)
    node.destroy_node()
    rclpy.shutdown()
    return 0


def run_check() -> int:
    server = subprocess.Popen([sys.executable, __file__, "server"])
    rclpy.init()
    node = rclpy.create_node(CLIENT_NODE)
    node.create_client(GetParameters, SERVICE)
    servers, clients = [], []
    deadline = time.monotonic() + DISCOVERY_S
    while time.monotonic() < deadline:
        servers = node.get_servers_info_by_service(SERVICE)
        clients = node.get_clients_info_by_service(SERVICE)
        if servers and clients:
            break
        time.sleep(0.2)
    server.kill()
    server.wait()
    for kind, infos in (("server", servers), ("client", clients)):
        for info in infos:
            print(kind, info)
    node.destroy_node()
    rclpy.shutdown()
    ok = (
        len(servers) == 1
        and len(clients) == 1
        and servers[0].node_name == SERVER_NODE
        and clients[0].node_name == CLIENT_NODE
        and servers[0].service_type == SERVICE_TYPE
        and clients[0].service_type == SERVICE_TYPE
    )
    print("service-info", "ok" if ok else "FAIL", len(servers), len(clients))
    return 0 if ok else 1


def main(argv) -> int:
    if len(argv) == 2 and argv[1] == "server":
        return run_server()
    if len(argv) != 1:
        print("usage: rmw-service-info-check.py", file=sys.stderr)
        return 2
    return run_check()


if __name__ == "__main__":
    sys.exit(main(sys.argv))
