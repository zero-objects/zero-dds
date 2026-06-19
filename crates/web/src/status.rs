// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `ReturnStatus` + HTTP-Status-Mapping — Spec §7.3 + §8.3.2.

/// Spec §7.3 — `ReturnStatus`-Codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReturnStatus {
    /// `OK` — context-dependent HTTP-Status (siehe `http_status_for`).
    Ok,
    /// `OBJECT_ALREADY_EXISTS` → `409 Conflict`.
    ObjectAlreadyExists,
    /// `INVALID_INPUT` → `422 Unprocessable Entity`.
    InvalidInput,
    /// `INVALID_OBJECT` → `404 Not Found`.
    InvalidObject,
    /// `ACCESS_DENIED` → `401 Unauthorized`.
    AccessDenied,
    /// `PERMISSIONS_ERROR` → `403 Forbidden`.
    PermissionsError,
    /// `GENERIC_SERVICE_ERROR` → `500 Internal Server Error`.
    GenericServiceError,
    /// `DDS_ERROR` → `500 Internal Server Error`.
    DdsError,
}

/// Spec §8.3.2 + Tab in §8.3 — HTTP-Status-Code-Mapping.
///
/// `Ok` depends on the HTTP method:
/// * POST (create) → 201 Created.
/// * DELETE → 204 No Content.
/// * GET → 200 OK.
/// * PUT (update) → 204 No Content.
#[must_use]
pub const fn http_status_for(status: ReturnStatus, method: crate::rest::RestMethod) -> u16 {
    use crate::rest::RestMethod;
    match status {
        ReturnStatus::Ok => match method {
            RestMethod::Post => 201,
            RestMethod::Delete | RestMethod::Put => 204,
            RestMethod::Get | RestMethod::Head => 200,
        },
        ReturnStatus::ObjectAlreadyExists => 409,
        ReturnStatus::InvalidInput => 422,
        ReturnStatus::InvalidObject => 404,
        ReturnStatus::AccessDenied => 401,
        ReturnStatus::PermissionsError => 403,
        ReturnStatus::GenericServiceError | ReturnStatus::DdsError => 500,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rest::RestMethod;

    #[test]
    fn ok_maps_to_201_for_post_create() {
        // Spec §8.3.2 — Create operations → 201 Created.
        assert_eq!(http_status_for(ReturnStatus::Ok, RestMethod::Post), 201);
    }

    #[test]
    fn ok_maps_to_204_for_delete() {
        // Spec — DELETE → 204 No Content.
        assert_eq!(http_status_for(ReturnStatus::Ok, RestMethod::Delete), 204);
    }

    #[test]
    fn ok_maps_to_200_for_get() {
        assert_eq!(http_status_for(ReturnStatus::Ok, RestMethod::Get), 200);
    }

    #[test]
    fn ok_maps_to_204_for_put_update() {
        // Spec — PUT update → 204 No Content.
        assert_eq!(http_status_for(ReturnStatus::Ok, RestMethod::Put), 204);
    }

    #[test]
    fn object_already_exists_maps_to_409() {
        // Spec.
        assert_eq!(
            http_status_for(ReturnStatus::ObjectAlreadyExists, RestMethod::Post),
            409
        );
    }

    #[test]
    fn invalid_input_maps_to_422() {
        // Spec — RFC 4918 422 Unprocessable Entity.
        assert_eq!(
            http_status_for(ReturnStatus::InvalidInput, RestMethod::Post),
            422
        );
    }

    #[test]
    fn invalid_object_maps_to_404() {
        assert_eq!(
            http_status_for(ReturnStatus::InvalidObject, RestMethod::Get),
            404
        );
    }

    #[test]
    fn access_denied_maps_to_401() {
        assert_eq!(
            http_status_for(ReturnStatus::AccessDenied, RestMethod::Get),
            401
        );
    }

    #[test]
    fn permissions_error_maps_to_403() {
        assert_eq!(
            http_status_for(ReturnStatus::PermissionsError, RestMethod::Post),
            403
        );
    }

    #[test]
    fn generic_service_error_maps_to_500() {
        assert_eq!(
            http_status_for(ReturnStatus::GenericServiceError, RestMethod::Get),
            500
        );
    }

    #[test]
    fn dds_error_maps_to_500() {
        assert_eq!(
            http_status_for(ReturnStatus::DdsError, RestMethod::Post),
            500
        );
    }
}
