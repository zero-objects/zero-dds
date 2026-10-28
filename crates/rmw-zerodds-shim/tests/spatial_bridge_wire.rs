//! The wire form of the spatial bridge's wrapper types.
//!
//! Measured on the Linux test host (`zerodds-examples/rmw-spatial-bridge`, ROS 2 Jazzy,
//! `rmw_zerodds_cpp`): the bridge's `PoseStamped` of position (1, 2, 3) and
//! frame `map` arrived at a ROS subscriber as (1.5e-306, 5.9e-313, 0) with an
//! empty frame. The generated types are `@appendable` and their XCDR2 codec
//! writes a DHEADER, while the wrappers declared `@final`: the encapsulation
//! said PLAIN_CDR2, and the RMW read every member four octets early. And a
//! ROS 2 middleware writes ROS messages as XCDR1 (`CDR_LE`), which the
//! wrappers could not read or write at all.

#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

use rmw_zerodds::spatial_bridge::{BridgeFramedPose, BridgePoseStamped};
use zerodds_dcps::DdsType;
use zerodds_dcps::dds_type::Extensibility;
use zerodds_spatial_ros2::{builtin_interfaces, geometry_msgs, std_msgs};

fn pose_stamped() -> BridgePoseStamped {
    BridgePoseStamped(geometry_msgs::msg::PoseStamped {
        header: std_msgs::msg::Header {
            stamp: builtin_interfaces::msg::Time { sec: 7, nanosec: 9 },
            frame_id: "map".to_string(),
        },
        pose: geometry_msgs::msg::Pose {
            position: geometry_msgs::msg::Point {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
            orientation: geometry_msgs::msg::Quaternion {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
        },
    })
}

/// A DHEADER is the first word, and it counts the octets after it.
fn starts_with_its_dheader(body: &[u8]) -> bool {
    body.len() >= 4 && u32::from_le_bytes(body[..4].try_into().unwrap()) as usize == body.len() - 4
}

#[test]
fn the_xcdr2_form_is_what_the_declared_extensibility_says() {
    assert_eq!(BridgePoseStamped::EXTENSIBILITY, Extensibility::Appendable);
    assert_eq!(BridgeFramedPose::EXTENSIBILITY, Extensibility::Appendable);
    let mut ros = Vec::new();
    pose_stamped().encode(&mut ros).unwrap();
    assert!(starts_with_its_dheader(&ros), "{ros:02x?}");
    let mut spatial = Vec::new();
    BridgeFramedPose::default().encode(&mut spatial).unwrap();
    assert!(starts_with_its_dheader(&spatial), "{spatial:02x?}");
}

/// XCDR1 has no DHEADER and aligns a double to eight: stamp (8 octets),
/// `frame_id` as length 4 and `map\0` (8 octets), then the seven doubles of
/// the pose from offset 16 — the layout `rmw_zerodds_cpp` reads.
#[test]
fn a_pose_stamped_goes_out_and_comes_back_as_xcdr1() {
    let sample = pose_stamped();
    let mut bytes = Vec::new();
    sample.encode_xcdr1(&mut bytes).unwrap();
    assert_eq!(bytes.len(), 16 + 7 * 8, "{bytes:02x?}");
    assert_eq!(&bytes[..8], &[7, 0, 0, 0, 9, 0, 0, 0]);
    assert_eq!(&bytes[8..16], &[4, 0, 0, 0, b'm', b'a', b'p', 0]);
    let x = f64::from_le_bytes(bytes[16..24].try_into().unwrap());
    let w = f64::from_le_bytes(bytes[64..72].try_into().unwrap());
    assert_eq!((x, w), (1.0, 1.0));
    assert_eq!(BridgePoseStamped::decode_xcdr1(&bytes).unwrap(), sample);
}
