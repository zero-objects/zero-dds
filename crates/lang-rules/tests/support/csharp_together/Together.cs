// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The cases of one corpus share of the C# gates in one .NET process
// (crates/lang-rules/tests/support/csharp_together.rs).
//
// csharp_corpus_compiles.rs and csharp_consumer_assembly.rs start Roslyn's
// csc.dll for every case, and the consumer gate a third .NET process that
// runs the consuming assembly; on the CI runner host a share of nine cases took up to
// 19.6 s that way (pipeline 8967, job 229634). Here the compiler of the same
// SDK (Microsoft.CodeAnalysis.CSharp beside csc.dll) compiles each case as
// the command line of the gate does, into memory, and the consumer runs in
// a collectible load context of its own, which is unloaded after the case.
// The gate trusts a run in which every case reports OK; for any other
// outcome it checks every case alone with csc.dll, so that a failure is
// named in the compiler's words.
//
//   dotnet Together.dll <roslyn dir> library <references.rsp> <manifest>
//   dotnet Together.dll <roslyn dir> consumer <references.rsp> <ZeroDds.Wire.dll> <manifest>
//
// <references.rsp> holds one `-r:<assembly>` per line (the reference pack);
// <manifest> one case directory per line. A case directory holds ZdTypes.cs
// (and Consumer.cs for `consumer`); the result goes to result.txt there:
// `OK`, or `FAIL` and what the compiler or the consumer said.

using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Runtime.Loader;
using System.Text;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.Emit;

public static class Together
{
    public static int Main(string[] args)
    {
        if (args.Length < 4)
        {
            Console.Error.WriteLine("usage: Together <roslyn dir> library|consumer <references.rsp> [<runtime>] <manifest>");
            return 2;
        }
        string roslyn = args[0];
        // The compiler's assemblies are beside csc.dll, not beside this one.
        AssemblyLoadContext.Default.Resolving += (context, name) =>
        {
            string candidate = Path.Combine(roslyn, name.Name + ".dll");
            return File.Exists(candidate) ? context.LoadFromAssemblyPath(candidate) : null;
        };
        return Run(args);
    }

    // Not inlined into Main: the compiler's types load when this method is
    // compiled, after the resolver above is in place.
    [MethodImpl(MethodImplOptions.NoInlining)]
    static int Run(string[] args)
    {
        List<MetadataReference> framework = File.ReadAllLines(args[2])
            .Where(line => line.StartsWith("-r:", StringComparison.Ordinal))
            .Select(line => (MetadataReference)MetadataReference.CreateFromFile(line.Substring(3)))
            .ToList();
        switch (args[1])
        {
            case "library":
                foreach (string dir in Cases(args[3]))
                {
                    Write(dir, Guarded(() => Library(dir, framework)));
                }
                return 0;
            case "consumer":
                if (args.Length < 5)
                {
                    Console.Error.WriteLine("consumer: <runtime> and <manifest> are missing");
                    return 2;
                }
                byte[] runtime = File.ReadAllBytes(args[3]);
                foreach (string dir in Cases(args[4]))
                {
                    Write(dir, Guarded(() => Consumer(dir, framework, runtime)));
                }
                return 0;
            default:
                Console.Error.WriteLine("unknown mode " + args[1]);
                return 2;
        }
    }

    static IEnumerable<string> Cases(string manifest)
    {
        return File.ReadAllLines(manifest).Where(line => line.Length > 0);
    }

    static void Write(string dir, string result)
    {
        File.WriteAllText(Path.Combine(dir, "result.txt"), result);
    }

    static string Guarded(Func<string> check)
    {
        try
        {
            return check();
        }
        catch (Exception e)
        {
            return "FAIL\n" + e;
        }
    }

    // csc without /doc parses no documentation comments.
    static SyntaxTree Parse(string path, LanguageVersion version)
    {
        CSharpParseOptions options = CSharpParseOptions.Default
            .WithLanguageVersion(version)
            .WithDocumentationMode(DocumentationMode.None);
        return CSharpSyntaxTree.ParseText(File.ReadAllText(path), options, path, Encoding.UTF8);
    }

    // The image, or null and the compiler's errors in `why`.
    static byte[] Emit(CSharpCompilation compilation, out string why)
    {
        using (MemoryStream image = new MemoryStream())
        {
            EmitResult result = compilation.Emit(image);
            if (result.Success)
            {
                why = "";
                return image.ToArray();
            }
            why = string.Join("\n", result.Diagnostics
                .Where(d => d.Severity == DiagnosticSeverity.Error)
                .Select(d => d.ToString()));
            return null;
        }
    }

