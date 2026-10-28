// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
// The OMG header names "vendor_dependent.h" and looks for it beside itself.
#include "../vendor_dependent.h"

// The OMG header declares `class ListenerBase { virtual ~ListenerBase(); };`: with no access
// label the destructor is private, and no class can derive from it and be destroyed. For the
// time the OMG header is read the keyword `virtual` carries a `public:` before it;
// `request_reply.hpp` and `function_call.hpp`, which the OMG headers end with, take it away
// again. Nothing else in the OMG headers is changed.
#ifndef virtual
#define virtual public : virtual
#endif
