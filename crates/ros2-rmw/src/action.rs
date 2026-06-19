// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! ROS-2 Action-Pattern (5-Topic-Set).
//!
//! Spec: `zerodds-ros2-bridge-1.0.md` §4.3 (= REP-2003 §3.5).
//!
//! A ROS-2 action `/foo/bar` with `Foo.action` is mapped to **five**
//! DDS topics:
//! * **goal**:      `rq/<base>/_action/send_goalRequest` (Reliable+KeepLast)
//! * **cancel**:    `rq/<base>/_action/cancel_goalRequest` (Reliable+KeepLast)
//! * **result**:    `rr/<base>/_action/get_resultReply` (Reliable+KeepLast)
//! * **feedback**:  `rt/<base>/_action/feedback` (Reliable+KeepLast)
//! * **status**:    `rt/<base>/_action/status` (Reliable+TransientLocal)
//!
//! Plus: `goal_id` (16-byte UUID) correlates the five topics.

use alloc::string::String;

/// Topic-name set for a ROS-2 action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionTopics {
    /// `rq/<base>/_action/send_goalRequest`-Topic.
    pub goal: String,
    /// `rq/<base>/_action/cancel_goalRequest`-Topic.
    pub cancel: String,
    /// `rr/<base>/_action/get_resultReply`-Topic.
    pub result: String,
    /// `rt/<base>/_action/feedback`-Topic.
    pub feedback: String,
    /// `rt/<base>/_action/status`-Topic.
    pub status: String,
}

impl ActionTopics {
    /// Generates all five topic names from the ROS-2 action path.
    /// Spec §4.3.
    #[must_use]
    pub fn from_action(ros_action_name: &str) -> Self {
        let base = ros_action_name.trim_start_matches('/');
        Self {
            goal: format!("rq/{base}/_action/send_goalRequest"),
            cancel: format!("rq/{base}/_action/cancel_goalRequest"),
            result: format!("rr/{base}/_action/get_resultReply"),
            feedback: format!("rt/{base}/_action/feedback"),
            status: format!("rt/{base}/_action/status"),
        }
    }

    /// Number of topics — exactly 5 per Spec §4.3.
    #[must_use]
    pub const fn count(&self) -> usize {
        5
    }

    /// Iterates all 5 topic names in a fixed order.
    #[must_use]
    pub fn all_topics(&self) -> [&str; 5] {
        [
            &self.goal,
            &self.cancel,
            &self.result,
            &self.feedback,
            &self.status,
        ]
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn topic_set_has_exactly_5_topics() {
        let a = ActionTopics::from_action("/turtle1/rotate_absolute");
        assert_eq!(a.count(), 5);
        assert_eq!(a.all_topics().len(), 5);
    }

    #[test]
    fn goal_topic_uses_rq_prefix_and_send_goal_suffix() {
        let a = ActionTopics::from_action("/turtle1/rotate_absolute");
        assert_eq!(
            a.goal,
            "rq/turtle1/rotate_absolute/_action/send_goalRequest"
        );
    }

    #[test]
    fn cancel_topic_uses_rq_prefix_and_cancel_suffix() {
        let a = ActionTopics::from_action("/turtle1/rotate_absolute");
        assert_eq!(
            a.cancel,
            "rq/turtle1/rotate_absolute/_action/cancel_goalRequest"
        );
    }

    #[test]
    fn result_topic_uses_rr_prefix_and_reply_suffix() {
        let a = ActionTopics::from_action("/turtle1/rotate_absolute");
        assert_eq!(
            a.result,
            "rr/turtle1/rotate_absolute/_action/get_resultReply"
        );
    }

    #[test]
    fn feedback_and_status_use_rt_prefix() {
        let a = ActionTopics::from_action("/x");
        assert!(a.feedback.starts_with("rt/"));
        assert!(a.status.starts_with("rt/"));
    }

    #[test]
    fn handles_action_name_without_leading_slash() {
        let a = ActionTopics::from_action("turtle1/rotate_absolute");
        assert!(a.goal.starts_with("rq/turtle1/"));
    }
}
