package org.omg.dds.type.dynamic;

import org.omg.dds.core.ServiceEnvironment;


public abstract class DynamicDataFactory implements org.omg.dds.core.DDSObject {
	
	/**
     * @param env       Identifies the Service instance to which the
     *                  object will belong.
     */
    public static DynamicDataFactory getInstance(ServiceEnvironment env)
    {
        return env.getSPI().getDynamicDataFactory();
    }
    
    public abstract DynamicData newDynamicData(DynamicType type); 
}
