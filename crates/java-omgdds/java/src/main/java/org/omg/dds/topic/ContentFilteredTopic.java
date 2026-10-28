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

package org.omg.dds.topic;

import java.util.List;


/**
 * ContentFilteredTopic is a specialization of TopicDescription that allows
 * for content-based subscriptions.
 * 
 * ContentFilteredTopic describes a more sophisticated subscription that
 * indicates the subscriber does not want to necessarily see all values of
 * each instance published under the {@link org.omg.dds.topic.Topic}. Rather, it wants to see
 * only the values whose contents satisfy certain criteria. This class
 * therefore can be used to request content-based subscriptions.
 * 
 * The selection of the content is done using the filterExpression with
 * parameters expressionParameters.
 * <ul>
 * <li>The filterExpression attribute is a string that specifies the criteria
 *     to select the data samples of interest. It is similar to the WHERE
 *     part of an SQL clause.</li>
 * <li>The expressionParameters attribute is a sequence of strings that give
 *     values to the "parameters" (i.e., "%n" tokens) in the filterExpression.
 *     The number of supplied parameters must fit with the requested values
 *     in the filterExpression (i.e., the number of "%n" tokens).</li>
 * </ul>
 * 
 * @param <TYPE>    The concrete type of the data that will be published and/
 *                  or subscribed by the readers and writers that use this
 *                  topic description.
 */
public interface ContentFilteredTopic<TYPE> extends TopicDescription<TYPE> {
    /**
     * @return  the filter expression associated with the
     *          ContentFilteredTopic, that is, the expression specified when
     *          the ContentFilteredTopic was created.
     */
    public String getFilterExpression();

    /**
     * This operation returns the expression parameters associated with the
     * ContentFilteredTopic, that is, the parameters specified on the last
     * successful call to {@link #setExpressionParameters(List)}, or if
     * {@link #setExpressionParameters(List)} was never called, the
     * parameters specified when the ContentFilteredTopic was created.
     * 
     * @return  an unmodifiable list.
     * 
     * @see     #setExpressionParameters(List)
     */
    public List<String> getExpressionParameters();

    /**
     * This operation changes the expression parameters associated with the
     * ContentFilteredTopic.
     * 
     * @see     #getExpressionParameters()
     */
    public void setExpressionParameters(List<String> expressionParameters);

    /**
     * @return  the {@link org.omg.dds.topic.Topic} associated with the ContentFilteredTopic,
     *          that is, the Topic specified when the ContentFilteredTopic
     *          was created.
     */
    public Topic<? extends TYPE> getRelatedTopic();
}
