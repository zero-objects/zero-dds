# zerodds-corba-cos-trading 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-corba-cos-trading-1.0.md` (ZeroDDS Vendor-Spec)

Implementation:

- `crates/corba-cos-trading/` — CORBA COS Trading Service.

## §1 Datenmodell

### §1.1 `Value`

**Spec:** §1.1 — `Value` (Int/Float/Str/Bool) mit `compare` (numerisch inkl. Float-Promotion, Str lexikographisch, Bool, inkompatibel → kein Vergleich).

**Repo:** `crates/corba-cos-trading/src/property.rs::Value` (`compare`, `as_f64`).

**Tests:** `crates/corba-cos-trading/src/constraint.rs::{numeric_comparisons, float_and_mixed_numeric, string_and_bool, incompatible_types_do_not_match}`.

**Status:** done

### §1.2 `Offer`

**Spec:** §1.2 — `Offer { service_type, properties: BTreeMap, ior }`.

**Repo:** `crates/corba-cos-trading/src/trader.rs::Offer` (`new`, `with`).

**Tests:** `crates/corba-cos-trading/src/trader.rs::export_withdraw`.

**Status:** done

## §2 Constraint-Sprache

### §2.1 Grammatik

**Spec:** §2.1 — EBNF: or/and/not, Vergleiche `== != < <= > >=`, `exist`, Klammern, Literale (int/float/'string'/TRUE/FALSE), nackte boolesche Property; `and` > `or`; leerer Constraint = TRUE.

**Repo:** `crates/corba-cos-trading/src/constraint.rs` (`tokenize`, `Parser::parse_or`/`parse_and`/`parse_not`/`parse_comparison`).

**Tests:** `crates/corba-cos-trading/src/constraint.rs::{numeric_comparisons, boolean_logic_and_precedence, parentheses_override_precedence, string_and_bool, empty_constraint_matches_all, negative_numbers}`.

**Status:** done

### §2.2 Evaluation

**Spec:** §2.2 — fehlende Property → Vergleich `false`; inkompatible Typen → `false`; `exist <prop>`; Keyword-Vergleiche case-insensitiv.

**Repo:** `crates/corba-cos-trading/src/constraint.rs::eval` + `keyword_or_ident`.

**Tests:** `crates/corba-cos-trading/src/constraint.rs::{exist_and_missing_property, incompatible_types_do_not_match}`.

**Status:** done

### §2.3 Fehler

**Spec:** §2.3 — `ConstraintError` (UnexpectedToken/UnexpectedEnd/BadNumber/UnterminatedString/BadChar) bei Syntaxfehlern.

**Repo:** `crates/corba-cos-trading/src/constraint.rs::ConstraintError`, `Constraint::parse`.

**Tests:** `crates/corba-cos-trading/src/constraint.rs::parse_errors`.

**Status:** done

## §3 Trader

### §3.1 Register + Lookup

**Spec:** §3.1 — `export`/`withdraw`/`query`; `query` filtert nach service_type (exakt) + constraint, sortiert nach preference, begrenzt auf how_many.

**Repo:** `crates/corba-cos-trading/src/trader.rs::Trader` (`export`, `withdraw`, `query`).

**Tests:** `crates/corba-cos-trading/src/trader.rs::{query_filters_by_type_and_constraint, query_respects_service_type, complex_constraint_query, bad_constraint_errors}`.

**Status:** done

### §3.2 Preferences

**Spec:** §3.2 — `Preference` First/Max/Min; nicht-numerische/fehlende Properties ans Ende.

**Repo:** `crates/corba-cos-trading/src/trader.rs::Preference` + `query`-Sortierung.

**Tests:** `crates/corba-cos-trading/src/trader.rs::{preference_max_orders_descending, preference_min_and_how_many}`.

**Status:** done

## §4 Föderation + Dynamic Properties + Service-Type-Hierarchien

### §4.1 Föderierte Trader (`Link` + `Proxy`)

**Spec:** CosTrading `Link`/`Proxy` — Trader-zu-Trader-Links mit Folge-Regel + Hop-Limit; Proxy-Offers leiten Queries weiter.

**Repo:** `crates/corba-cos-trading/src/federation.rs::Federation` (`link`/`add_proxy`/`query` mit `FollowOption` + BFS-Loop-Vermeidung), `ProxyOffer` mit Recipe-AND-Constraint.

**Tests:** `crates/corba-cos-trading/src/federation.rs::{query_follows_links_within_hop_limit, local_only_link_not_followed, if_no_local_only_follows_when_empty, cycle_is_not_infinite, global_preference_orders_merged_result, proxy_offer_redirects_with_recipe, unknown_node_link_errors}`.

**Status:** done

### §4.2 Dynamic Properties

**Spec:** CosTradingDynamic `DynamicPropEval` — pro Query berechnete Property-Werte.

**Repo:** `crates/corba-cos-trading/src/dynamic.rs::DynamicPropEval`; Auflösung in `trader.rs::Trader::effective_props` (vor Constraint- und Preference-Auswertung).

**Tests:** `crates/corba-cos-trading/src/trader.rs::{dynamic_property_resolved_in_constraint, dynamic_property_used_in_preference}`; `crates/corba-cos-trading/src/dynamic.rs::evaluator_computes_value_from_context`.

**Status:** done

### §4.3 Service-Type-Hierarchien

**Spec:** CosTradingRepos `ServiceTypeRepository` — Vererbungsgraph; Query auf einen Basis-Type matcht abgeleitete Offers (Subtyp-Matching) + Mandatory-Property-Modi.

**Repo:** `crates/corba-cos-trading/src/service_type.rs::ServiceTypeRepository` (`is_a`/`mandatory_properties`); `trader.rs::Trader::type_matches` + `export_checked`.

**Tests:** `crates/corba-cos-trading/src/service_type.rs::{is_a_transitive, unknown_super_type_rejected, duplicate_type_rejected, mandatory_properties_inherited, effective_properties_merge_without_dup, all_super_types_includes_self}`; `crates/corba-cos-trading/src/trader.rs::{query_on_base_type_matches_subtypes, export_checked_validates_mandatory_and_type, export_checked_accepts_dynamic_mandatory}`.

**Status:** done

---

## Audit-Status

10 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-corba-cos-trading` — 36 Tests grün, 0 failed.
