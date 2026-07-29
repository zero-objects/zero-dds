# Maven — `exec-maven-plugin` (thin form; documented gap to a full Mojo)

This is the **thin `exec-maven-plugin` binding**, not a full custom Mojo.
It is exactly the pattern already documented in
`documentation/04-idl/idlc-handbook.md` §4.5 ("Maven integration via
`exec-maven-plugin`") — this directory turns that documented snippet into
an actual buildable, runnable sample (`sample-consumer/`), plus the
`build-helper-maven-plugin` step the handbook's snippet omits (without
it, `generate-sources`-phase output is never added as a compile source
root, so `mvn compile` would not actually see the generated `.java`).

## What a full Mojo would add (not built here — flagged, not silently skipped)

A real `zerodds-idlc-maven-plugin` (a `Mojo` class implementing
`org.apache.maven.plugin.AbstractMojo`, annotated
`@Mojo(name = "generate", defaultPhase = LifecyclePhase.GENERATE_SOURCES)`)
would give:

- **Native `@Parameter`-bound configuration** (`<idlFiles>`, `<backend>`,
  `<outputDirectory>`) instead of raw `<argument>` strings — type-checked
  at `mvn` startup, not at process-exec time.
- **Incremental build participation** via `BuildContext`
  (`org.sonatype.plexus:plexus-build-api`), the same primitive
  `m2e`/`mvn -o` incremental compilation uses — `exec-maven-plugin`
  always re-execs the process every `mvn compile`, it has no
  file-staleness awareness (the Gradle plugin's `@InputFiles`/
  `@OutputDirectory` and Mix's `manifests/0` both have this;
  `exec-maven-plugin` does not).
- **Automatic source-root registration** (`project.addCompileSourceRoot(...)`
  from Java code) instead of a second plugin (`build-helper-maven-plugin`)
  bolted on to do it.
- Publication to Maven Central as `org.zerodds:zerodds-idlc-maven-plugin`,
  invoked as `<plugin><groupId>org.zerodds</groupId>...` instead of
  `org.codehaus.mojo:exec-maven-plugin` + a raw command line.

This is real, scoped-out work (a new Mojo module, its own `plugin.xml`
descriptor, Maven Central publication) — not implemented in this pass.
The thin form below is fully functional and validated; the gap above is
what "full Mojo" would close.

## Usage

```
cd ../../../crates/java-omgdds/java && mvn -q install -DskipTests   # runtime dep, once
cd -
cd sample-consumer
mvn compile exec:java -Dexec.mainClass=com.example.Main
```

`mvn compile` runs, in order: `exec:exec@generate-idl` (generate-sources
phase, runs `zerodds-idlc`) → `build-helper:add-source` (adds
`target/generated-sources/java` as a compile root) → the normal
`compiler:compile`.

## Validated, and a discovered runtime gap (not a build-tool bug)

Against a locally-built `zerodds-idlc` 1.0.0-rc.6 (Maven 3.9.9, JDK via
Maven toolchain default):

- **Generation: validated, works.** `exec-maven-plugin` runs
  `zerodds-idlc generate ... --java -o target/generated-sources/java`
  and `build-helper-maven-plugin` correctly registers that directory as
  a compile source root (confirmed: `mvn compile` reaches `javac`, not a
  "no such class" resolution failure).
- **Full compile: blocked by a discovered, pre-existing runtime gap.**
  The generated `robot/Pose.java` does:

  ```java
  @org.zerodds.types.Extensibility(org.zerodds.types.Extensibility.Kind.APPENDABLE)
  public class Pose implements org.omg.dds.topic.TopicType<Pose> {
      @org.zerodds.types.Key
      private String robot_id;
  ```

  `org.zerodds.types` (the `@Extensibility`/`@Key` annotations) **does
  not exist anywhere in the repository** — confirmed by a repo-wide grep
  for `package org.zerodds.types`, zero hits. `org.omg.dds.topic.TopicType<T>`
  also doesn't exist; `crates/java-omgdds/java/src/main/java/org/omg/dds/topic/`
  has `Topic`, `ContentFilteredTopic`, `TopicTypeSupport` — no plain
  `TopicType` marker interface. Adding `crates/java-omgdds` (groupId
  `org.zerodds`, artifactId `omgdds`) as a Maven dependency (this
  sample's `pom.xml` does exactly that, `mvn install`ing it into the
  local repo first) resolves `org.zerodds.cdr.*`/`org.omg.dds.topic.TopicTypeSupport`
  correctly, narrowing the failure down to these two missing symbols
  specifically.

  This is the Java analogue of the C# finding in
  `../csharp-msbuild/README.md` (there, both referenced runtime pieces
  exist but collide; here, one referenced package doesn't exist at
  all) — a runtime-completeness gap in `crates/idl-java`'s expected
  companion runtime, not in this build integration or in the
  code-generation emitter logic itself (the emitter correctly emits
  what a complete runtime would need; the runtime piece is
  incomplete/missing). Out of this task's scope to author from
  scratch. Flagged, not silently worked around.
