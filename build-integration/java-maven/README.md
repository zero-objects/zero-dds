# Maven — `exec-maven-plugin`

The sample in `sample-consumer/` runs `zerodds-idlc` from Maven through
`exec-maven-plugin`, plus `build-helper-maven-plugin` to add the generated directory as a
compile source root.

## Usage

```bash
cd ../../../crates/java-omgdds/java && mvn -q install -DskipTests   # org.zerodds.cdr, once
cd -
cd sample-consumer
mvn compile
java -cp target/classes:$HOME/.m2/repository/org/zerodds/omgdds/1.0.0-rc.7/omgdds-1.0.0-rc.7.jar Main
# OK: Robot.Pose round-tripped through generated Support_Robot_Pose (40 wire bytes), robot_id=r2d2
```

`mvn compile` runs `exec:exec@generate-idl` (generate-sources phase,
`zerodds-idlc generate --lang java --java-mapping omg --out
target/generated-sources/java src/main/idl/Robot.idl`; the sample reads the
fields, so it names the `omg` mapping (the default is `psm`, with accessors)), then `build-helper:add-source`, then
`compiler:compile`.

The generated code needs two runtimes:

- `org.zerodds.wire.*` from `runtime/java/src/main/java`, added as a second
  source root;
- `org.zerodds.cdr.*` (`TopicTypeSupport`, `EndianMode`,
  `TypeObjectRegistry`) from the `omgdds` artifact of
  `crates/java-omgdds/java`. An `omgdds` installed from an older tree lacks
  `HexBytes` and `TypeObjectRegistry`; install it again.

The generated classes (`ZdTypes`, `ZdCodec`, `ZdTypeSupports`) are in the
unnamed package, so the class that uses them is too (`src/main/java/Main.java`).
Java writes these three file names for every input: one IDL file per
`--out` directory.

## Not covered

- `exec-maven-plugin` has no staleness check; `zerodds-idlc` runs on every
  `mvn compile`.
- No Mojo of its own (`@Parameter` configuration, `BuildContext`,
  `addCompileSourceRoot`) and no Maven Central artifact.
