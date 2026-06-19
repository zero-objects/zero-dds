# QoS-Profile XML-Handling — Vendor-Interop

**Status**: completed (2026-06-13)
**Datum**: 2026-05-07

## Resolution (2026-06-13)

Der fehlende **Profile-Resolver + Entity-Wireup** ist umgesetzt (die XML-Parser-
Schicht in `crates/xml` stand bereits):

1. **`zerodds_xml::QosProfileRegistry`** (`crates/xml/src/registry.rs`):
   `from_xml(xml)` / `from_file(path)` lädt alle `<qos_library>`, und
   `writer_qos("Lib::Profile")` / `reader_qos(...)` löst einen (qualifizierten
   oder unqualifizierten) Profil-Verweis unter **voller `base_name`-Inheritance**
   (§7.3.2.4.2, base→derived-Merge via `resolve_chain` + `EntityQos::merge`) zu
   einer materialisierten `zerodds_qos::WriterQos`/`ReaderQos` auf. 5 Unit-Tests
   (Base-Resolve, Inheritance-Override, unqualifiziert→erste Library, Reader,
   UnresolvedReference).
2. **Entity-Wireup**: `impl From<zerodds_qos::WriterQos> for DataWriterQos` +
   `From<ReaderQos> for DataReaderQos` in `crates/dcps/src/qos.rs` (alle geteilten
   Policies; DCPS-only `data_representation` bleibt Default). Damit:
   ```rust
   let reg = QosProfileRegistry::from_file("profiles.xml")?;
   publisher.create_datawriter::<T>(&topic, reg.writer_qos("MyLib::HighPerf")?.into())?;
   ```
   Migration ohne Hardcoding der QoS in Rust — RTI/Cyclone/FastDDS-Profil-XML
   wird direkt konsumiert.

Layering respektiert: `xml` kennt `qos` (nicht `dcps`); die `From`-Brücke lebt in
`dcps` (kennt `qos`). Phase D (Cross-Vendor-Live-Demo + Hot-Reload) bleibt als
Test-Rig-Erweiterung optional; der normative Resolver-Pfad ist vollständig.
**Sprint-Kontext**: bei der Cross-Vendor Live-Demo (D.5g) gegen RTI Connext Shapes Demo aufgekommen — RTI lädt QoS aus XML-Profilen, ZeroDDS hat keinen Loader/Resolver dafür
**Verantwortlich**: open

## Was ist offen

ZeroDDS soll **vendor-konforme XML-QoS-Profile** lesen und auf Pub/Sub-Erstellung anwenden können. RTI, Cyclone und FastDDS lassen ihre User über XML-Files QoS-Settings definieren und per Profile-Name referenzieren:

```xml
<!-- RTI / FastDDS Style -->
<dds>
  <qos_library name="MyLib">
    <qos_profile name="HighPerf">
      <datawriter_qos>
        <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
        <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>64</depth></history>
        <representation>
          <value><element>XCDR2_DATA_REPRESENTATION</element></value>
        </representation>
      </datawriter_qos>
    </qos_profile>
  </qos_library>
</dds>
```

User-Code referenziert nur `MyLib::HighPerf` und QoS wird automatisch geladen.

## Warum offen

Cross-Vendor-Live-Interop hat ohne Profile-Loader bereits funktioniert (siehe `docs/interop/shapes-demo.md`) — ZeroDDS-Default-QoS reichten für RTI Shapes Demo. Profile-Loader ist nur dann zwingend, wenn:

* User von einem RTI/Cyclone/FastDDS-Deployment migriert und seine bestehenden QoS-Profile **nicht hardcoden** möchte
* User die Shapes Demo mit non-default-Profile fährt (z.B. Reliability-Profile, oder eigenes Custom-Profile)
* Compliance-Test gegen DDS-XML 1.0 Conformance-Suite

## Existing-Infrastructure-Check

ZeroDDS hat **DDS-XML 1.0 Spec-Coverage live**:

* `crates/xml/` — DDS-XML 1.0 Loader (mit XSD-Validation, 14 normative XSD-Files)
* Topics + Domain-Inheritance + Single-QoS-Override implementiert

**Pre-existing**: nur die **Wire-up zu DcpsRuntime + DataWriter/DataReader** ist offen. Die XML-Parser-Schicht steht.

