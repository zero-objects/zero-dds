# RPM-Spec for ZeroDDS — combined source-spec, splits into sub-packages.
# Spec: zerodds-deployment-1.0.md §2.2.2 (RHEL/Fedora) and §2.1 (FHS layout).
%global _hardened_build 1
%global debug_package %{nil}

# Version fields are NOT written down here. .github/workflows/publish-rpm.yml
# derives them from the release tag and passes them in:
#   rpmbuild --define "zd_version 1.0.0" \
#            --define "zd_release 0.rc7" \
#            --define "zd_prerel  rc.7"      (omitted for a final release)
# The defaults below only apply to a manual local rpmbuild and describe a
# final release, so they can never carry a stale RC number.
%{!?zd_version:%global zd_version 1.0.0}
%{!?zd_release:%global zd_release 1}
# Upstream archive + source-directory name: "1.0.0" or "1.0.0-rc.7".
# zd_prerel is intentionally left undefined by default — %{?...} then
# expands to nothing instead of a dangling "-".
%global zd_fullver %{zd_version}%{?zd_prerel:-%{zd_prerel}}

Name:           zerodds
Version:        %{zd_version}
Release:        %{zd_release}%{?dist}
Summary:        ZeroDDS — Rust-native DDS implementation with bridges
License:        Apache-2.0
URL:            https://zerodds.org
Source0:        https://github.com/zero-objects/zero-dds/archive/v%{zd_fullver}/zerodds-%{zd_fullver}.tar.gz

# BuildRequires: cargo >= 1.88 (kommt via rustup im rpm-job)
# BuildRequires: rust >= 1.88 (kommt via rustup im rpm-job)
BuildRequires:  pkgconfig
BuildRequires:  openssl-devel
BuildRequires:  systemd-rpm-macros
# The install section builds the SDK (C and C++ headers, static library)
# with CMake.
BuildRequires:  cmake
BuildRequires:  gcc-c++

Requires(pre):  shadow-utils
Requires(post): systemd
Requires(preun): systemd
Requires(postun): systemd

%description
ZeroDDS is a pure-Rust implementation of the OMG Data-Distribution-Service
specifications (DDS 1.4, RTPS 2.5, XTypes 1.3, DDS-Security 1.2, plus
PSMs and bridges). This top-level package is meta and pulls in
zerodds-core; install zerodds-* sub-packages for individual bridges or
the CLI tool-set.

# ---------------------------------------------------------------------------
# Sub-packages.
# ---------------------------------------------------------------------------
%package core
Summary: ZeroDDS shared library and runtime metadata
Requires: openssl-libs

%description core
Provides libzerodds.so (ABI v1), runtime config directories, the
zerodds system-user and tmpfiles fragments. Mandatory dependency for
every zerodds-* sub-package.

%package cli
Summary: ZeroDDS administration and diagnostic command-line tools
Requires: zerodds-core = %{version}-%{release}

%description cli
Ships the 17 CLI tools listed in zerodds-deployment-1.0.md §1.3
(zerodds-admin, zerodds-bench, zerodds-idlc, zerodds-spy, etc.).

%package devel
Summary: ZeroDDS C/C++ development files
Requires: zerodds-core = %{version}-%{release}
# The SDK's zeroddsConfig.cmake references /usr/bin/zerodds-idlc, owned by -cli.
Requires: zerodds-cli = %{version}-%{release}

%description devel
Public C/C++ headers, static library, pkg-config descriptor, and the
find_package(zerodds CONFIG) CMake package (config + version + IDLC generator
module) needed to build native code against libzerodds.
Cross-Ref: zerodds-ffi-loader-1.0.

%package ws-bridge
Summary: ZeroDDS DDS to WebSocket bridge daemon
Requires: zerodds-core = %{version}-%{release}

%description ws-bridge
Spec: zerodds-ws-bridge-1.0 §11.

%package mqtt-bridge
Summary: ZeroDDS DDS to MQTT 5 bridge daemon
Requires: zerodds-core = %{version}-%{release}

%description mqtt-bridge
Spec: zerodds-mqtt-bridge-1.0 §11.

%package coap-bridge
Summary: ZeroDDS DDS to CoAP bridge daemon
Requires: zerodds-core = %{version}-%{release}