    // csharp_corpus_compiles.rs: `-nologo -nostdlib -target:library` and
    // the reference pack; the language version, warning level and
    // optimization are csc's defaults.
    static string Library(string dir, List<MetadataReference> framework)
    {
        CSharpCompilation compilation = CSharpCompilation.Create(
            "ZdTypes",
            new[] { Parse(Path.Combine(dir, "ZdTypes.cs"), LanguageVersion.Default) },
            framework,
            new CSharpCompilationOptions(OutputKind.DynamicallyLinkedLibrary));
        string why;
        return Emit(compilation, out why) != null ? "OK" : "FAIL\n" + why;
    }

    // csharp_consumer_assembly.rs, `compile`: `-nostdlib -noconfig
    // -langversion:12.0 -warnaserror+ -warn:4 -optimize+ -deterministic`;
    // the library references the runtime, the consumer the runtime and the
    // library, and runs.
    static string Consumer(string dir, List<MetadataReference> framework, byte[] runtime)
    {
        CSharpCompilationOptions strict = new CSharpCompilationOptions(
            OutputKind.DynamicallyLinkedLibrary,
            optimizationLevel: OptimizationLevel.Release,
            warningLevel: 4,
            generalDiagnosticOption: ReportDiagnostic.Error,
            deterministic: true);
        List<MetadataReference> withRuntime = new List<MetadataReference>(framework)
        {
            MetadataReference.CreateFromImage(runtime),
        };
        string why;
        byte[] library = Emit(CSharpCompilation.Create(
            "ZdTypes",
            new[] { Parse(Path.Combine(dir, "ZdTypes.cs"), LanguageVersion.CSharp12) },
            withRuntime,
            strict), out why);
        if (library == null)
        {
            return "FAIL\nthe generated library did not compile:\n" + why;
        }
        List<MetadataReference> withLibrary = new List<MetadataReference>(withRuntime)
        {
            MetadataReference.CreateFromImage(library),
        };
        byte[] consumer = Emit(CSharpCompilation.Create(
            "Consumer",
            new[] { Parse(Path.Combine(dir, "Consumer.cs"), LanguageVersion.CSharp12) },
            withLibrary,
            strict.WithOutputKind(OutputKind.ConsoleApplication).WithMainTypeName("Consumer")), out why);
        if (consumer == null)
        {
            return "FAIL\nthe consuming assembly did not compile:\n" + why;
        }
        return RunConsumer(consumer, library, runtime);
    }

    // The consumer's Main in a load context of its own that knows the two
    // assemblies it references; the framework comes from the default one.
    static string RunConsumer(byte[] consumer, byte[] library, byte[] runtime)
    {
        CaseContext context = new CaseContext(new Dictionary<string, byte[]>
        {
            ["ZdTypes"] = library,
            ["ZeroDds.Wire"] = runtime,
        });
        TextWriter output = Console.Out;
        TextWriter error = Console.Error;
        StringWriter said = new StringWriter();
        Console.SetOut(said);
        Console.SetError(said);
        try
        {
            Assembly assembly = context.LoadFromStream(new MemoryStream(consumer));
            MethodInfo main = assembly.EntryPoint;
            object[] arguments = main.GetParameters().Length == 0 ? null : new object[] { new string[0] };
            object code = main.Invoke(null, arguments);
            int exit = code is int number ? number : 0;
            return exit == 0 ? "OK" : "FAIL\nthe consuming assembly answered " + exit + ":\n" + said;
        }
        catch (TargetInvocationException e)
        {
            return "FAIL\nthe consuming assembly threw:\n" + said + e.InnerException;
        }
        finally
        {
            Console.SetOut(output);
            Console.SetError(error);
            context.Unload();
        }
    }

    sealed class CaseContext : AssemblyLoadContext
    {
        readonly Dictionary<string, byte[]> images;

        public CaseContext(Dictionary<string, byte[]> images) : base(isCollectible: true)
        {
            this.images = images;
        }

        protected override Assembly Load(AssemblyName name)
        {
            byte[] image;
            return name.Name != null && images.TryGetValue(name.Name, out image)
                ? LoadFromStream(new MemoryStream(image))
                : null;
        }
    }
}
