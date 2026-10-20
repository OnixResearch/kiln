//! Bounded no-std UCAN `1.0.0` authorization core.
//!
//! This module owns deterministic final-profile authority meaning: slash
//! commands and complete-segment attenuation, the Delegation policy
//! language (parse and evaluate), ordered proof-chain composition with
//! principal alignment, subject continuity, Powerline substitution, time
//! bounds, revocation observations, and exact Invocation admission. It
//! performs no I/O, clock reads, key lookup, proof-store access, or replay.
//! The shell owns wire verification, CID-bound proof resolution, the
//! caller-owned replay commit, live values, and redacted receipts.

#![allow(clippy::module_name_repetitions)]

use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

pub use crate::profile::v1::IpldValue;

/// Maximum accepted policy evaluation steps.
pub const MAX_POLICY_EVALUATION_STEPS: usize = 4_096;
/// Maximum accepted policy AST depth.
pub const MAX_POLICY_DEPTH: usize = 64;
/// Maximum accepted policy statements per alternative.
pub const MAX_POLICY_STATEMENTS: usize = 1_024;
/// Maximum accepted proof count per Invocation.
pub const MAX_CHAIN_PROOFS: usize = crate::profile::v1::MAX_PROOF_REFERENCES;
/// Maximum accepted chain authority work units.
pub const MAX_CHAIN_WORK: usize = 16_384;

/// Command-hierarchy error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandError {
    MustStartWithSlash,
    EmptySegment,
    TrailingSlash,
    ControlOrSpace,
    UppercaseAscii,
    TooLong,
    NotADescendant,
}

impl fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::MustStartWithSlash => "command must start with a slash",
            Self::EmptySegment => "command has an empty segment",
            Self::TrailingSlash => "command must not end with a slash unless it is the root",
            Self::ControlOrSpace => "command contains a control or space character",
            Self::UppercaseAscii => "command segments must be lowercase ASCII or non-ASCII",
            Self::TooLong => "command exceeds the length limit",
            Self::NotADescendant => "command is not a complete-segment descendant",
        };
        formatter.write_str(text)
    }
}

/// A validated final-profile slash command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalCommand(String);

impl FinalCommand {
    /// Parse a final-profile command.
    ///
    /// # Errors
    ///
    /// Returns [`CommandError`] for malformed commands.
    pub fn new(value: &str) -> Result<Self, CommandError> {
        if value.len() > crate::profile::v1::MAX_SLASH_COMMAND_BYTES {
            return Err(CommandError::TooLong);
        }
        let bytes = value.as_bytes();
        if bytes.first() != Some(&b'/') {
            return Err(CommandError::MustStartWithSlash);
        }
        let root = bytes.len() == 1;
        if bytes.last() == Some(&b'/') && !root {
            return Err(CommandError::TrailingSlash);
        }
        for (index, &byte) in bytes.iter().enumerate() {
            if index > 0 && byte == b'/' && bytes[index - 1] == b'/' {
                return Err(CommandError::EmptySegment);
            }
            if byte <= 0x20 || byte == 0x7F {
                return Err(CommandError::ControlOrSpace);
            }
            if byte.is_ascii_uppercase() {
                return Err(CommandError::UppercaseAscii);
            }
        }
        Ok(Self(value.to_owned()))
    }

    /// Return the command text.
    #[must_use]
    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Return whether `self` is the root command `/`.
    #[must_use]
    pub const fn is_root(&self) -> bool {
        self.0.len() == 1
    }

    fn segments(&self) -> Vec<&str> {
        if self.is_root() {
            return vec![];
        }
        self.0.split('/').skip(1).collect()
    }

    /// Return whether `self` covers `child` by exact equality or complete
    /// descendant segments. A lexical prefix with a different segment is
    /// not a parent.
    #[must_use]
    pub fn covers(&self, child: &Self) -> bool {
        if self.is_root() {
            return true;
        }
        if self == child {
            return true;
        }
        let parent = self.segments();
        let child_segments = child.segments();
        if child_segments.len() <= parent.len() {
            return false;
        }
        for (index, segment) in parent.iter().enumerate() {
            if child_segments[index] != *segment {
                return false;
            }
        }
        true
    }
}

impl fmt::Display for FinalCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Policy parse or evaluation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PolicyError {
    NotAnArray,
    UnknownOperator(String),
    UnsupportedJq { feature: String },
    MalformedSelector,
    InvalidStatement,
    TooDeep,
    TooManyStatements,
    WorkLimitExceeded,
}