%description coap-bridge
Spec: zerodds-coap-bridge-1.0 §11.

%package amqp-bridge
Summary: ZeroDDS DDS to AMQP 1.0 bridge daemon
Requires: zerodds-core = %{version}-%{release}

%description amqp-bridge
Spec: zerodds-amqp-bridge-daemon-1.0 §11.

%package grpc-bridge
Summary: ZeroDDS DDS to gRPC bridge daemon
Requires: zerodds-core = %{version}-%{release}

%description grpc-bridge
Spec: zerodds-grpc-bridge-1.0 §11.

%package corba-bridge
Summary: ZeroDDS DDS to CORBA GIOP/IIOP bridge daemon
Requires: zerodds-core = %{version}-%{release}

%description corba-bridge
Spec: zerodds-corba-bridge-1.0 §11.

%package zenoh-bridge
Summary: ZeroDDS DDS to Eclipse Zenoh bridge daemon
Requires: zerodds-core = %{version}-%{release}

%description zenoh-bridge
Bidirectional bridge between DDS topics and Zenoh key expressions,
wire-compatible with Eclipse Zenoh's own DDS bridge (zenoh-plugin-dds /
zenoh-bridge-dds). See zerodds-zenoh-bridged(1) and
zerodds-zenoh-bridged.yaml(5).

%package opcua-bridge
Summary: ZeroDDS OMG DDS-OPCUA Gateway 1.0 daemon
Requires: zerodds-core = %{version}-%{release}

%description opcua-bridge
Spec: zerodds-opcua-bridge-1.0 §11.

%package webdds-bridge
Summary: ZeroDDS OMG DDS-WEB 1.0 gateway daemon (REST + SOAP)
Requires: zerodds-core = %{version}-%{release}

%description webdds-bridge
Exposes DDS applications, participants, topics, publishers,
subscribers, data writers and data readers as HTTP resources per OMG
Web-Enabled DDS 1.0: the REST PSM under /dds/rest1 and the SIMPLE-
WSDL-SOAP PSM under /soap, from one process. See
zerodds-webdds-bridged(1) and zerodds-webdds-bridged.yaml(5).

%package ros2
Summary: ZeroDDS ROS 2 diagnostics (zerodds-ros2-shim)
Requires: zerodds-core = %{version}-%{release}

%description ros2
zerodds-ros2-shim, which prints what rmw_zerodds_cpp reads from the runtime
configuration, its man pages and the ros2 drop-in template. The RMW library
itself is not here: it ships per ROS distro as
ros-<distro>-rmw-zerodds-cpp, built from the ament package rmw_zerodds_cpp
(packaging/linux/deb/build-rmw-deb.sh).
Spec: zerodds-ros2-bridge-1.0 §11.

%package router
Summary: ZeroDDS in-DDS routing service daemon
Requires: zerodds-core = %{version}-%{release}

%description router
Spec: zerodds-deployment-1.0 §2.3. DDS-to-DDS forwarding daemon
(see zerodds-router(1)) with per-route content filter and field transform.

%package durability
Summary: ZeroDDS Durability-Service daemon
Requires: zerodds-core = %{version}-%{release}

%description durability
zerodds-durability-svc keeps TRANSIENT and PERSISTENT samples for late
joiners, one daemon per domain, with the sqlite or file store (see
zerodds-durability-svc(1)). The DuckDB store is an opt-in build feature.

%package naming
Summary: ZeroDDS OMG CosNaming 1.3 NameService daemon
Requires: zerodds-core = %{version}-%{release}

%description naming
zerodds-naming-svc: NamingContext + NamingContextExt + BindingIterator
over IIOP/SSLIOP, optionally CSIv2 GSSUP, persisted to a file (see
zerodds-naming-svc(1)).

%package xrce-agent
Summary: ZeroDDS DDS-XRCE agent daemon
Requires: zerodds-core = %{version}-%{release}

%description xrce-agent
zerodds-xrce-agentd represents an eProsima-compatible DDS-XRCE Micro
Client in the DDS data space over UDP/TCP/serial (see
zerodds-xrce-agentd(1)). Implements OMG DDS-XRCE 1.0.

