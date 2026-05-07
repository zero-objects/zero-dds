# RTI-Fixture-Lizenzen

Die IDL-Files in diesem Verzeichnis sind **nicht** aus dem RTI Connext SDK
kopiert. Sie sind handgepflegte Repraesentanten typischer RTI-Connext-IDL-
Konstrukte fuer Grammar-/Delta-Tests, basierend auf der publizierten
RTI-Doku-Syntax (https://community.rti.com/static/documentation/connext-dds/).

Verwendete RTI-spezifische Konstrukte:
- `keylist <Type> (<field>+);` — alternativ zu `@key`-Annotation
- (perspektivisch) `#pragma keylist <Type> <field>...` — Preprocessor-Form

Lizenz: dieselbe Lizenz wie das umgebende `zerodds-idl`-Crate (Workspace-Default).
Keine RTI-eigentumlichen Texte enthalten.