## Implikationen wenn nicht implementiert

**Funktional**: User kann QoS heute nur per Code (Rust) setzen — fluent style:
```rust
let qos = DataWriterQos {
    reliability: ReliabilityQosPolicy { kind: ReliabilityKind::Reliable, ... },
    history: HistoryQosPolicy { kind: HistoryKind::KeepLast, depth: 64 },
    ..DataWriterQos::default()
};
```

Das funktioniert. Aber:

1. **Migration-Pfad fehlt**: User mit existierendem `<qos_profile>`-XML-File aus RTI/Cyclone/FastDDS muss die Settings manuell in Rust-Code übersetzen — Reibung beim Vendor-Switch
2. **Operations-Pfad fehlt**: SREs können QoS nicht im laufenden Betrieb umstellen ohne Recompile (z.B. via Hot-Reload eines XML-Files)
3. **Spec-Compliance**: DDS-XML 1.0 §6.4 fordert Profile-Resolution für vollständige Conformance — heute hat ZeroDDS Profile-**Parsing** aber keinen Profile-**Resolver** für DDS-Entity-Erstellung
4. **Vendor-Tools-Interop**: Tools wie RTI Admin Console oder Cyclone-CLI generieren QoS-Profile-XML — ZeroDDS kann sie nicht direkt konsumieren

## Wann pick-up sinnvoll

* Wenn ein Migration-Use-Case "ich habe eine RTI-Deployment-XML, will ZeroDDS einsetzen" entsteht
* Wenn der Customer-Onboarding-Flow durch das Manual-Translate-XML-zu-Rust-Code als Reibung identifiziert wird
* Wenn DDS-XML 1.0 Compliance-Audit (z.B. K7 strict) das fordert
* Wenn Hot-Reload-QoS als Feature priorisiert wird

## Wie pick-up aussehen würde

Geschätzt **1-1.5 Sprints** (1-2 Wochen). Vier Phasen:

### A — XML-Loader-API (2-3 Tage)
1. `DomainParticipantFactory::load_qos_profiles(path: &Path)` — lädt XML-File, validiert via XSD, baut ein in-memory `QosProfileRegistry`
2. `QosProfileRegistry::get_datawriter_qos(profile_name)` etc.
3. Cyclone-XML-Format vs RTI-XML-Format Detection (beide leicht unterschiedlich)

### B — Profile-Resolution-Chain (3-4 Tage)
Resolution bei `create_datawriter`:
```
Per-Call-QoS  ⇒  Profile-Inherit (parent_profile)  ⇒  Domain-Default  ⇒  Lib-Default
```
Spec-konform per DDS-XML 1.0 §7.3.

### C — Wire-up an DataWriter/DataReader/Topic (2-3 Tage)
```rust
let qos = factory.qos_profile("MyLib::HighPerf").to_datawriter_qos();
publisher.create_datawriter::<T>(&topic, qos)?;
```
ODER inline: `publisher.create_datawriter_with_profile::<T>(&topic, "MyLib::HighPerf")?;`

### D — Tests + Conformance (3-5 Tage)
1. RTI-XML-Profile-Roundtrip (Bytes-identisch zu RTI-Encoding)
2. Cyclone-XML-Profile-Roundtrip
3. FastDDS-XML-Profile-Roundtrip
4. Cross-Vendor-Demo: ZeroDDS-Pub mit "MyLib::HighPerf"-Profile, RTI-Sub mit gleichem Profile-Name + XML-File → matched
5. Hot-Reload-Test (XML-File ändert, neuer Pub picks up new QoS)

## Pfad bei Pick-up

* **Branch**: `feat/qos-profile-xml-loader`
* **Pre-Reqs**: `crates/xml` muss aktuelle DDS-XML 1.0 Spec abdecken (heute ✓)
* **Test-Vendoren-XMLs**: aus `tests/interop/cyclonedds.xml` (existiert bereits) + `crates/xml/tests/fixtures/` (RTI-Style-XSDs vorhanden)
* **Risiko**: niedrig — DDS-XML 1.0 ist unstrittig spec'd, Wire-up ist Low-Drama-Engineering
* **Cross-Vendor-Validierung**: gegen RTI Shapes Demo Workspace-File (`-workspaceFile rti_workspace/.../shapes_demo.xml`)