# ---------------------------------------------------------------------------
# Build orchestration.
# ---------------------------------------------------------------------------
%prep
%autosetup -n zerodds-%{zd_fullver}

%build
# zerodds-foreign-typeobject-probe is an interop test tool, no package ships it;
# the install section takes every target/release/zerodds-* binary. The naming and XRCE
# daemons need their package's `daemon` feature, which is not a default feature.
cargo build --release --workspace --exclude zerodds-durability-store-lakehouse --exclude iceoryx2-oracle-consumer --exclude zerodds-endpoint-codegen --exclude zerodds-endpoint-golden --exclude zerodds-xrce-agent-demo --exclude zerodds-foreign-typeobject-probe --features zerodds-corba-cosnaming/daemon,zerodds-xrce-agent/daemon

%install
install -d %{buildroot}%{_bindir}
install -d %{buildroot}%{_libdir}
install -d %{buildroot}%{_includedir}
install -d %{buildroot}%{_libdir}/pkgconfig
install -d %{buildroot}%{_unitdir}
install -d %{buildroot}%{_tmpfilesdir}
install -d %{buildroot}%{_sysusersdir}
install -d %{buildroot}%{_sysconfdir}/zerodds
install -d %{buildroot}%{_sysconfdir}/zerodds/certs
install -d %{buildroot}%{_localstatedir}/log/zerodds
install -d %{buildroot}%{_localstatedir}/lib/zerodds
install -d %{buildroot}%{_mandir}/man1
install -d %{buildroot}%{_mandir}/man5
install -d %{buildroot}%{_datadir}/zerodds/configs

# Bridge-Daemons + CLIs.
for bin in target/release/zerodds-*; do
    [ -x "$bin" ] || continue
    install -m 0755 "$bin" %{buildroot}%{_bindir}/
done

# libzerodds — the versioned runtime shared object goes in -core.
install -m 0755 target/release/libzerodds.so %{buildroot}%{_libdir}/libzerodds.so.1.0.0
ln -sf libzerodds.so.1.0.0 %{buildroot}%{_libdir}/libzerodds.so.1
ln -sf libzerodds.so.1     %{buildroot}%{_libdir}/libzerodds.so

# -devel files: the single SDK install staging tree, NO hand-copied header list
# (spec P0-4). `cmake --install` emits C + C++ headers, the static library,
# zeroddsConfig.cmake + ConfigVersion, the IDLC generator module and zerodds.pc.
# We install into a private staging prefix, then relocate the SDK bits into the
# buildroot; the zerodds-idlc host tool is skipped here because -cli already
# ships /usr/bin/zerodds-idlc (config Requires: zerodds-cli). Static lib: the
# runtime .so above is the -core artifact; -devel provides the static SDK.
%global zdw_stage %{_builddir}/zdw-stage
cmake -S . -B %{_builddir}/zdw-build \
    -DCMAKE_BUILD_TYPE=Release \
    -DBUILD_SHARED_LIBS=OFF \
    -DZERODDS_BUILD_CPP=ON \
    -DZERODDS_BUILD_IDLC=ON \
    -DZERODDS_BUILD_EXAMPLES=OFF \
    -DZERODDS_BUILD_TESTS=OFF \
    -DCMAKE_INSTALL_PREFIX=%{_prefix} \
    -DCMAKE_INSTALL_LIBDIR=%{_lib}
cmake --build %{_builddir}/zdw-build
DESTDIR=%{zdw_stage} cmake --install %{_builddir}/zdw-build
# Relocate SDK payload into the buildroot (headers, cmake package, pkgconfig,
# static lib). Drop the idlc host tool copy — -cli owns it.
rm -f %{zdw_stage}%{_bindir}/zerodds-idlc
cp -a %{zdw_stage}%{_prefix}/. %{buildroot}%{_prefix}/

# systemd + tmpfiles + sysusers.
for u in packaging/linux/systemd/zerodds-*.service; do
    install -m 0644 "$u" %{buildroot}%{_unitdir}/
done
install -m 0644 packaging/linux/systemd/zerodds-tmpfiles.conf %{buildroot}%{_tmpfilesdir}/zerodds.conf
install -m 0644 packaging/linux/systemd/zerodds-sysusers.conf %{buildroot}%{_sysusersdir}/zerodds.conf

