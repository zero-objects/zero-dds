// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.core.PreconditionNotMetException;
import org.omg.dds.type.dynamic.DynamicData;
import org.omg.dds.type.dynamic.DynamicDataFactory;
import org.omg.dds.type.dynamic.DynamicType;

/**
 * ZeroDDS {@link DynamicDataFactory} (DDS-Java-PSM 1.0 §7.8.1.1, §7.8.1.4).
 */
public final class DynamicDataFactoryImpl extends DynamicDataFactory {

    private final ServiceEnvironment environment;

    public DynamicDataFactoryImpl(ServiceEnvironment environment) {
        this.environment = environment;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public DynamicData newDynamicData(DynamicType type) {
        if (type == null) {
            throw new PreconditionNotMetException("a DynamicType must not be null");
        }
        return new DynamicDataImpl(environment, type);
    }
}