<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS — C# (binding async surface)

The C# binding (`ZeroDDS`) has an idiomatic modern-C# async surface in
[`Async.cs`](../../crates/cs/csharp/ZeroDDS/src/Async.cs): `Task`-returning
extension methods plus an `IAsyncEnumerable` sample stream, all with
`CancellationToken` support.

## Surface

```csharp
// wait — Task<bool>, honours the CancellationToken
await reader.WaitForDataAsync(TimeSpan.FromSeconds(2), ct);

// take — async stream of samples; cancellation aborts the enumeration
await foreach (var sample in reader.TakeAsync(TimeSpan.FromSeconds(2), ct))
    Handle(sample.Data);

// write — Task
await writer.WriteAsync(sample, ct);
```

Extension methods over the synchronous `DataReader<T>`/`DataWriter<T>`, so the
async surface layers on without duplicating handle state. Covered by the
`ZeroDDS.Tests` suite.
