#![allow(
    unknown_lints,
    non_trait_imports,
    reason = "the narrow service composition keeps each core admission and wire type visible at handler boundaries"
)]

use crate::AdmissionError;
use crate::ExchangeCommand;
use crate::ExchangeRequest;
use crate::ExchangeResponse;
use crate::ExchangeResult;
use crate::ExchangeSuccess;
use crate::HandlerError;
use crate::Rejection;
use crate::RejectionCode;
use crate::new_response;
use crate::validate_exchange_request;
use crate::validate_exchange_response;

// r[impl lattice.workflow_interchange.transport_handler]
pub trait WorkflowExchangeHandler {
    fn handle(&mut self, command: &ExchangeCommand) -> Result<ExchangeSuccess, HandlerError>;
}

#[derive(Debug)]
pub struct WorkflowExchangeService<H> {
    handler: H,
}

impl<H: WorkflowExchangeHandler> WorkflowExchangeService<H> {
    pub const fn new(handler: H) -> Self {
        Self { handler }
    }

    #[must_use]
    pub const fn handler(&self) -> &H {
        &self.handler
    }

    pub const fn handler_mut(&mut self) -> &mut H {
        &mut self.handler
    }

    #[must_use]
    pub fn into_handler(self) -> H {
        self.handler
    }

    pub fn exchange(&mut self, request: &ExchangeRequest) -> Result<ExchangeResponse, AdmissionError> {
        if let Err(error) = validate_exchange_request(request) {
            return rejected(request, rejection_for_admission(error));
        }
        let result = match self.handler.handle(&request.command) {
            Ok(success) => ExchangeResult::Success(Box::new(success)),
            Err(error) => ExchangeResult::Rejected(map_rejection(error)),
        };
        let candidate = new_response(request.identity.clone(), result)?;
        if validate_exchange_response(request, &candidate).is_ok() {
            return Ok(candidate);
        }
        rejected(request, Rejection {
            code: RejectionCode::InvalidOperationResult,
            field: "exchange.result".to_string(),
        })
    }
}

fn rejected(request: &ExchangeRequest, rejection: Rejection) -> Result<ExchangeResponse, AdmissionError> {
    new_response(request.identity.clone(), ExchangeResult::Rejected(rejection))
}

// Closed safe-wire mapping: each internal class has one stable public code and no payload text.
#[allow(
    unknown_lints,
    assertion_density,
    fragile_exhaustive_enum_match,
    missing_const_fn,
    reason = "the exhaustive mapping is a compatibility table and allocates the bounded field string"
)]
fn rejection_for_admission(error: AdmissionError) -> Rejection {
    let (code, field) = match error {
        AdmissionError::ProfileSchema => (RejectionCode::InvalidSchema, "transport.profile"),
        AdmissionError::ProfileLimit => (RejectionCode::FrameTooLarge, "transport.limits"),
        AdmissionError::PeerPolicy => (RejectionCode::PeerDenied, "transport.peer"),
        AdmissionError::RequestSchema => (RejectionCode::InvalidSchema, "exchange.schema"),
        AdmissionError::RequestIdentity => (RejectionCode::InvalidIdentity, "exchange.identity"),
        AdmissionError::Contract => (RejectionCode::InvalidContract, "exchange.contract"),
        AdmissionError::ResponseSchema
        | AdmissionError::ResponseIdentity
        | AdmissionError::ResponseRequest
        | AdmissionError::OperationResult => (RejectionCode::InvalidOperationResult, "exchange.result"),
        AdmissionError::LogBound | AdmissionError::LogLinkage => (RejectionCode::LogTooLarge, "exchange.log"),
        AdmissionError::Encoding => (RejectionCode::EncodingFailure, "exchange.encoding"),
    };
    Rejection {
        code,
        field: field.to_string(),
    }
}

// Closed handler mapping: transport callers receive only reviewed failure classes.
#[allow(
    unknown_lints,
    fragile_exhaustive_enum_match,
    missing_const_fn,
    reason = "the exhaustive mapping is a compatibility table and allocates the bounded field string"
)]
fn map_rejection(error: HandlerError) -> Rejection {
    let code = match error {
        HandlerError::Unavailable => RejectionCode::HandlerUnavailable,
        HandlerError::NotFound => RejectionCode::NotFound,
        HandlerError::Rejected => RejectionCode::InvalidContract,
    };
    Rejection {
        code,
        field: "exchange.handler".to_string(),
    }
}
