// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The security of spec §7 as the daemon serves it: TLS with ALPN, the auth
//! modes, the topic ACL, and the SIGHUP reload of all three (§9.2).

use super::*;

/// The §7 settings as the daemon serves them: TLS with ALPN `h2`, the
/// auth mode, the topic ACL. `Err((exit code, message))`; exit code 4 is
/// Spec §2's TLS error.
pub(crate) fn security_from(cfg: &DaemonConfig) -> Result<SecurityCtx, (u8, String)> {
    let mut ctx = security_settings(cfg)?;
    if cfg.tls_enabled {
        if cfg.tls_cert_file.is_empty() || cfg.tls_key_file.is_empty() {
            return Err((
                4,
                "tls: grpc.tls.enabled needs cert_file and key_file".into(),
            ));
        }
        let ca = (!cfg.tls_client_ca_file.is_empty())
            .then(|| std::path::PathBuf::from(&cfg.tls_client_ca_file));
        let built = build_grpc_tls_config_opts(
            cfg.tls_cert_file.clone().into(),
            cfg.tls_key_file.clone().into(),
            ca,
            cfg.tls_require_client_cert,
            &cfg.tls_options,
        )
        .map_err(|e| (4, format!("tls: {e}")))?;
        // §3 `grpc.tls.alpn`.
        let mut server = Arc::unwrap_or_clone(built);
        server.alpn_protocols = cfg.tls_alpn.iter().map(|p| p.as_bytes().to_vec()).collect();
        ctx.tls = Some(Arc::new(server));
    }
    Ok(ctx)
}

/// The configuration part of [`security_from`], without the TLS material:
/// the auth mode with what it needs and the topic ACL. Every error is a
/// configuration error (exit 1); `--check-config` runs it too (deployment
/// §2.1: the configuration is loaded as at startup).
///
/// Spec §7.2: `auth.mode: none` is the dev mode of a loopback `grpc.bind`
/// (127.0.0.0/8, `::1`, `localhost`); on another address it stops the
/// start, whether TLS is on or not.
pub(crate) fn security_settings(cfg: &DaemonConfig) -> Result<SecurityCtx, (u8, String)> {
    let config_error = |m: String| (1u8, format!("config: {m}"));
    let mut sc = SecurityConfig::default();
    sc.auth_mode = cfg.auth_mode.clone();
    match cfg.auth_mode.as_str() {
        "none" if !bind_is_loopback(&cfg.bind) => {
            return Err(config_error(format!(
                "auth.mode: none answers every call unauthenticated and is the dev mode of a \
                 loopback grpc.bind (spec §7.2); grpc.bind {} needs auth.mode bearer, jwt or mtls",
                cfg.bind
            )));
        }
        "bearer" => {
            let token = cfg
                .auth_bearer_token
                .clone()
                .ok_or_else(|| config_error("auth.mode: bearer needs auth.bearer_token".into()))?;
            let subject = cfg
                .auth_bearer_subject
                .clone()
                .unwrap_or_else(|| "anonymous".into());
            sc.bearer_tokens.insert(token, subject);
        }
        "jwt" => sc
            .apply_jwt(&cfg.auth_jwt)
            .map_err(|e| config_error(e.to_string()))?,
        "mtls" if (!cfg.tls_enabled || cfg.tls_client_ca_file.is_empty()) => {
            return Err(config_error(
                "auth.mode: mtls needs grpc.tls.enabled and grpc.tls.client_ca_file".into(),
            ));
        }
        _ => {}
    }
    // §1 L6: a tenant's topic is `<tenant>/<dds_name>` in the ACL.
    let every_topic = cfg
        .topics
        .iter()
        .map(|t| (None, t))
        .chain(cfg.tenants.iter().flat_map(|tenant| {
            tenant
                .topics
                .iter()
                .map(|t| (Some(tenant.name.as_str()), t))
        }));
    for (tenant, t) in every_topic {
        if let Some((publish, subscribe)) = &t.acl {
            sc.topic_acl.insert(
                tenant_label(tenant, &t.dds_name),
                AclEntry {
                    read: subscribe.clone(),
                    write: publish.clone(),
                },
            );
        }
    }
    build_ctx(&sc).map_err(|e| config_error(e.to_string()))
}

