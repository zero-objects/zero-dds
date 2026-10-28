# zerodds-corba-cos-trading 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-corba-cos-trading-1.0.md` (ZeroDDS vendor spec)

Implementation:

- `crates/corba-cos-trading/` — CORBA COS Trading Service.

## §1 Data model

### §1.1 `Value`

**Spec:** §1.1 — `Value` (Int/Float/Str/Bool) with `compare` (numeric incl. float promotion, Str lexicographic, Bool, incompatible → no comparison).

**Repo:** `crates/corba-cos-trading/src/property.rs::Value` (`compare`, `as_f64`).

**Tests:** `crates/corba-cos-trading/src/constraint.rs::{numeric_comparisons, float_and_mixed_numeric, string_and_bool, incompatible_types_do_not_match}`.

**Status:** done

### §1.2 `Offer`

**Spec:** §1.2 — `Offer { service_type, properties: BTreeMap, ior }`.

**Repo:** `crates/corba-cos-trading/src/trader.rs::Offer` (`new`, `with`).

**Tests:** `crates/corba-cos-trading/src/trader.rs::export_withdraw`.

**Status:** done

## §2 Constraint language

### §2.1 Grammar

**Spec:** §2.1 — EBNF: or/and/not, comparisons `== != < <= > >=`, `exist`, parentheses, literals (int/float/'string'/TRUE/FALSE), bare boolean property; `and` > `or`; empty constraint = TRUE.

**Repo:** `crates/corba-cos-trading/src/constraint.rs` (`tokenize`, `Parser::parse_or`/`parse_and`/`parse_not`/`parse_comparison`).

**Tests:** `crates/corba-cos-trading/src/constraint.rs::{numeric_comparisons, boolean_logic_and_precedence, parentheses_override_precedence, string_and_bool, empty_constraint_matches_all, negative_numbers}`.

**Status:** done

### §2.2 Evaluation

**Spec:** §2.2 — missing property → comparison `false`; incompatible types → `false`; `exist <prop>`; keyword comparisons case-insensitive.

**Repo:** `crates/corba-cos-trading/src/constraint.rs::eval` + `keyword_or_ident`.

**Tests:** `crates/corba-cos-trading/src/constraint.rs::{exist_and_missing_property, incompatible_types_do_not_match}`.

**Status:** done

### §2.3 Errors

**Spec:** §2.3 — `ConstraintError` (UnexpectedToken/UnexpectedEnd/BadNumber/UnterminatedString/BadChar) on syntax errors.

**Repo:** `crates/corba-cos-trading/src/constraint.rs::ConstraintError`, `Constraint::parse`.

**Tests:** `crates/corba-cos-trading/src/constraint.rs::parse_errors`.

**Status:** done

## §3 Trader

### §3.1 Register + lookup

**Spec:** §3.1 — `export`/`withdraw`/`query`; `query` filters by service_type (exact) + constraint, sorts by preference, limited to how_many.

**Repo:** `crates/corba-cos-trading/src/trader.rs::Trader` (`export`, `withdraw`, `query`).

**Tests:** `crates/corba-cos-trading/src/trader.rs::{query_filters_by_type_and_constraint, query_respects_service_type, complex_constraint_query, bad_constraint_errors}`.

**Status:** done

### §3.2 Preferences

**Spec:** §3.2 — `Preference` First/Max/Min; non-numeric/missing properties last.

**Repo:** `crates/corba-cos-trading/src/trader.rs::Preference` + `query` sorting.

**Tests:** `crates/corba-cos-trading/src/trader.rs::{preference_max_orders_descending, preference_min_and_how_many}`.

**Status:** done

## §4 Federation + dynamic properties + service-type hierarchies

### §4.1 Federated traders (`Link` + `Proxy`)

**Spec:** CosTrading `Link`/`Proxy` — trader-to-trader links with a follow rule + hop limit; proxy offers forward queries.

**Repo:** `crates/corba-cos-trading/src/federation.rs::Federation` (`link`/`add_proxy`/`query` with `FollowOption` + BFS loop avoidance), `ProxyOffer` with recipe-AND constraint.

**Tests:** `crates/corba-cos-trading/src/federation.rs::{query_follows_links_within_hop_limit, local_only_link_not_followed, if_no_local_only_follows_when_empty, cycle_is_not_infinite, global_preference_orders_merged_result, proxy_offer_redirects_with_recipe, unknown_node_link_errors}`.

**Status:** done

### §4.2 Dynamic properties

**Spec:** CosTradingDynamic `DynamicPropEval` — property values computed per query.

**Repo:** `crates/corba-cos-trading/src/dynamic.rs::DynamicPropEval`; resolution in `trader.rs::Trader::effective_props` (before constraint and preference evaluation).

**Tests:** `crates/corba-cos-trading/src/trader.rs::{dynamic_property_resolved_in_constraint, dynamic_property_used_in_preference}`; `crates/corba-cos-trading/src/dynamic.rs::evaluator_computes_value_from_context`.

**Status:** done

### §4.3 Service-type hierarchies

**Spec:** CosTradingRepos `ServiceTypeRepository` — inheritance graph; a query on a base type matches derived offers (subtype matching) + mandatory-property modes.

**Repo:** `crates/corba-cos-trading/src/service_type.rs::ServiceTypeRepository` (`is_a`/`mandatory_properties`); `trader.rs::Trader::type_matches` + `export_checked`.

**Tests:** `crates/corba-cos-trading/src/service_type.rs::{is_a_transitive, unknown_super_type_rejected, duplicate_type_rejected, mandatory_properties_inherited, effective_properties_merge_without_dup, all_super_types_includes_self}`; `crates/corba-cos-trading/src/trader.rs::{query_on_base_type_matches_subtypes, export_checked_validates_mandatory_and_type, export_checked_accepts_dynamic_mandatory}`.

**Status:** done

---

## Audit status

10 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-corba-cos-trading` — 36 tests green, 0 failed.
