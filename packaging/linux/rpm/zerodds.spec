# RPM-Spec for ZeroDDS — combined source-spec, splits into sub-packages.
# Spec: zerodds-deployment-1.0.md §2.2.2 (RHEL/Fedora) and §2.1 (FHS layout).
%global _hardened_build 1
%global debug_package %{nil}

Name:           zerodds
Version:        1.0.0
Release:        0.rc1%{?dist}
Summary:        ZeroDDS — Rust-native DDS implementation with bridges
License:        Apache-2.0
URL:            https://zerodds.org
Source0:        https://github.com/zero-objects/zerodds/archive/v%{version}-rc.1/zerodds-%{version}-rc.1.tar.gz

# BuildRequires: cargo >= 1.88 (kommt via rustup im rpm-job)
# BuildRequires: rust >= 1.88 (kommt via rustup im rpm-job)
BuildRequires:  pkgconfig
BuildRequires:  openssl-devel
BuildRequires:  systemd-rpm-macros

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

%description devel
Public C-headers, static library, and pkg-config descriptor needed to
build native code against libzerodds. Cross-Ref: zerodds-ffi-loader-1.0.

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

%package ros2
Summary: ZeroDDS ROS-2 RMW shim and diagnostics
Requires: zerodds-core = %{version}-%{release}

%description ros2
Spec: zerodds-ros2-bridge-1.0 §11.

# ---------------------------------------------------------------------------
# Build orchestration.
# ---------------------------------------------------------------------------
%prep
%autosetup -n zerodds-%{version}-rc.1

%build
cargo build --release --workspace

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

# libzerodds.
install -m 0755 target/release/libzerodds.so %{buildroot}%{_libdir}/libzerodds.so.1.0.0
ln -sf libzerodds.so.1.0.0 %{buildroot}%{_libdir}/libzerodds.so.1
ln -sf libzerodds.so.1     %{buildroot}%{_libdir}/libzerodds.so
install -m 0644 crates/zerodds-c-api/include/zerodds.h %{buildroot}%{_includedir}/zerodds.h
install -m 0644 packaging/linux/rpm/zerodds.pc          %{buildroot}%{_libdir}/pkgconfig/zerodds.pc

# systemd + tmpfiles + sysusers.
for u in packaging/linux/systemd/zerodds-*.service; do
    install -m 0644 "$u" %{buildroot}%{_unitdir}/
done
install -m 0644 packaging/linux/systemd/zerodds-tmpfiles.conf %{buildroot}%{_tmpfilesdir}/zerodds.conf
install -m 0644 packaging/linux/systemd/zerodds-sysusers.conf %{buildroot}%{_sysusersdir}/zerodds.conf

# Default-Configs als example.
for c in packaging/linux/configs/*.yaml.example; do
    install -m 0640 "$c" %{buildroot}%{_datadir}/zerodds/configs/
done

# man-pages — optional (nullglob: skip when no files match).
shopt -s nullglob
for m in man/man1/*.1; do
    install -m 0644 "$m" %{buildroot}%{_mandir}/man1/
done
for m in man/man5/*.5; do
    install -m 0644 "$m" %{buildroot}%{_mandir}/man5/
done
shopt -u nullglob

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

%post   ros2             %systemd_post   zerodds-ros2-shim.service
%preun  ros2             %systemd_preun  zerodds-ros2-shim.service
%postun ros2             %systemd_postun_with_restart zerodds-ros2-shim.service

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

%files cli
%{_bindir}/zerodds-admin
%{_bindir}/zerodds-bench
%{_bindir}/zerodds-bench-suite
%{_bindir}/zerodds-idlc
%{_bindir}/zerodds-spy
%{_bindir}/zerodds-record
%{_bindir}/zerodds-replay
%{_bindir}/zerodds-snitch
%{_bindir}/zerodds-monitor
%{_bindir}/zerodds-mq
%{_bindir}/zerodds-pcap
%{_bindir}/zerodds-perf
%{_bindir}/zerodds-secure-keygen
%{_bindir}/zerodds-secure-permissions
%{_bindir}/zerodds-typeobject
%{_bindir}/zerodds-xml-config
%{_mandir}/man1/zerodds-admin.1*
%{_mandir}/man1/zerodds-idlc.1*

%files devel
%{_includedir}/zerodds.h
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

%files ros2
%{_bindir}/zerodds-ros2-shim
%{_unitdir}/zerodds-ros2-shim.service
%{_datadir}/zerodds/configs/ros2-shim.yaml.example
%{_mandir}/man1/zerodds-ros2-shim.1*
%{_mandir}/man5/zerodds-ros2-shim.yaml.5*

%changelog
* Wed May 06 2026 ZeroDDS Contributors <release@zerodds.org> - 1.0.0-0.rc1
- Initial RC1 RPM split-package layout per zerodds-deployment-1.0 §2.2.2.
