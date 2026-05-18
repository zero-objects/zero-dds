# ShapeExtended-Type Support — RTI 7.x Default-Compat (offen)

**Status**: deferred (Quick-Win-Kandidat)
**Datum**: 2026-05-07
**Sprint-Kontext**: bei Cross-Vendor Live-Demo gegen RTI Connext Shapes Demo aufgekommen — RTI 7.x Default-DataType ist `ShapeExtended`, nicht `Shape`. Aktuell muss User RTI mit `-dataType Shape` flag starten, sonst Type-Name-Mismatch.
**Verantwortlich**: open

## Was ist offen

ZeroDDS hat heute nur den **klassischen** `ShapeType`-Topic-Type:
```idl
struct ShapeType {
    @key string color;     // BLUE, RED, GREEN, ...
    long x;                // 0..240
    long y;                // 0..270
    long shapesize;        // typically 30
};
```

RTI 7.x Default ist **ShapeExtendedType**:
```idl
@final
enum ShapeFillKind { SOLID_FILL, TRANSPARENT_FILL, HORIZONTAL_HATCH, VERTICAL_HATCH };

@final
struct ShapeExtendedType {
    @key string color;
    long x;
    long y;
    long shapesize;
    ShapeFillKind fillKind;     // NEU
    float angle;                // NEU
};
```

Pro Type ist es ein **eigener Topic** mit eigenem `TYPE_NAME`. SEDP-Match passiert nur wenn beide Seiten denselben Type haben — `ShapeType ≠ ShapeExtendedType` strict.

## Warum offen

Cross-Vendor-Live-Demo (siehe `docs/interop/shapes-demo.md`) wurde mit dem Standard-`Shape`-Type erfolgreich gegen RTI gefahren. RTI muss dafür mit `-dataType Shape` gestartet werden — eine **kleine User-Friction**, kein Blocker.

ShapeExtended ist der **moderne** Vendor-Default und damit der "no-flag-needed"-Pfad für RTI.

## Implikationen wenn nicht implementiert

**Funktional**: ZeroDDS kann mit RTI-Default-ShapesDemo nicht direkt sprechen. User-Workaround:
```bash
rtishapesdemo -dataType Shape -domainId 0    # explicit fallback
```

Friction:
1. User muss diesen Flag kennen — Setup-Guide dokumentiert es (`docs/interop/shapes-demo.md` §"RTI Shapes Demo zeigt Data Type Shape Extended")
2. RTI-User die Workspace-Files mit ShapeExtended-QoS-Profilen pflegen (übliche Praxis) müssen für ZeroDDS-Tests entweder umstellen oder zwei Workspaces pflegen
3. Cyclone- und FastDDS-Demos sind unauffällig — beide nutzen ebenfalls `Shape` (legacy default)

**Spec-Compliance**: keine. ShapeExtended ist eine Vendor-Convention (RTI Shapes Demo IDL), kein OMG-Spec-Type.

## Wann pick-up sinnvoll

* Wenn ein RTI-User Shapes-Demo gegen ZeroDDS fahren will **ohne Workspace-Anpassung**
* Wenn ein Onboarding-Demo "in 30 Sekunden" ohne CLI-Flags-Erklärung gehen soll
* Wenn eProsima ShapesDemo (Java) ein ähnliches Default updated (sollten wir tracken)
* Wenn RTI XTypes-Conformance-Audit die Extended-Variant als pflicht erklärt (unwahrscheinlich)

## Implementations-Pfad

Geschätzt **1-2 Tage** (Quick-Win):

### A — IDL-Definition + DdsType-Impl (2-3 Std)
Neue Datei `crates/dcps/src/interop_extended.rs`:
```rust
pub enum ShapeFillKind { SolidFill = 0, TransparentFill = 1,
                        HorizontalHatch = 2, VerticalHatch = 3 }

pub struct ShapeExtendedType {
    pub color: String,
    pub x: i32, pub y: i32, pub shapesize: i32,
    pub fill_kind: ShapeFillKind,
    pub angle: f32,
}

impl DdsType for ShapeExtendedType {
    const TYPE_NAME: &'static str = "ShapeExtendedType";
    const HAS_KEY: bool = true;
    const EXTENSIBILITY: Extensibility = Extensibility::Final;
    fn encode(&self, ...) { /* XCDR2-LE-Body */ }
    fn decode(...) { ... }
    fn encode_key_holder_be(...) { /* nur color, gleicher KeyHash wie Shape */ }
}
```

### B — Wire-Tests gegen RTI/eProsima-Captures (3-4 Std)
Datei `crates/dcps/tests/shape_extended_wire.rs`:
* RTI-pcap-Capture decodieren → erwartetes Sample-Object
* ZeroDDS-Encoder-Output gegen RTI-Bytes vergleichen
* Roundtrip-Test wie bei `shapes_type_wire.rs`

### C — Beispiel-Apps (2-3 Std)
* `crates/dcps/examples/shapes_extended_publisher.rs` — analog zu shapes_demo_publisher.rs, aber ShapeExtendedType + animierter angle (sample fillKind cycling)
* `crates/dcps/examples/shapes_extended_subscriber.rs`

### D — Setup-Guide-Update (1 Std)
* `docs/interop/shapes-demo.md` — Section "ShapeExtended-Variante" hinzufügen
* `examples/demos/shapes/README.md` — alternativen run-shapes-extended.sh erwähnen

### E — Live-Cross-Vendor-Test (1 Std)
RTI Shapes Demo **ohne** `-dataType Shape` flag (= ShapeExtended) gegen ZeroDDS-extended-publisher → bouncing shapes mit angle/fill-Variation.

## Pfad bei Pick-up

* **Branch**: `feat/shape-extended-type`
* **Pre-Reqs**: keine — alle Bausteine (DdsType, XCDR2-Encoder, Examples-Pattern) stehen
* **Test-Fixture**: pcap-Capture gegen RTI 7.7.0 mit ShapeExtended publishing (nimm 3 Samples auf, hex-export)
* **Risiko**: minimal — IDL/DdsType-Pattern ist etabliert, FillKind-Enum ist trivial
* **Bonus**: gleicher Pattern lässt sich später auf `dds-twin`-Use-Cases erweitern (komplexere Test-Types)

## Cross-Reference

* `docs/interop/shapes-demo.md` — User-facing Setup-Guide, dokumentiert heute den `-dataType Shape`-Workaround
* `examples/demos/shapes/README.md` — Bundle, aktuell nur ShapeType-Pfad
* `crates/dcps/src/interop.rs` — bestehende ShapeType-Definition als Vorlage
