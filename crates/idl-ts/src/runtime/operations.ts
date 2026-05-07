// SPDX-License-Identifier: Apache-2.0
// DDS-TS 1.0 — Operations-Profile descriptor types (Chapter 11).

import type { DdsTypeDescriptor, DdsTypeRef } from "./types.js";

/** Parameter direction in an IDL operation signature. */
export type ParameterMode = "in" | "out" | "inout";

/** Per-parameter metadata for an operation. */
export interface OperationParameterDescriptor {
  readonly name: string;
  readonly mode: ParameterMode;
  readonly type: DdsTypeRef;
}

/** Per-operation metadata. `returnType` is `{ kind: "void" }` for
 *  `void`-returning operations. `oneway` reflects the IDL `oneway`
 *  marker. `raises` holds the descriptors of declared exceptions. */
export interface OperationDescriptor {
  readonly name:        string;
  readonly oneway:      boolean;
  readonly returnType:  DdsTypeRef | { kind: "void" };
  readonly parameters:  ReadonlyArray<OperationParameterDescriptor>;
  readonly raises:      ReadonlyArray<DdsTypeDescriptor>;
}

/** Per-attribute metadata. */
export interface AttributeDescriptor {
  readonly name:       string;
  readonly readonly:   boolean;
  readonly type:       DdsTypeRef;
  readonly getRaises:  ReadonlyArray<DdsTypeDescriptor>;
  readonly setRaises:  ReadonlyArray<DdsTypeDescriptor>;
}

/** Top-level service descriptor for an IDL `interface`. */
export interface ServiceDescriptor<TClient, THandler> {
  readonly name:       string;
  readonly inherits:   ReadonlyArray<ServiceDescriptor<unknown, unknown>>;
  readonly operations: ReadonlyArray<OperationDescriptor>;
  readonly attributes: ReadonlyArray<AttributeDescriptor>;
  // Phantom storage so that TClient and THandler appear in the
  // type — they are erased at runtime but enforce client/handler
  // pairing at compile time.
  readonly __client?:  (c: TClient) => void;
  readonly __handler?: (h: THandler) => void;
}
