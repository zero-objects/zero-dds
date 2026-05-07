# Cyclone-DDS-Fixture-Lizenzen

Die IDL-Files in diesem Verzeichnis sind **nicht** aus dem Eclipse Cyclone
DDS Repo kopiert. Sie sind handgepflegte Repraesentanten typischer
Cyclone-DDS-IDL-Konstrukte (OMG-Standard-IDL-4.2 + Standard-Annotations
@final/@appendable/@key) fuer Grammar-Coverage-Tests.

Cyclone DDS nutzt **keine** Vendor-spezifischen Grammar-Erweiterungen —
diese Fixtures parsen mit der Base-Grammar (`IDL_42`) ohne Delta.

Lizenz: dieselbe Lizenz wie das umgebende `zerodds-idl`-Crate.
