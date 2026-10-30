//! Typed refusals. Every refusal carries a stable Kiln code and maps onto one
//! XRPC error envelope; the class decides whether a knot retries.

use kiln_provider_api::PortFailure;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RejectionClass {
    /// 400 `InvalidRequest`: malformed input; the knot drops the event.
    InvalidRequest,
    /// 401 `AuthenticationRequired`: missing or unverifiable credentials.
    Authentication,
    /// 403 `Forbidden`: authenticated but not permitted.
    Forbidden,
    /// 404 with a lexicon error name such as `RepoNotFound`.
    NotFound(&'static str),
    /// 400 with a lexicon error name such as `PipelineNotBlocked`.
    Precondition(&'static str),
    /// 422: final refusal of well-formed input a retry cannot fix.
    Unprocessable,
    /// 503: uncertain or busy; the knot retries.
    Unavailable,
    /// 500: durable state is inconsistent.
    Internal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rejection {
    pub class: RejectionClass,
    pub code: &'static str,
    pub message: &'static str,
}

pub type TangledResult<T> = Result<T, Rejection>;

impl Rejection {
    pub const fn new(class: RejectionClass, code: &'static str, message: &'static str) -> Self {
        Self {
            class,
            code,
            message,
        }
    }

    pub const fn invalid(code: &'static str, message: &'static str) -> Self {
        Self::new(RejectionClass::InvalidRequest, code, message)
    }

    pub const fn authentication(code: &'static str, message: &'static str) -> Self {
        Self::new(RejectionClass::Authentication, code, message)
    }

    pub const fn forbidden(code: &'static str, message: &'static str) -> Self {
        Self::new(RejectionClass::Forbidden, code, message)
    }

    pub const fn unavailable(code: &'static str, message: &'static str) -> Self {
        Self::new(RejectionClass::Unavailable, code, message)
    }

    pub const fn internal(code: &'static str, message: &'static str) -> Self {
        Self::new(RejectionClass::Internal, code, message)
    }

    /// HTTP status and XRPC error name.
    pub const fn status(&self) -> (u16, &'static str) {
        match self.class {
            RejectionClass::InvalidRequest => (400, "InvalidRequest"),
            RejectionClass::Authentication => (401, "AuthenticationRequired"),
            RejectionClass::Forbidden => (403, "Forbidden"),
            RejectionClass::NotFound(name) => (404, name),
            RejectionClass::Precondition(name) => (400, name),
            RejectionClass::Unprocessable => (422, "InvalidRequest"),
            RejectionClass::Unavailable => (503, "ServiceUnavailable"),
            RejectionClass::Internal => (500, "InternalServerError"),
        }
    }

    /// A shell port failure. Domain failures are final; every other failure,
    /// including `Unknown`, is reported as retryable unavailability so the knot
    /// redelivers and replay protection decides.
    pub fn from_port(failure: &PortFailure, code: &'static str, message: &'static str) -> Self {
        match failure {
            PortFailure::Domain(_) => Self::new(RejectionClass::Unprocessable, code, message),
            PortFailure::Infrastructure(_)
            | PortFailure::Retryable(_)
            | PortFailure::TimedOut(_)
            | PortFailure::Cancelled(_)
            | PortFailure::Unknown(_) => Self::unavailable(code, message),
        }
    }
}

pub(crate) fn port_code(failure: &PortFailure) -> &str {
    match failure {
        PortFailure::Domain(error) => &error.code,
        PortFailure::Infrastructure(error)
        | PortFailure::Retryable(error)
        | PortFailure::TimedOut(error)
        | PortFailure::Cancelled(error)
        | PortFailure::Unknown(error) => &error.code,
    }
}

pub(crate) fn infrastructure(code: &str, message: &str) -> PortFailure {
    PortFailure::Infrastructure(kiln_provider_api::InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    })
}

pub(crate) fn unknown(code: &str, message: &str) -> PortFailure {
    PortFailure::Unknown(kiln_provider_api::InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    })
}

pub(crate) fn retryable(code: &str, message: &str) -> PortFailure {
    PortFailure::Retryable(kiln_provider_api::InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    })
}

pub(crate) fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(kiln_core::types::Diagnostic::new(code, field, message))
}
