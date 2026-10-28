// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

/**
 * The Operations-Profile descriptor types — DDS-TS 1.0 §11.5.
 *
 * Chapter 8's descriptors describe *data*: a struct, a union, an
 * enumeration. These describe *calls*: what an IDL `interface` declares,
 * in the same side-table form and referring to the same
 * {@link DdsTypeRef} graph, so one walk reads both.
 *
 * The generator emits one `ServiceDescriptor` constant per IDL
 * interface, named `<IfaceName>Service`, beside the `<IfaceName>Client`
 * and `<IfaceName>Handler` interfaces (§11.1). No class is emitted:
 * "Concrete client construction is the responsibility of the runtime …
 * which produces objects satisfying the client interface from the
 * Service Descriptor."
 *
 * ## Why the shapes are not classes here either
 *
 * Every member is `readonly` and every collection is a
 * `ReadonlyArray`. A descriptor is a value the generator writes once and
 * every consumer reads; a mutable one would let a caller edit the
 * metadata another caller is dispatching against, and nothing would
 * report it.
 *
 * ## Where the exception narrowing lives
 *
 * §11.4 sends a caught rejection to `isOneOf`, which chapter 8 §8.7
 * defines and `registry.ts` exports. It is not re-exported here: two
 * modules exporting one name through `export *` is a name the barrel
 * silently drops, and one definition of a type guard is the point of
 * having it.
 */

import type { DdsTypeDescriptor, DdsTypeRef } from "./types.js";

/**
 * How a parameter is passed. DDS-TS 1.0 §11.5.
 *
 * The three IDL directions of grammar rule (87), spelled as the IDL
 * spells them. `in` and `inout` reach the method's parameter list;
 * `out` and `inout` reach the resolved value (§11.2).
 */
export type ParameterMode = "in" | "out" | "inout";

/**
 * One parameter of one operation. DDS-TS 1.0 §11.5.
 */
export interface OperationParameterDescriptor {
  /** The IDL parameter name, as declared. */
  readonly name: string;
  /** `in`, `out` or `inout`. */
  readonly mode: ParameterMode;
  /** What the parameter holds. */
  readonly type: DdsTypeRef;
}

/**
 * One operation of an interface. DDS-TS 1.0 §11.5.
 *
 * `returnType` is a {@link DdsTypeRef} or the marker `{ kind: "void" }`.
 * A marker and not `undefined`: an absent property and a `void` return
 * are different facts, and under `exactOptionalPropertyTypes` a consumer
 * would have to know which one an older generator wrote.
 *
 * `parameters` holds **every** parameter in declaration order, including
 * the `out` ones the client method does not take. The descriptor
 * describes the IDL operation; the method signature is one projection of
 * it, and a descriptor that carried only the projection could not
 * reconstruct the reply.
 */
export interface OperationDescriptor {
  /** The IDL operation name, as declared. */
  readonly name: string;
  /** Whether the IDL wrote `oneway`. */
  readonly oneway: boolean;
  /** The declared return, or the `void` marker. */
  readonly returnType: DdsTypeRef | { kind: "void" };
  /** Every parameter, in declaration order. */
  readonly parameters: ReadonlyArray<OperationParameterDescriptor>;
  /** The descriptors of the exceptions the `raises` clause names. */
  readonly raises: ReadonlyArray<DdsTypeDescriptor>;
}

/**
 * One attribute of an interface. DDS-TS 1.0 §11.5.
 *
 * The client and handler interfaces carry `get_<name>` and, unless
 * `readonly`, `set_<name>` (§11.3). `getRaises` and `setRaises` are the
 * `getraises` and `setraises` clauses of IDL 4.2 grammar rule (89),
 * which have nowhere else to go: neither reaches a method signature,
 * because §11.4 rejects the promise rather than declaring a `throws`.
 */
export interface AttributeDescriptor {
  /** The IDL attribute name, as declared. */
  readonly name: string;
  /** Whether the IDL wrote `readonly`. */
  readonly readonly: boolean;
  /** What the attribute holds. */
  readonly type: DdsTypeRef;
  /** The descriptors of the exceptions `getraises` names. */
  readonly getRaises: ReadonlyArray<DdsTypeDescriptor>;
  /** The descriptors of the exceptions `setraises` names. */
  readonly setRaises: ReadonlyArray<DdsTypeDescriptor>;
}