impl fmt::Display for PolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAnArray => formatter.write_str("policy must be an array"),
            Self::UnknownOperator(op) => write!(formatter, "unknown policy operator {op}"),
            Self::UnsupportedJq { feature } => write!(formatter, "unsupported jq feature: {feature}"),
            Self::MalformedSelector => formatter.write_str("malformed selector"),
            Self::InvalidStatement => formatter.write_str("invalid policy statement"),
            Self::TooDeep => formatter.write_str("policy nesting exceeds the depth limit"),
            Self::TooManyStatements => formatter.write_str("policy statement count exceeds the limit"),
            Self::WorkLimitExceeded => formatter.write_str("policy evaluation work limit exceeded"),
        }
    }
}

/// A dotted selector such as `.a.b`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Selector {
    segments: Vec<String>,
}

impl Selector {
    /// Parse a dotted selector.
    ///
    /// # Errors
    ///
    /// Returns [`PolicyError`] for malformed or unsupported-jq forms.
    pub fn parse(value: &str) -> Result<Self, PolicyError> {
        if value.is_empty() {
            return Err(PolicyError::MalformedSelector);
        }
        if value == "." {
            return Ok(Self { segments: Vec::new() });
        }
        if value.starts_with(".|") || value.contains("..") {
            return Err(PolicyError::UnsupportedJq {
                feature: "pipes or recursive descent".to_owned(),
            });
        }
        for unsupported in ['[', ']', '|', '?', '~'] {
            if value.contains(unsupported) {
                return Err(PolicyError::UnsupportedJq {
                    feature: unsupported.to_string(),
                });
            }
        }
        let mut segments = Vec::new();
        for raw in value.split('.') {
            let segment = raw;
            if segment.is_empty() {
                if segments.is_empty() && value.starts_with('.') {
                    continue;
                }
                return Err(PolicyError::MalformedSelector);
            }
            segments.push(segment.to_owned());
        }
        if segments.is_empty() || segments.len() > crate::profile::v1::MAX_IPLD_VALUE_DEPTH {
            return Err(PolicyError::MalformedSelector);
        }
        Ok(Self { segments })
    }

    fn resolve<'a>(&self, args: &'a IpldValue) -> Option<&'a IpldValue> {
        let mut current = args;
        for segment in &self.segments {
            let IpldValue::Map(items) = current else {
                return None;
            };
            let next = items.iter().find(|(key, _)| key == segment).map(|(_, value)| value)?;
            current = next;
        }
        Some(current)
    }
}

/// A policy expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expr {
    Atom(IpldValue),
    Select(Selector),
    Compare(BinOp, Box<Self>, Box<Self>),
    And(Vec<Self>),
    Or(Vec<Self>),
    Not(Box<Self>),
    Any(Selector, Box<Self>),
    All(Selector, Box<Self>),
}

/// Supported binary policy operators.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinOp {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    Like,
}

/// One policy alternative: a conjunction of statements. Invocation is
/// authorized by any alternative whose statements all hold.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyAlternative {
    statements: Vec<Expr>,
}

/// A parsed final Delegation policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Policy {
    alternatives: Vec<PolicyAlternative>,
}

struct ParseContext {
    statements_seen: usize,
}

impl Policy {
    /// Parse a policy from a bounded IPLD value.
    ///
    /// # Errors
    ///
    /// Returns [`PolicyError`] for malformed or unsupported input.
    pub fn parse(value: &IpldValue) -> Result<Self, PolicyError> {
        let IpldValue::List(alternatives) = value else {
            return Err(PolicyError::NotAnArray);
        };
        if alternatives.len() > MAX_POLICY_STATEMENTS {
            return Err(PolicyError::TooManyStatements);
        }
        let mut parsed = Vec::new();
        let mut context = ParseContext { statements_seen: 0 };
        for alternative in alternatives {
            let IpldValue::List(statements) = alternative else {
                return Err(PolicyError::InvalidStatement);
            };
            let mut items = Vec::new();
            for statement in statements {
                items.push(parse_statement(statement, 0, &mut context)?);
            }
            parsed.push(PolicyAlternative { statements: items });
        }
        Ok(Self { alternatives: parsed })
    }