/// §9.2 SIGHUP: [`apply_reload`] with its outcome in the log —
/// `reloaded` and `reload_needs_restart`, or `reload_refused` with the
/// reason.
pub(crate) fn reload_security(
    live: &zerodds_bridge_security::LiveSecurity,
    started: &DaemonConfig,
    new: Result<DaemonConfig, String>,
) {
    match apply_reload(live, started, new) {
        Ok(restart) => {
            event(Level::Info, "reloaded")
                .text("applied", "tls, auth, acl")
                .emit();
            if !restart.is_empty() {
                event(Level::Warn, "reload_needs_restart")
                    .text("settings", &restart.join(", "))
                    .emit();
            }
        }
        Err(e) => event(Level::Warn, "reload_refused").text("err", &e).emit(),
    }
}

/// §9.2 SIGHUP: the TLS material, the auth settings and the ACL of the
/// reloaded files replace the live ones; `Ok` names the settings that
/// need a restart. What needs a restart (the listener, the DDS side, the
/// topic mapping, TLS on or off) keeps its started value, and the auth
/// rules are checked against it: §7.2 `none` against the `grpc.bind` the
/// listener serves, not the one the file names. A file that does not
/// load, or settings that cannot be served, change nothing (`Err` with the
/// reason).
pub(crate) fn apply_reload(
    live: &zerodds_bridge_security::LiveSecurity,
    started: &DaemonConfig,
    new: Result<DaemonConfig, String>,
) -> Result<Vec<&'static str>, String> {
    let mut new = new?;
    let mapping = |c: &DaemonConfig| -> Vec<TopicMapping> {
        c.topics
            .iter()
            .cloned()
            .map(|mut t| {
                t.acl = None;
                t
            })
            .collect()
    };
    let mut restart: Vec<&'static str> = Vec::new();
    if new.bind != started.bind || new.domain != started.domain {
        restart.push("bind/domain");
        // The listener keeps its address until a restart: `none` on a
        // reload that moves `grpc.bind` to loopback would answer the
        // still-bound public address unauthenticated.
        new.bind = started.bind.clone();
    }
    if mapping(&new) != mapping(started) {
        restart.push("topics");
    }
    // A tenant's ACL is applied by the reload; the rest needs a restart.
    let tenants = |c: &DaemonConfig| -> Vec<TenantConfig> {
        c.tenants
            .iter()
            .cloned()
            .map(|mut t| {
                for topic in &mut t.topics {
                    topic.acl = None;
                }
                t
            })
            .collect()
    };
    if tenants(&new) != tenants(started) {
        restart.push("tenants");
    }
    if new.limits != started.limits
        || new.keepalive_time_secs != started.keepalive_time_secs
        || new.keepalive_timeout_secs != started.keepalive_timeout_secs
    {
        restart.push("grpc limits/keepalive");
    }
    if new.drain_secs != started.drain_secs {
        restart.push("grpc.drain_secs");
    }
    if new.metrics_path != started.metrics_path {
        restart.push("metrics.path");
    }
    if new.reflection_enabled != started.reflection_enabled {
        restart.push("grpc.reflection.enabled");
    }
    let tls_toggled = new.tls_enabled != started.tls_enabled;
    if tls_toggled {
        restart.push("grpc.tls.enabled");
        new.tls_enabled = false;
    }
    let mut ctx = security_from(&new).map_err(|(_, e)| e)?;
    if tls_toggled {
        ctx.tls = live.current().tls.clone();
    }
    live.replace(ctx);
    Ok(restart)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use crate::bridge_daemon::test_support::parse_yaml_subset;
    use crate::bridge_daemon::*;
    use crate::bridge_security::AuthMode;

    /// Spec §1 L6, §7.3: the ACL entries of a tenant's topics are
    /// `<tenant>/<dds_name>`, so a tenant's rule does not open the topic of
    /// the same name of `domain`.
    #[test]
    fn grpcbridge_1_l6_the_acl_of_a_tenant_topic_carries_its_name() {
        let cfg = parse_yaml_subset(
            "auth:\n  mode: mtls\ngrpc:\n  tls:\n    enabled: true\n    client_ca_file: ca.pem\ntopics:\n  - dds_name: T\n    grpc_service: S\n    acl:\n      publish: [bob]\ntenants:\n  - name: a\n    subjects: [x]\n    topics:\n      - dds_name: T\n        grpc_service: S\n        acl:\n          publish: [alice]\n",
        )
        .expect("parse");
        // The configuration part: the test has no TLS material.
        let ctx = security_settings(&cfg).expect("security");
        let alice = AuthSubject::new("alice".to_string());
        assert!(authorize(&ctx.acl, &alice, AclOp::Write, "a/T"));
        assert!(!authorize(&ctx.acl, &alice, AclOp::Write, "T"));
        assert!(authorize(
            &ctx.acl,
            &AuthSubject::new("bob".to_string()),
            AclOp::Write,
            "T"
        ));
    }

    /// `authorization` metadata of a call.
    fn bearer(token: &str) -> Vec<(String, String)> {
        vec![("authorization".to_string(), format!("Bearer {token}"))]
    }

    /// Spec §7.2: `none` only on a loopback `grpc.bind` — 127.0.0.0/8,
    /// `::1`, `localhost` — with or without TLS; on another address the
    /// start stops (exit 1) and names the modes that serve it.
    #[test]
    fn grpcbridge_7_2_mode_none_only_on_a_loopback_bind() {
        for bind in [
            "127.0.0.1:50051",
            "127.3.0.9:1",
            "[::1]:50051",
            "localhost:50051",
        ] {
            let cfg = DaemonConfig {
                bind: bind.into(),
                ..DaemonConfig::default()
            };
            let ctx = security_settings(&cfg).expect(bind);
            assert!(matches!(*ctx.auth, AuthMode::None), "{bind}");
        }
        for (bind, tls) in [
            ("0.0.0.0:50051", true),
            ("192.0.2.7:50051", true),
            ("[::]:1", false),
        ] {
            let cfg = DaemonConfig {
                bind: bind.into(),
                tls_enabled: tls,
                ..DaemonConfig::default()
            };
            let (code, msg) = security_settings(&cfg).unwrap_err();
            assert_eq!(code, 1, "{bind}");
            assert!(
                msg.contains("auth.mode: none") && msg.contains("§7.2"),
                "{msg}"
            );
        }
        // Another mode on the same address is not refused for it.
        let cfg = DaemonConfig {
            bind: "0.0.0.0:50051".into(),
            tls_enabled: true,
            auth_mode: "bearer".into(),
            auth_bearer_token: Some("t".into()),
            ..DaemonConfig::default()
        };
        assert!(security_settings(&cfg).is_ok());
    }

    /// Spec §7.2: each mode builds its check — `bearer` takes its token and
    /// stands for `auth.bearer_subject`, `mtls` takes the certificate's
    /// subject and refuses a call without one, `none` is `anonymous`.
    #[test]
    fn grpcbridge_12_1_auth_each_mode_builds_its_check() {
        let cfg = DaemonConfig {
            bind: "127.0.0.1:1".into(),
            auth_mode: "bearer".into(),
            auth_bearer_token: Some("s3cret".into()),
            auth_bearer_subject: Some("alice".into()),
            ..DaemonConfig::default()
        };
        let ctx = security_settings(&cfg).expect("bearer");
        let ok = authenticate_grpc(&ctx.auth, &bearer("s3cret"), None).expect("token");
        assert_eq!(ok.name, "alice");
        assert!(authenticate_grpc(&ctx.auth, &bearer("other"), None).is_err());
        assert!(authenticate_grpc(&ctx.auth, &[], None).is_err());
        // Without auth.bearer_subject the token stands for `anonymous`.
        let cfg = DaemonConfig {
            auth_bearer_subject: None,
            ..cfg
        };
        let ctx = security_settings(&cfg).expect("bearer");
        let ok = authenticate_grpc(&ctx.auth, &bearer("s3cret"), None).expect("token");
        assert_eq!(ok.name, "anonymous");

        let cfg = DaemonConfig {
            bind: "0.0.0.0:1".into(),
            tls_enabled: true,
            tls_client_ca_file: "ca.pem".into(),
            auth_mode: "mtls".into(),
            ..DaemonConfig::default()
        };
        let ctx = security_settings(&cfg).expect("mtls");
        let cert = AuthSubject::new("CN=bob".to_string());
        let ok = authenticate_grpc(&ctx.auth, &[], Some(cert)).expect("certificate");
        assert_eq!(ok.name, "CN=bob");
        assert!(authenticate_grpc(&ctx.auth, &bearer("s3cret"), None).is_err());

        let ctx = security_settings(&DaemonConfig {
            bind: "127.0.0.1:1".into(),
            ..DaemonConfig::default()
        })
        .expect("none");
        assert_eq!(
            authenticate_grpc(&ctx.auth, &[], None).expect("none").name,
            "anonymous"
        );
    }

    /// Spec §3, §7.2: what a mode needs and does not have stops the start
    /// with the key at fault — `bearer` without a token, `jwt` without a
    /// key or with a key file that is not there or holds no key, `mtls`
    /// without TLS or without a client CA.
    #[test]
    fn grpcbridge_12_1_auth_a_mode_without_what_it_needs_does_not_start() {
        let base = DaemonConfig {
            bind: "127.0.0.1:1".into(),
            ..DaemonConfig::default()
        };
        let not_a_key =
            std::env::temp_dir().join(format!("zd-grpc-auth-{}-not-a-key.pem", std::process::id()));
        std::fs::write(&not_a_key, "no key\n").expect("file");
        let jwt = |public_key: &str| DaemonConfig {
            auth_mode: "jwt".into(),
            auth_jwt: zerodds_bridge_security::JwtSettings {
                public_key: public_key.into(),
                ..Default::default()
            },
            ..base.clone()
        };
        for (cfg, needle) in [
            (
                DaemonConfig {
                    auth_mode: "bearer".into(),
                    ..base.clone()
                },
                "auth.bearer_token",
            ),
            (jwt(""), "auth.jwt.public_key"),
            (jwt("/nonexistent/zd-jwt.pem"), "/nonexistent/zd-jwt.pem"),
            (
                jwt(not_a_key.to_str().expect("utf-8")),
                "auth.jwt.public_key",
            ),
            (
                DaemonConfig {
                    auth_mode: "mtls".into(),
                    ..base.clone()
                },
                "grpc.tls.enabled and grpc.tls.client_ca_file",
            ),
            (
                DaemonConfig {
                    auth_mode: "mtls".into(),
                    tls_enabled: true,
                    ..base.clone()
                },
                "grpc.tls.enabled and grpc.tls.client_ca_file",
            ),
        ] {
            let (code, msg) = security_settings(&cfg).unwrap_err();
            assert_eq!(code, 1, "{msg}");
            assert!(msg.contains(needle), "{needle}: {msg}");
        }
        let _ = std::fs::remove_file(not_a_key);
    }

    /// Spec §9.2 SIGHUP: a reload replaces the auth settings; a file that
    /// does not load, and settings that cannot be served, keep the live
    /// ones.
    #[test]
    fn grpcbridge_12_1_auth_a_reload_that_fails_keeps_the_live_settings() {
        let started = DaemonConfig {
            bind: "127.0.0.1:1".into(),
            auth_mode: "bearer".into(),
            auth_bearer_token: Some("first".into()),
            ..DaemonConfig::default()
        };
        let live =
            zerodds_bridge_security::LiveSecurity::new(security_from(&started).expect("started"));
        let accepts =
            |token: &str| authenticate_grpc(&live.current().auth, &bearer(token), None).is_ok();
        assert!(accepts("first"));
        assert_eq!(
            apply_reload(&live, &started, Err("syntax error".into())),
            Err("syntax error".to_string())
        );
        assert!(
            accepts("first"),
            "a file that does not load changes nothing"
        );
        let err = apply_reload(
            &live,
            &started,
            Ok(DaemonConfig {
                auth_mode: "jwt".into(),
                ..started.clone()
            }),
        )
        .unwrap_err();
        assert!(err.contains("auth.jwt.public_key"), "{err}");
        assert!(accepts("first"), "jwt without a key cannot be served");
        assert_eq!(
            apply_reload(
                &live,
                &started,
                Ok(DaemonConfig {
                    auth_bearer_token: Some("second".into()),
                    ..started.clone()
                }),
            ),
            Ok(Vec::new())
        );
        assert!(accepts("second"));
        assert!(!accepts("first"));
    }

    /// Spec §7.2, §9.2: a reload checks `none` against the `grpc.bind` the
    /// listener serves; a changed bind waits for a restart. Started on
    /// 0.0.0.0 with `bearer`, a file that moves the bind to 127.0.0.1 and
    /// sets `none` is refused and the token stays the check; a moved bind
    /// with another mode applies the mode and names the restart; a daemon
    /// that serves 127.0.0.1 takes `none`. (The TLS material, which a
    /// public bind needs at the start, takes no part in these rules and is
    /// left out.)
    #[test]
    fn grpcbridge_7_2_a_reload_checks_none_against_the_served_bind() {
        use zerodds_bridge_security::LiveSecurity;
        let started = DaemonConfig {
            bind: "0.0.0.0:50999".into(),
            auth_mode: "bearer".into(),
            auth_bearer_token: Some("tk".into()),
            ..DaemonConfig::default()
        };
        let live = LiveSecurity::new(security_from(&started).expect("started"));
        let to_none = DaemonConfig {
            bind: "127.0.0.1:50999".into(),
            auth_mode: "none".into(),
            auth_bearer_token: None,
            ..started.clone()
        };
        let err = apply_reload(&live, &started, Ok(to_none.clone())).unwrap_err();
        assert!(
            err.contains("auth.mode: none") && err.contains("grpc.bind 0.0.0.0:50999"),
            "{err}"
        );
        assert!(matches!(*live.current().auth, AuthMode::Bearer { .. }));
        assert!(authenticate_grpc(&live.current().auth, &[], None).is_err());
        assert!(authenticate_grpc(&live.current().auth, &bearer("tk"), None).is_ok());

        let moved = DaemonConfig {
            bind: "127.0.0.1:50999".into(),
            auth_bearer_token: Some("tk-2".into()),
            ..started.clone()
        };
        assert_eq!(
            apply_reload(&live, &started, Ok(moved)),
            Ok(vec!["bind/domain"])
        );
        assert!(authenticate_grpc(&live.current().auth, &bearer("tk-2"), None).is_ok());
        assert!(authenticate_grpc(&live.current().auth, &bearer("tk"), None).is_err());

        let local = DaemonConfig {
            bind: "127.0.0.1:50999".into(),
            ..started
        };
        let live = LiveSecurity::new(security_from(&local).expect("local"));
        assert_eq!(apply_reload(&live, &local, Ok(to_none)), Ok(Vec::new()));
        assert!(matches!(*live.current().auth, AuthMode::None));
    }
}
