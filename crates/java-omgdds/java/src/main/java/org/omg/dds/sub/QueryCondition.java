/* Copyright 2010, Object Management Group, Inc.
 * Copyright 2010, PrismTech, Inc.
 * Copyright 2010, Real-Time Innovations, Inc.
 * All rights reserved.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

package org.omg.dds.sub;

import java.util.List;


/**
 * QueryCondition objects are specialized {@link org.omg.dds.sub.ReadCondition} objects that
 * allow the application to also specify a filter on the locally available
 * data.
 * 
 * The query (queryExpression) is similar to an SQL WHERE clause can be
 * parameterized by arguments that are dynamically changeable by the
 * {@link #setQueryParameters(List)} operation.
 * 
 * This feature is optional. In the cases where it is not supported, the
 * {@link org.omg.dds.sub.DataReader#createQueryCondition(String, List)} will return null.
 * 
 * The triggerValue of a QueryCondition is like that of a ReadCondition with
 * the additional condition that the data associated with at least one sample
 * must be such that the queryExpression evaluates to true.
 * 
 * @param <TYPE>    The concrete type of the data that can be read using the
 *                  the {@link org.omg.dds.sub.DataReader} that created this QueryCondition.
 */
public interface QueryCondition<TYPE> extends ReadCondition<TYPE> {
    /**
     * This operation returns the queryExpression associated with the
     * QueryCondition. That is, the expression specified when the
     * QueryCondition was created.
     * 
     * @see     #getQueryParameters()
     */
    public String getQueryExpression();

    /**
     * This operation returns the queryParameters associated with the
     * QueryCondition. That is, the parameters specified on the last
     * successful call to {@link #setQueryParameters(List)}, or if
     * {@link #setQueryParameters(List)} was never called, the arguments
     * specified when the QueryCondition was created.
     * 
     * @return  an unmodifiable list of the current query parameters.
     * 
     * @see     #setQueryParameters(List)
     * @see     #getQueryExpression()
     */
    public List<String> getQueryParameters();

    /**
     * This operation changes the queryParameters associated with the
     * QueryCondition.
     * 
     * @param   queryParameters a container, into which this method will
     *          place its result.
     * 
     * @see     #getQueryParameters()
     */
    public void setQueryParameters(List<String> queryParameters);
}