/**
 * One IDL interface, as the metadata a runtime dispatches on.
 * DDS-TS 1.0 §11.5.
 *
 * `TClient` and `THandler` appear in the type and in neither member.
 * That is §11.5's shape and it is deliberate: the parameters tie a
 * descriptor constant to the two interfaces the same generator run
 * emitted, so a runtime factory typed
 * `(d: ServiceDescriptor<C, H>) => C` hands back the client of *this*
 * service rather than an `unknown`.
 *
 * `inherits` reflects the IDL inheritance specification (grammar rule
 * (78)) in declaration order and nothing is flattened into the child:
 * §11.6 says so outright, and the client interface already `extends` the
 * parents', so a flattened copy would be a second, divergent statement
 * of the same operation set.
 */
export interface ServiceDescriptor<TClient, THandler> {
  /** The qualified IDL name — `robot::Counter`, not `Counter`. */
  readonly name: string;
  /** The parent services, in declaration order. */
  readonly inherits: ReadonlyArray<ServiceDescriptor<unknown, unknown>>;
  /** The operations this interface declares itself. */
  readonly operations: ReadonlyArray<OperationDescriptor>;
  /** The attributes this interface declares itself. */
  readonly attributes: ReadonlyArray<AttributeDescriptor>;
}

/**
 * The two type parameters, kept reachable for a consumer that has a
 * descriptor and wants the interfaces it was emitted with.
 *
 * `ClientOf<typeof CounterService>` is `CounterClient`. Without these a
 * caller would have to name both halves again at every call site, which
 * is the drift `ServiceDescriptor`'s parameters exist to prevent.
 */
export type ClientOf<D> =
  D extends ServiceDescriptor<infer TClient, unknown> ? TClient : never;

/** The handler half of a service descriptor. See {@link ClientOf}. */
export type HandlerOf<D> =
  D extends ServiceDescriptor<unknown, infer THandler> ? THandler : never;

/**
 * Whether an operation's reply carries more than a single value.
 *
 * §11.2: an operation that declares `out` or `inout` parameters resolves
 * to an object literal carrying the return under `result` and one
 * property per `out`/`inout` parameter. A runtime building the resolved
 * value asks this before deciding which shape to construct, and asking
 * it here rather than at each call site keeps one reading of the rule.
 */
export function hasAggregatedResult(operation: OperationDescriptor): boolean {
  return operation.parameters.some(
    (parameter) => parameter.mode === "out" || parameter.mode === "inout",
  );
}

/**
 * The parameters a client method takes, in declaration order.
 *
 * §11.2: "parameter list is composed of the IDL `in` and `inout`
 * parameters, in declaration order".
 */
export function inputParameters(
  operation: OperationDescriptor,
): ReadonlyArray<OperationParameterDescriptor> {
  return operation.parameters.filter(
    (parameter) => parameter.mode === "in" || parameter.mode === "inout",
  );
}

/**
 * The properties the resolved value carries beside `result`, in
 * declaration order.
 *
 * §11.2: one property per `out`/`inout` parameter, named after the IDL
 * parameter.
 */
export function outputParameters(
  operation: OperationDescriptor,
): ReadonlyArray<OperationParameterDescriptor> {
  return operation.parameters.filter(
    (parameter) => parameter.mode === "out" || parameter.mode === "inout",
  );
}

/**
 * An operation of a service or of any service it inherits from.
 *
 * §11.6 keeps the descriptor's `operations` array to what the interface
 * declares itself, so a lookup by name has to walk `inherits`. Depth
 * first and in declaration order, which is the order CORBA multiple
 * inheritance resolves a name in; the first match wins, so a redeclared
 * operation resolves to the most derived one.
 *
 * Returns `undefined` for a name no interface in the chain declares.
 */
export function findOperation(
  service: ServiceDescriptor<unknown, unknown>,
  name: string,
): OperationDescriptor | undefined {
  const own = service.operations.find((operation) => operation.name === name);
  if (own !== undefined) {
    return own;
  }
  for (const base of service.inherits) {
    const inherited = findOperation(base, name);
    if (inherited !== undefined) {
      return inherited;
    }
  }
  return undefined;
}

/**
 * An attribute of a service or of any service it inherits from.
 * See {@link findOperation} for the resolution order.
 */
export function findAttribute(
  service: ServiceDescriptor<unknown, unknown>,
  name: string,
): AttributeDescriptor | undefined {
  const own = service.attributes.find((attribute) => attribute.name === name);
  if (own !== undefined) {
    return own;
  }
  for (const base of service.inherits) {
    const inherited = findAttribute(base, name);
    if (inherited !== undefined) {
      return inherited;
    }
  }
  return undefined;
}
