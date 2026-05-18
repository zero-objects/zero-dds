# OMG DDS 1.2 — DLRL — Open + Partial Items

— keine offenen Items.

DLRL-Metamodel + Entity-Klassen + Exception-Types + Default-
Mapping + Cache-Lifecycle + Annex-B-Query als Stub-Layer in
`crates/dlrl/src/metamodel.rs`:

- ObjectRoot-Trait (§8.1.3.2.1)
- ClassMetamodel + MultiAttributeMetamodel + MonoRelationMetamodel
  (§8.1.3.3 + §8.1.4.4)
- default_mapping-Modul mit topic_name/multi_topic + DEFAULT_*-
  Konstanten (§8.1.4.3)
- DlrlEntityKind-Enum mit allen 17 Spec-Klassen (§8.1.6.2:
  CacheFactory/CacheBase/Cache/CacheAccess/Contract/Selection/
  SelectionCriterion/FilterCriterion/QueryCriterion/SelectionListener/
  ObjectRoot/ObjectHome/Collection/List/Set/StrMap/IntMap)
- DlrlException-Enum mit allen 8 Spec-Exception-Types
  (DCPSError/BadHomeDefinition/NotFound/AlreadyExisting/
  AlreadyDeleted/PreconditionNotMet/NoSuchElement/SQLError)
  + Spec-Repository-IDs (`IDL:omg.org/DLRL/*:1.0`)
- CacheAttachmentState + CacheMode (§8.1.5)
- QueryExpression (Annex B)

Konkrete Wire-Bindung erfolgt durch den ZeroDDS-DCPS-Stack
(`crates/dcps/`) und das DLRL-Codegen
(`crates/dlrl-codegen/`).

## Decision-Records (`n/a (rejected)`)

— keine.
