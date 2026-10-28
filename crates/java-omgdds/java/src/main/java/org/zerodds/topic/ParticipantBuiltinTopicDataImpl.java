// SPDX-License-Identifier: Apache-2.0
package org.zerodds.topic;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.policy.UserData;
import org.omg.dds.topic.BuiltinTopicKey;
import org.omg.dds.topic.ParticipantBuiltinTopicData;

/**
 * ZeroDDS {@link ParticipantBuiltinTopicData} (DDS 1.2 §2.2.1.3.1,
 * DDSI-RTPS 2.5 §9.3.1.2): what the "DCPSParticipant" built-in topic
 * carries — the key of the discovered participant and its USER_DATA.
 */
public final class ParticipantBuiltinTopicDataImpl
implements ParticipantBuiltinTopicData {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private BuiltinTopicKey key;
    private UserData userData;

    public ParticipantBuiltinTopicDataImpl(ServiceEnvironment environment,
            BuiltinTopicKey key, UserData userData) {
        this.environment = environment;
        this.key = key;
        this.userData = userData;
    }

    @Override
    public BuiltinTopicKey getKey() {
        return key;
    }

    public void setKey(BuiltinTopicKey key) {
        this.key = key;
    }

    @Override
    public UserData getUserData() {
        return userData;
    }

    public void setUserData(UserData userData) {
        this.userData = userData;
    }

    @Override
    public void copyFrom(ParticipantBuiltinTopicData src) {
        this.key = src.getKey().clone();
        this.userData = src.getUserData();
    }

    @Override
    public ParticipantBuiltinTopicData clone() {
        return new ParticipantBuiltinTopicDataImpl(environment, key.clone(),
                userData);
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public String toString() {
        return "ParticipantBuiltinTopicData[" + key + "]";
    }

    /**
     * Java serialization (DDS-Java-PSM 1.0 §7.3.3) carries the value, not its
     * {@link ServiceEnvironment}: an environment is a runtime object and is not
     * serializable. A deserialized object belongs to the default environment of
     * this implementation.
     */
    private void readObject(java.io.ObjectInputStream in)
            throws java.io.IOException, ClassNotFoundException {
        in.defaultReadObject();
        environment = ServiceEnvironmentImpl.getDefaultEnvironment();
    }
}