    /// Evaluate the policy against Invocation arguments.
    ///
    /// # Errors
    ///
    /// Returns [`PolicyError`] when the work, depth, or statement limits run
    /// out.
    ///
    /// # Panics
    ///
    /// Does not panic; all resolution and type failures evaluate false.
    pub fn satisfied(&self, args: &IpldValue, budget: &mut usize) -> Result<bool, PolicyError> {
        if *budget < 1 {
            *budget = 0;
        }
        let mut consumed = 0;
        for alternative in &self.alternatives {
            let mut holds = true;
            for statement in &alternative.statements {
                consumed += 1;
                if consumed > MAX_POLICY_EVALUATION_STEPS {
                    return Err(PolicyError::WorkLimitExceeded);
                }
                if !evaluate(statement, args, 0, budget)? {
                    holds = false;
                    break;
                }
            }
            if holds {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

fn parse_statement(value: &IpldValue, depth: usize, context: &mut ParseContext) -> Result<Expr, PolicyError> {
    if depth > MAX_POLICY_DEPTH {
        return Err(PolicyError::TooDeep);
    }
    context.statements_seen += 1;
    if context.statements_seen > MAX_POLICY_STATEMENTS {
        return Err(PolicyError::TooManyStatements);
    }
    let IpldValue::List(items) = value else {
        return Err(PolicyError::InvalidStatement);
    };
    let Some(IpldValue::Text(operator)) = items.first() else {
        return Err(PolicyError::InvalidStatement);
    };
    match operator.as_str() {
        "==" | "!=" | "<" | ">" | "<=" | ">=" | "like" => {
            if items.len() != 3 {
                return Err(PolicyError::InvalidStatement);
            }
            let op = match operator.as_str() {
                "==" => BinOp::Eq,
                "!=" => BinOp::Ne,
                "<" => BinOp::Lt,
                ">" => BinOp::Gt,
                "<=" => BinOp::Le,
                ">=" => BinOp::Ge,
                _ => BinOp::Like,
            };
            Ok(Expr::Compare(
                op,
                Box::new(parse_operand(&items[1], depth + 1, context)?),
                Box::new(parse_operand(&items[2], depth + 1, context)?),
            ))
        }
        "and" | "or" => {
            if items.len() != 2 {
                return Err(PolicyError::InvalidStatement);
            }
            let IpldValue::List(sub) = &items[1] else {
                return Err(PolicyError::InvalidStatement);
            };
            let operands =
                sub.iter().map(|item| parse_statement(item, depth + 1, context)).collect::<Result<Vec<_>, _>>()?;
            if operator == "and" {
                Ok(Expr::And(operands))
            } else {
                Ok(Expr::Or(operands))
            }
        }
        "any" | "all" => {
            if items.len() != 3 {
                return Err(PolicyError::InvalidStatement);
            }
            let IpldValue::Text(selector) = &items[1] else {
                return Err(PolicyError::InvalidStatement);
            };
            let selector = Selector::parse(selector)?;
            let predicate = Box::new(parse_statement(&items[2], depth + 1, context)?);
            if operator == "any" {
                Ok(Expr::Any(selector, predicate))
            } else {
                Ok(Expr::All(selector, predicate))
            }
        }
        "not" => {
            if items.len() != 2 {
                return Err(PolicyError::InvalidStatement);
            }
            Ok(Expr::Not(Box::new(parse_statement(&items[1], depth + 1, context)?)))
        }
        other => Err(PolicyError::UnknownOperator(other.to_owned())),
    }
}

fn parse_operand(value: &IpldValue, depth: usize, context: &mut ParseContext) -> Result<Expr, PolicyError> {
    if depth > MAX_POLICY_DEPTH {
        return Err(PolicyError::TooDeep);
    }
    match value {
        IpldValue::Text(text) if text.starts_with('.') => Ok(Expr::Select(Selector::parse(text)?)),
        IpldValue::List(items)
            if matches!(
                items.first(),
                Some(IpldValue::Text(operator))
                    if matches!(
                        operator.as_str(),
                        "==" | "!=" | "<" | ">" | "<=" | ">=" | "like" | "and" | "or" | "not" | "any" | "all"
                    )
            ) =>
        {
            parse_statement(value, depth + 1, context)
        }
        _ => Ok(Expr::Atom(value.clone())),
    }
}

fn evaluate(expr: &Expr, args: &IpldValue, depth: usize, budget: &mut usize) -> Result<bool, PolicyError> {
    if depth > MAX_POLICY_DEPTH {
        return Err(PolicyError::TooDeep);
    }
    if *budget == 0 {
        return Err(PolicyError::WorkLimitExceeded);
    }
    *budget -= 1;
    match expr {
        Expr::Atom(_) => Ok(true),
        Expr::Select(selector) => Ok(selector.resolve(args).is_some()),
        Expr::Compare(op, left, right) => compare_operands(*op, left, right, args, depth + 1, budget),
        Expr::And(items) => {
            for item in items {
                if !evaluate(item, args, depth + 1, budget)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Expr::Or(items) => {
            for item in items {
                if evaluate(item, args, depth + 1, budget)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Expr::Not(item) => Ok(!evaluate(item, args, depth + 1, budget)?),
        Expr::Any(selector, predicate) => {
            let Some(collection) = selector.resolve(args) else {
                return Ok(false);
            };
            evaluate_any(collection, predicate, depth + 1, budget)
        }
        Expr::All(selector, predicate) => {
            let Some(collection) = selector.resolve(args) else {
                return Ok(false);
            };
            evaluate_all(collection, predicate, depth + 1, budget)
        }
    }
}

fn evaluate_any(
    collection: &IpldValue,
    predicate: &Expr,
    depth: usize,
    budget: &mut usize,
) -> Result<bool, PolicyError> {
    match collection {
        IpldValue::List(items) => {
            for item in items {
                if evaluate(predicate, item, depth, budget)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        IpldValue::Map(items) => {
            for (_, item) in items {
                if evaluate(predicate, item, depth, budget)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}

fn evaluate_all(
    collection: &IpldValue,
    predicate: &Expr,
    depth: usize,
    budget: &mut usize,
) -> Result<bool, PolicyError> {
    match collection {
        IpldValue::List(items) => {
            for item in items {
                if !evaluate(predicate, item, depth, budget)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        IpldValue::Map(items) => {
            for (_, item) in items {
                if !evaluate(predicate, item, depth, budget)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn resolve_operand(
    expr: &Expr,
    args: &IpldValue,
    depth: usize,
    budget: &mut usize,
) -> Result<Option<IpldValue>, PolicyError> {
    match expr {
        Expr::Atom(value) => Ok(Some(value.clone())),
        Expr::Select(selector) => Ok(selector.resolve(args).cloned()),
        Expr::Compare(..) | Expr::And(..) | Expr::Or(..) | Expr::Not(..) | Expr::Any(..) | Expr::All(..) => {
            // A sub-expression used as an operand is a boolean-valued guard.
            let result = evaluate(expr, args, depth, budget)?;
            Ok(Some(IpldValue::Bool(result)))
        }
    }
}

fn compare_operands(
    op: BinOp,
    left: &Expr,
    right: &Expr,
    args: &IpldValue,
    depth: usize,
    budget: &mut usize,
) -> Result<bool, PolicyError> {
    let Some(left_value) = resolve_operand(left, args, depth, budget)? else {
        // An unresolved required operand evaluates false.
        return Ok(false);
    };
    let Some(right_value) = resolve_operand(right, args, depth, budget)? else {
        return Ok(false);
    };
    Ok(compare_values(op, &left_value, &right_value))
}

const fn parse_number(value: &IpldValue) -> Option<i64> {
    match value {
        IpldValue::Int(value) | IpldValue::Float(value) => Some(*value),
        _ => None,
    }
}

fn compare_values(op: BinOp, left: &IpldValue, right: &IpldValue) -> bool {
    match op {
        BinOp::Eq => values_equal(left, right),
        BinOp::Ne => !values_equal(left, right),
        BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
            if let (Some(a), Some(b)) = (parse_number(left), parse_number(right)) {
                return match op {
                    BinOp::Lt => a < b,
                    BinOp::Gt => a > b,
                    BinOp::Le => a <= b,
                    _ => a >= b,
                };
            }
            let (IpldValue::Text(a), IpldValue::Text(b)) = (left, right) else {
                return false;
            };
            match op {
                BinOp::Lt => a.as_str() < b.as_str(),
                BinOp::Gt => a.as_str() > b.as_str(),
                BinOp::Le => a.as_str() <= b.as_str(),
                _ => a.as_str() >= b.as_str(),
            }
        }
        BinOp::Like => like_match(right, left),
    }
}

fn values_equal(left: &IpldValue, right: &IpldValue) -> bool {
    if let (Some(left_number), Some(right_number)) = (parse_number(left), parse_number(right)) {
        return left_number == right_number;
    }
    match (left, right) {
        (IpldValue::Null, IpldValue::Null) => true,
        (IpldValue::Bool(a), IpldValue::Bool(b)) => a == b,
        (IpldValue::Text(a), IpldValue::Text(b)) => a == b,
        (IpldValue::Bytes(a), IpldValue::Bytes(b)) => a == b,
        (IpldValue::Bytes(a), IpldValue::List(b)) | (IpldValue::List(b), IpldValue::Bytes(a)) => {
            bytes_equal_int_list(a, b)
        }
        (IpldValue::List(a), IpldValue::List(b)) => a == b,
        (IpldValue::Map(a), IpldValue::Map(b)) => a == b,
        _ => false,
    }
}

fn like_match(pattern_value: &IpldValue, text_value: &IpldValue) -> bool {
    let (IpldValue::Text(pattern), IpldValue::Text(text)) = (pattern_value, text_value) else {
        return false;
    };
    glob_match(pattern.as_str(), text.as_str())
}

fn bytes_equal_int_list(bytes: &[u8], items: &[IpldValue]) -> bool {
    if bytes.len() != items.len() {
        return false;
    }
    for (index, item) in items.iter().enumerate() {
        let IpldValue::Int(value) = item else {
            return false;
        };
        if *value < 0 || *value > i64::from(u8::MAX) {
            return false;
        }
        if bytes[index] != u8::try_from(*value).expect("bounded byte value") {
            return false;
        }
    }
    true
}

#[derive(Clone, Copy)]
enum GlobToken {
    Literal(char),
    AnyOne,
    AnyMany,
}

fn glob_match(pattern: &str, text: &str) -> bool {
    let mut tokens = Vec::new();
    let mut pattern_chars = pattern.chars();
    while let Some(pattern_char) = pattern_chars.next() {
        match pattern_char {
            '\\' => {
                let Some(escaped) = pattern_chars.next() else {
                    return false;
                };
                tokens.push(GlobToken::Literal(escaped));
            }
            '*' => tokens.push(GlobToken::AnyMany),
            '?' => tokens.push(GlobToken::AnyOne),
            literal => tokens.push(GlobToken::Literal(literal)),
        }
    }

    let text_chars = text.chars().collect::<Vec<_>>();
    let mut previous = vec![false; text_chars.len() + 1];
    previous[0] = true;
    for token in tokens {
        let mut current = vec![false; text_chars.len() + 1];
        match token {
            GlobToken::AnyMany => {
                current[0] = previous[0];
                for text_index in 1..=text_chars.len() {
                    current[text_index] = previous[text_index] || current[text_index - 1];
                }
            }
            GlobToken::AnyOne => {
                current[1..].copy_from_slice(&previous[..text_chars.len()]);
            }
            GlobToken::Literal(literal) => {
                for text_index in 1..=text_chars.len() {
                    current[text_index] = previous[text_index - 1] && text_chars[text_index - 1] == literal;
                }
            }
        }
        previous = current;
    }
    previous[text_chars.len()]
}

/// Delegation authority and chain errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChainError {
    EmptyChain,
    DuplicateProof,
    CycleProof,
    RootWithoutSubject,
    RootSubstitution,
    SubjectMismatch,
    OrderViolation { first: usize },
    NotInvoker,
    ExecutorMismatch,
    InvalidClockSkew,
    CommandNotCovered { index: usize },
    TimeInvalid { index: usize },
    PolicyDenied { index: usize },
    Revoked { index: usize },
    LimitExceeded,
}

impl fmt::Display for ChainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyChain => formatter.write_str("invocation has an empty proof chain"),
            Self::DuplicateProof => formatter.write_str("proof chain contains a duplicate CID"),
            Self::CycleProof => formatter.write_str("proof chain contains a cycle"),
            Self::RootWithoutSubject => formatter.write_str("root delegation has no subject"),
            Self::RootSubstitution => formatter.write_str("root subject substitution is invalid"),
            Self::SubjectMismatch => formatter.write_str("delegation subject does not match the chain subject"),
            Self::OrderViolation { first } => write!(formatter, "proof order breaks alignment at index {first}"),
            Self::NotInvoker => formatter.write_str("the final audience is not the Invoker"),
            Self::ExecutorMismatch => {
                formatter.write_str("the Invocation audience does not match the admitted executor")
            }
            Self::InvalidClockSkew => formatter.write_str("clock skew must be non-negative"),
            Self::CommandNotCovered { index } => write!(formatter, "command attenuation fails at index {index}"),
            Self::TimeInvalid { index } => write!(formatter, "time bounds fail at index {index}"),
            Self::PolicyDenied { index } => write!(formatter, "delegation policy denies at index {index}"),
            Self::Revoked { index } => write!(formatter, "delegation is revoked at index {index}"),
            Self::LimitExceeded => formatter.write_str("chain work limit exceeded"),
        }
    }
}

/// Verified Delegation facts supplied by the shell. No bytes beyond the
/// proof identity are carried.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DelegationFacts {
    /// Delegation issuer DID.
    pub issuer: String,
    /// Delegation audience DID.
    pub audience: String,
    /// Delegation subject DID when present.
    pub subject: Option<String>,
    /// Delegation command when present.
    pub command: Option<FinalCommand>,
    /// Parsed Delegation policy when present.
    pub policy: Option<Policy>,
    /// Not-before seconds when present.
    pub not_before: Option<i64>,
    /// Expires-at seconds when present (nullable).
    pub expires_at: Option<i64>,
    /// The exact proof CID bytes of this Delegation.
    pub cid: Vec<u8>,
    /// Fail-closed revocation observation supplied by the caller.
    pub revoked: bool,
}

/// Invocation facts extracted from the verified wire envelope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvocationFacts {
    /// Invocation issuer DID (the Invoker).
    pub issuer: String,
    /// Invocation audience DID (the executor).
    pub executor: String,
    /// Whether the caller admitted the exact executor observation.
    pub executor_allowed: bool,
    /// Invocation subject DID when present.
    pub subject: Option<String>,
    /// The exact command being invoked.
    pub command: FinalCommand,
    /// Bounded Invocation arguments.
    pub args: IpldValue,
    /// Ordered proof CID bytes from root to Invoker.
    pub proofs: Vec<Vec<u8>>,
    /// Not-before seconds when present.
    pub not_before: Option<i64>,
    /// Expires-at seconds when present (nullable).
    pub expires_at: Option<i64>,
}

/// The exact effective authority facts for a satisfied chain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectiveAuthority {
    /// The shared chain subject.
    pub subject: String,
    /// The exact admitted executor.
    pub executor: String,
    /// The root authority issuer.
    pub root_issuer: String,
    /// The effective (leaf) command.
    pub effective_command: FinalCommand,
    /// Ordered proof identities.
    pub proof_cids: Vec<Vec<u8>>,
}

/// Compose an ordered proof chain and admit the Invocation.
///
/// The shell must present Delegations in the exact order of
/// `invocation.proofs` and must have bound each returned token to its CID.
///
/// # Errors
///
/// Returns [`ChainError`] on any authority, order, subject, command, time,
/// policy, revocation, or limit failure.
///
/// # Panics
///
/// Does not panic; all admission outcomes are typed.
pub fn compose_chain(
    invocation: &InvocationFacts,
    delegations: &[DelegationFacts],
    now_seconds: i64,
    clock_skew: i64,
    work: &mut usize,
) -> Result<EffectiveAuthority, ChainError> {
    if delegations.is_empty() {
        return Err(ChainError::EmptyChain);
    }
    if invocation.proofs.len() != delegations.len() {
        return Err(ChainError::OrderViolation { first: 0 });
    }
    for (index, delegation) in delegations.iter().enumerate() {
        if delegation.cid != invocation.proofs[index] {
            return Err(ChainError::OrderViolation { first: index });
        }
    }
    let root = &delegations[0];
    let chain_subject = root.subject.as_ref().ok_or(ChainError::RootWithoutSubject)?.clone();
    if root.issuer.is_empty() || chain_subject.is_empty() {
        return Err(ChainError::RootSubstitution);
    }
    if let Some(invocation_subject) = &invocation.subject
        && invocation_subject != &chain_subject
    {
        return Err(ChainError::SubjectMismatch);
    }
    if invocation.executor.is_empty() || !invocation.executor_allowed {
        return Err(ChainError::ExecutorMismatch);
    }
    if clock_skew < 0 {
        return Err(ChainError::InvalidClockSkew);
    }
    check_dup_cycle(delegations)?;
    check_order(delegations, invocation)?;
    check_subjects(delegations, &chain_subject)?;
    check_time(invocation, delegations, now_seconds, clock_skew)?;
    check_commands(delegations, &invocation.command)?;
    check_revocation(delegations)?;
    check_policies(delegations, &invocation.args, work)?;
    if *work == 0 {
        return Err(ChainError::LimitExceeded);
    }
    *work -= 1;

    let final_command = invocation.command.clone();
    let mut proof_cids = Vec::with_capacity(delegations.len());
    for delegation in delegations {
        proof_cids.push(delegation.cid.clone());
    }
    Ok(EffectiveAuthority {
        subject: chain_subject,
        executor: invocation.executor.clone(),
        root_issuer: root.issuer.clone(),
        effective_command: final_command,
        proof_cids,
    })
}

fn check_dup_cycle(delegations: &[DelegationFacts]) -> Result<(), ChainError> {
    for (index, delegation) in delegations.iter().enumerate() {
        if delegations[..index].iter().any(|earlier| earlier.cid == delegation.cid) {
            return Err(ChainError::DuplicateProof);
        }
    }
    Ok(())
}

fn check_order(delegations: &[DelegationFacts], invocation: &InvocationFacts) -> Result<(), ChainError> {
    for index in 1..delegations.len() {
        if delegations[index - 1].audience != delegations[index].issuer {
            return Err(ChainError::OrderViolation { first: index });
        }
    }
    let final_index = delegations.len() - 1;
    if delegations[final_index].audience != invocation.issuer {
        return Err(ChainError::NotInvoker);
    }
    Ok(())
}

fn check_subjects(delegations: &[DelegationFacts], chain_subject: &str) -> Result<(), ChainError> {
    for delegation in delegations.iter().skip(1) {
        if let Some(subject) = &delegation.subject
            && subject != chain_subject
        {
            return Err(ChainError::SubjectMismatch);
        }
    }
    Ok(())
}

fn check_time(
    invocation: &InvocationFacts,
    delegations: &[DelegationFacts],
    now_seconds: i64,
    clock_skew: i64,
) -> Result<(), ChainError> {
    check_time_bounds(invocation.not_before, invocation.expires_at, now_seconds, clock_skew)
        .map_err(|()| ChainError::TimeInvalid { index: 0 })?;
    for (index, delegation) in delegations.iter().enumerate() {
        check_time_bounds(delegation.not_before, delegation.expires_at, now_seconds, clock_skew)
            .map_err(|()| ChainError::TimeInvalid { index })?;
    }
    Ok(())
}

const fn check_time_bounds(nbf: Option<i64>, exp: Option<i64>, now: i64, skew: i64) -> Result<(), ()> {
    let Some(latest) = now.checked_add(skew) else {
        return Err(());
    };
    let Some(earliest) = now.checked_sub(skew) else {
        return Err(());
    };
    if let Some(nbf_value) = nbf
        && latest < nbf_value
    {
        return Err(());
    }
    if let Some(exp_value) = exp
        && earliest > exp_value
    {
        return Err(());
    }
    Ok(())
}

fn check_commands(delegations: &[DelegationFacts], invocation_command: &FinalCommand) -> Result<(), ChainError> {
    let last = delegations.len() - 1;
    if let Some(command) = &delegations[last].command
        && !command.covers(invocation_command)
    {
        return Err(ChainError::CommandNotCovered { index: last });
    }
    for index in 0..last {
        if let Some(parent) = &delegations[index].command
            && (!parent.covers(invocation_command)
                || delegations[index + 1].command.as_ref().is_some_and(|child| !parent.covers(child)))
        {
            return Err(ChainError::CommandNotCovered { index });
        }
    }
    Ok(())
}

fn check_revocation(delegations: &[DelegationFacts]) -> Result<(), ChainError> {
    for (index, delegation) in delegations.iter().enumerate() {
        if delegation.revoked {
            return Err(ChainError::Revoked { index });
        }
    }
    Ok(())
}

fn check_policies(delegations: &[DelegationFacts], args: &IpldValue, work: &mut usize) -> Result<(), ChainError> {
    for (index, delegation) in delegations.iter().enumerate() {
        if let Some(policy) = &delegation.policy {
            let satisfied = policy.satisfied(args, work).map_err(|_| ChainError::LimitExceeded)?;
            if !satisfied {
                return Err(ChainError::PolicyDenied { index });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
