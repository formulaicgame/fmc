// Yoinked this from bevy::render::Primitives so the render feature doesn't have to be
// added.

use bevy::math::{DMat3, DVec3};
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::blocks::BlockFace;
use crate::prelude::Transform;

/// An Axis-Aligned Bounding Box
#[derive(Clone, Debug, Default, Reflect, Serialize, Deserialize)]
pub struct Aabb {
    pub center: DVec3,
    pub half_extents: DVec3,
}

impl Aabb {
    #[inline]
    pub fn from_min_max(min: DVec3, max: DVec3) -> Self {
        let min = DVec3::from(min);
        let max = DVec3::from(max);
        let center = 0.5 * (max + min);
        let half_extents = 0.5 * (max - min);
        Self {
            center,
            half_extents,
        }
    }

    /// Calculate the relative radius of the AABB with respect to a plane
    #[inline]
    pub fn relative_radius(&self, p_normal: &DVec3, axes: &[DVec3]) -> f64 {
        // NOTE: dot products on Vec3A use SIMD and even with the overhead of conversion are net faster than Vec3
        let half_extents = self.half_extents;
        DVec3::new(
            p_normal.dot(axes[0]),
            p_normal.dot(axes[1]),
            p_normal.dot(axes[2]),
        )
        .abs()
        .dot(half_extents)
    }

    #[inline]
    pub fn min(&self) -> DVec3 {
        self.center - self.half_extents
    }

    #[inline]
    pub fn max(&self) -> DVec3 {
        self.center + self.half_extents
    }

    pub fn transform(&self, transform: &Transform) -> Self {
        let rot_mat = DMat3::from_quat(transform.rotation);
        // If you rotate a square normally, its aabb will grow larger at 45 degrees because the
        // diagonal the square is longer and pointing in the axis direction. We don't want
        // our aabb to grow larger, we want uniform aabb to stay constant because they are easier
        // to deal with.
        //
        // let abs_rot_mat = DMat3::from_cols(
        //     rot_mat.x_axis.abs(),
        //     rot_mat.y_axis.abs(),
        //     rot_mat.z_axis.abs(),
        // );
        //
        // This is how you do it normally, each column will have a euclidian length of 1. At a 45
        // degree angle around the y axis, this will give an x_axis of
        // [sqrt(2)/2=0.707, 0.0, 0.707], i.e. take 70% of the x extent and 70% of the z
        // extent. We want it to only take 50%. This is done by normalizing it so its total
        // SUM is 1. For visualization this is like rotating it around a "diamond" instead of a
        // circle.
        let abs_rot_mat = DMat3::from_cols(
            rot_mat.x_axis.abs() / rot_mat.x_axis.abs().element_sum(),
            rot_mat.y_axis.abs() / rot_mat.y_axis.abs().element_sum(),
            rot_mat.z_axis.abs() / rot_mat.z_axis.abs().element_sum(),
        );

        Self {
            center: rot_mat * self.center * transform.scale + transform.translation,
            half_extents: abs_rot_mat * self.half_extents * transform.scale,
        }
    }

    pub fn intersection(&self, other: &Self) -> Option<DVec3> {
        let distance = self.center - other.center;
        let overlap = self.half_extents + other.half_extents - distance.abs();

        if overlap.cmpgt(DVec3::ZERO).all() {
            Some(overlap.copysign(distance))
        } else {
            None
        }
    }

    // "Slab method" ray intersection test
    /// Returns distance to intersection and which face was intersected with.
    pub fn ray_intersection(
        &self,
        aabb_transform: &Transform,
        ray_transform: &Transform,
    ) -> Option<(f64, BlockFace)> {
        let after_transform = self.transform(aabb_transform);
        // let mut t_min = f64::NEG_INFINITY;
        // let mut t_max = f64::INFINITY;

        // The writings on this say better speed to do 1 div + n mul than n div
        let direction_reciprocal = 1.0 / ray_transform.forward();

        let t1 = (after_transform.min() - ray_transform.translation) * direction_reciprocal;
        let t2 = (after_transform.max() - ray_transform.translation) * direction_reciprocal;

        let t_min = t1.min(t2).max_element();
        let t_max = t1.max(t2).min_element();

        //for i in 0..3 {
        //    let t1 = (min[i] - origin[i]) * direction_inverse[i];
        //    let t2 = (max[i] - origin[i]) * direction_inverse[i];

        //    t_min = t_min.max(t1.min(t2));
        //    t_max = t_max.min(t1.max(t2));
        //}

        if t_max >= t_min {
            const FACES: [[BlockFace; 2]; 3] = [
                [BlockFace::Right, BlockFace::Left],
                [BlockFace::Top, BlockFace::Bottom],
                [BlockFace::Front, BlockFace::Back],
            ];
            // Move the aabb to be centered at the origin and normalize the point to as if the
            // aabb was of equal side lengths. The axis with the highest absolute value will then
            // be the axis of the face that was intersected. Since the aabb is at the origin we can
            // use the sign to determine which of the two faces it was.
            let point = ray_transform.translation + ray_transform.forward() * t_min;
            let normalized = (point - after_transform.center) / after_transform.half_extents;
            let abs = normalized.abs();
            let axis = abs.cmpeq(DVec3::splat(abs.max_element()));
            // bitmask() gives a u32 with bit layout 0bzyx, is always numbers 1, 2, 4 since only
            // one is ever true.
            let index_1 = (axis.bitmask() / 2) as usize;
            let index_2 = normalized[index_1].is_sign_negative() as usize;
            let face = FACES[index_1][index_2];

            return Some((t_min, face));
        } else {
            return None;
        }
    }
}

//impl From<Sphere> for Aabb {
//    #[inline]
//    fn from(sphere: Sphere) -> Self {
//        Self {
//            center: sphere.center,
//            half_extents: Vec3A::splat(sphere.radius),
//        }
//    }
//}
//
//#[derive(Clone, Debug, Default)]
//pub struct Sphere {
//    pub center: Vec3A,
//    pub radius: f32,
//}
//
//impl Sphere {
//    #[inline]
//    pub fn intersects_obb(&self, aabb: &Aabb, local_to_world: &Mat4) -> bool {
//        let aabb_center_world = *local_to_world * aabb.center.extend(1.0);
//        let axes = [
//            Vec3A::from(local_to_world.x_axis),
//            Vec3A::from(local_to_world.y_axis),
//            Vec3A::from(local_to_world.z_axis),
//        ];
//        let v = Vec3A::from(aabb_center_world) - self.center;
//        let d = v.length();
//        let relative_radius = aabb.relative_radius(&(v / d), &axes);
//        d < self.radius + relative_radius
//    }
//}