# Default-Configs als example (*.yaml.example for the bridges, plus the
# routing service's *.json.example — glob on *.example covers both).
for c in packaging/linux/configs/*.example; do
    install -m 0640 "$c" %{buildroot}%{_datadir}/zerodds/configs/
done

# man-pages — optional: a glob that matches nothing stays unexpanded and is
# skipped. POSIX sh; the install section does not run under bash everywhere.
for m in man/man1/*.1; do
    [ -e "$m" ] || continue
    install -m 0644 "$m" %{buildroot}%{_mandir}/man1/
done
for m in man/man5/*.5; do
    [ -e "$m" ] || continue
    install -m 0644 "$m" %{buildroot}%{_mandir}/man5/
done

# ---------------------------------------------------------------------------
# Pre/Post Scriptlets.
# ---------------------------------------------------------------------------
%pre core
getent group zerodds >/dev/null || groupadd -r zerodds
getent passwd zerodds >/dev/null || \
    useradd -r -g zerodds -d /var/lib/zerodds -s /sbin/nologin \
            -c "ZeroDDS service user" zerodds
exit 0

%post core
%tmpfiles_create %{_tmpfilesdir}/zerodds.conf
/sbin/ldconfig

%postun core -p /sbin/ldconfig

%post   ws-bridge        %systemd_post   zerodds-ws-bridged.service
%preun  ws-bridge        %systemd_preun  zerodds-ws-bridged.service
%postun ws-bridge        %systemd_postun_with_restart zerodds-ws-bridged.service

%post   mqtt-bridge      %systemd_post   zerodds-mqtt-bridged.service
%preun  mqtt-bridge      %systemd_preun  zerodds-mqtt-bridged.service
%postun mqtt-bridge      %systemd_postun_with_restart zerodds-mqtt-bridged.service

%post   coap-bridge      %systemd_post   zerodds-coap-bridged.service
%preun  coap-bridge      %systemd_preun  zerodds-coap-bridged.service
%postun coap-bridge      %systemd_postun_with_restart zerodds-coap-bridged.service

%post   amqp-bridge      %systemd_post   zerodds-amqp-bridged.service
%preun  amqp-bridge      %systemd_preun  zerodds-amqp-bridged.service
%postun amqp-bridge      %systemd_postun_with_restart zerodds-amqp-bridged.service

%post   grpc-bridge      %systemd_post   zerodds-grpc-bridged.service
%preun  grpc-bridge      %systemd_preun  zerodds-grpc-bridged.service
%postun grpc-bridge      %systemd_postun_with_restart zerodds-grpc-bridged.service

%post   corba-bridge     %systemd_post   zerodds-corba-bridged.service
%preun  corba-bridge     %systemd_preun  zerodds-corba-bridged.service
%postun corba-bridge     %systemd_postun_with_restart zerodds-corba-bridged.service

%post   zenoh-bridge     %systemd_post   zerodds-zenoh-bridged.service
%preun  zenoh-bridge     %systemd_preun  zerodds-zenoh-bridged.service
%postun zenoh-bridge     %systemd_postun_with_restart zerodds-zenoh-bridged.service

%post   opcua-bridge     %systemd_post   zerodds-opcua-bridged.service
%preun  opcua-bridge     %systemd_preun  zerodds-opcua-bridged.service
%postun opcua-bridge     %systemd_postun_with_restart zerodds-opcua-bridged.service
%post   webdds-bridge    %systemd_post   zerodds-webdds-bridged.service
%preun  webdds-bridge    %systemd_preun  zerodds-webdds-bridged.service
%postun webdds-bridge    %systemd_postun_with_restart zerodds-webdds-bridged.service

%post   router           %systemd_post   zerodds-router.service
%preun  router           %systemd_preun  zerodds-router.service
%postun router           %systemd_postun_with_restart zerodds-router.service

%post   durability       %systemd_post   zerodds-durability-svc.service
%preun  durability       %systemd_preun  zerodds-durability-svc.service
%postun durability       %systemd_postun_with_restart zerodds-durability-svc.service

%post   naming           %systemd_post   zerodds-naming-svc.service
%preun  naming           %systemd_preun  zerodds-naming-svc.service
%postun naming           %systemd_postun_with_restart zerodds-naming-svc.service

%post   xrce-agent       %systemd_post   zerodds-xrce-agentd.service
%preun  xrce-agent       %systemd_preun  zerodds-xrce-agentd.service
%postun xrce-agent       %systemd_postun_with_restart zerodds-xrce-agentd.service

# ---------------------------------------------------------------------------
# File-Lists.
# ---------------------------------------------------------------------------
%files core
%license LICENSE
%doc README.md CHANGELOG.md
%{_libdir}/libzerodds.so.1.0.0
%{_libdir}/libzerodds.so.1
%{_libdir}/libzerodds.so
%{_tmpfilesdir}/zerodds.conf
%{_sysusersdir}/zerodds.conf
%dir %attr(0750,zerodds,zerodds) %{_sysconfdir}/zerodds
%dir %attr(0700,zerodds,zerodds) %{_sysconfdir}/zerodds/certs
%dir %attr(0750,zerodds,zerodds) %{_localstatedir}/log/zerodds
%dir %attr(0750,zerodds,zerodds) %{_localstatedir}/lib/zerodds
%dir %{_datadir}/zerodds/configs
%{_datadir}/zerodds/configs/zerodds.yaml.example
%{_mandir}/man5/zerodds.yaml.5*

%files cli
# Bin-list auf existing workspace-bins beschraenkt (siehe cargo metadata).
%{_bindir}/zerodds-admin
%{_bindir}/zerodds-idlc
%{_bindir}/zerodds-replay
%{_bindir}/zerodds-perf
%{_bindir}/zerodds-xmlc
%{_bindir}/zerodds-traceability
%{_bindir}/zerodds-lint
%{_bindir}/zerodds-chaos
%{_bindir}/zerodds-dashboard
%{_bindir}/zerodds-recorder-bridge
%{_bindir}/zerodds-record
%{_bindir}/zerodds-bench
%{_bindir}/zerodds-monitor
%{_bindir}/zerodds-spy
%{_bindir}/zerodds-snitch
%{_bindir}/zerodds-pcap
%{_bindir}/zerodds-mq
%{_bindir}/zerodds-secure-permissions
%{_mandir}/man1/zerodds-admin.1*
%{_mandir}/man1/zerodds-idlc.1*
%{_mandir}/man1/zerodds-record.1*
%{_mandir}/man1/zerodds-replay.1*
%{_mandir}/man1/zerodds-bench.1*
%{_mandir}/man1/zerodds-monitor.1*
%{_mandir}/man1/zerodds-spy.1*
%{_mandir}/man1/zerodds-snitch.1*
%{_mandir}/man1/zerodds-pcap.1*
%{_mandir}/man1/zerodds-mq.1*
%{_mandir}/man1/zerodds-xmlc.1*

%files devel
# All from the single `cmake --install` SDK staging tree (spec P0-4).
%{_includedir}/zerodds.h
%{_includedir}/zerodds_xcdr2.h
%{_includedir}/dds/
%{_includedir}/zerodds/
%{_libdir}/libzerodds.a
%{_libdir}/libzerodds-wire-cpp.a
%{_libdir}/cmake/zerodds/
%{_libdir}/pkgconfig/zerodds.pc

%files ws-bridge
%{_bindir}/zerodds-ws-bridged
%{_unitdir}/zerodds-ws-bridged.service
%{_datadir}/zerodds/configs/ws-bridged.yaml.example
%{_mandir}/man1/zerodds-ws-bridged.1*
%{_mandir}/man5/zerodds-ws-bridged.yaml.5*

%files mqtt-bridge
%{_bindir}/zerodds-mqtt-bridged
%{_unitdir}/zerodds-mqtt-bridged.service
%{_datadir}/zerodds/configs/mqtt-bridged.yaml.example
%{_mandir}/man1/zerodds-mqtt-bridged.1*
%{_mandir}/man5/zerodds-mqtt-bridged.yaml.5*

%files coap-bridge
%{_bindir}/zerodds-coap-bridged
%{_unitdir}/zerodds-coap-bridged.service
%{_datadir}/zerodds/configs/coap-bridged.yaml.example
%{_mandir}/man1/zerodds-coap-bridged.1*
%{_mandir}/man5/zerodds-coap-bridged.yaml.5*

%files amqp-bridge
%{_bindir}/zerodds-amqp-bridged
%{_unitdir}/zerodds-amqp-bridged.service
%{_datadir}/zerodds/configs/amqp-bridged.yaml.example
%{_mandir}/man1/zerodds-amqp-bridged.1*
%{_mandir}/man5/zerodds-amqp-bridged.yaml.5*

%files grpc-bridge
%{_bindir}/zerodds-grpc-bridged
%{_unitdir}/zerodds-grpc-bridged.service
%{_datadir}/zerodds/configs/grpc-bridged.yaml.example
%{_mandir}/man1/zerodds-grpc-bridged.1*
%{_mandir}/man5/zerodds-grpc-bridged.yaml.5*

%files corba-bridge
%{_bindir}/zerodds-corba-bridged
%{_unitdir}/zerodds-corba-bridged.service
%{_datadir}/zerodds/configs/corba-bridged.yaml.example
%{_mandir}/man1/zerodds-corba-bridged.1*
%{_mandir}/man5/zerodds-corba-bridged.yaml.5*

%files zenoh-bridge
%{_bindir}/zerodds-zenoh-bridged
%{_unitdir}/zerodds-zenoh-bridged.service
%{_datadir}/zerodds/configs/zenoh-bridged.yaml.example
%{_mandir}/man1/zerodds-zenoh-bridged.1*
%{_mandir}/man5/zerodds-zenoh-bridged.yaml.5*

%files opcua-bridge
%{_bindir}/zerodds-opcua-bridged
%{_unitdir}/zerodds-opcua-bridged.service
%{_datadir}/zerodds/configs/opcua-bridged.yaml.example
%{_mandir}/man1/zerodds-opcua-bridged.1*
%{_mandir}/man5/zerodds-opcua-bridged.yaml.5*

%files webdds-bridge
%{_bindir}/zerodds-webdds-bridged
%{_unitdir}/zerodds-webdds-bridged.service
%{_datadir}/zerodds/configs/webdds-bridged.yaml.example
%{_mandir}/man1/zerodds-webdds-bridged.1*
%{_mandir}/man5/zerodds-webdds-bridged.yaml.5*

%files ros2
# The diagnostics CLI of the RMW; no service: rcl loads the RMW into each node.
%{_bindir}/zerodds-ros2-shim
%{_datadir}/zerodds/configs/ros2.yaml.example
%{_mandir}/man1/zerodds-ros2-shim.1*
%{_mandir}/man5/zerodds-ros2.yaml.5*

%files router
%{_bindir}/zerodds-router
%{_unitdir}/zerodds-router.service
%{_datadir}/zerodds/configs/router.json.example
%{_mandir}/man1/zerodds-router.1*

%files durability
%{_bindir}/zerodds-durability-svc
%{_unitdir}/zerodds-durability-svc.service
%{_datadir}/zerodds/configs/durability-svc.yaml.example
%{_mandir}/man1/zerodds-durability-svc.1*
%{_mandir}/man5/zerodds-durability-svc.yaml.5*

%files naming
%{_bindir}/zerodds-naming-svc
%{_unitdir}/zerodds-naming-svc.service
%{_datadir}/zerodds/configs/naming-svc.yaml.example
%{_mandir}/man1/zerodds-naming-svc.1*
%{_mandir}/man5/zerodds-naming-svc.yaml.5*

%files xrce-agent
%{_bindir}/zerodds-xrce-agentd
%{_unitdir}/zerodds-xrce-agentd.service
%{_datadir}/zerodds/configs/xrce-agentd.yaml.example
%{_mandir}/man1/zerodds-xrce-agentd.1*
%{_mandir}/man5/zerodds-xrce-agentd.yaml.5*

%changelog
* Wed May 06 2026 ZeroDDS Contributors <release@zerodds.org> - 1.0.0-0.rc1
- Initial RC1 RPM split-package layout per zerodds-deployment-1.0 §2.2.2.
