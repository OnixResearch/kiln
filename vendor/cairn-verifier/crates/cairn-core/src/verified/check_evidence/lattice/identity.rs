//! The published Lattice v1/v2 framing protocol: raw domain followed by
//! little-endian u64-length-prefixed labels and values. These are wire bytes,
//! not Cairn canonical-JSON identities.
use super::model::*;
struct Frame(blake3::Hasher);
impl Frame {
    fn new(domain: &str) -> Self {
        let mut hash = blake3::Hasher::new();
        hash.update(domain.as_bytes());
        Self(hash)
    }
    fn field(&mut self, label: &str, bytes: &[u8]) {
        for part in [label.as_bytes(), bytes] {
            self.0.update(&(part.len() as u64).to_le_bytes());
            self.0.update(part);
        }
    }
    fn text(&mut self, label: &str, text: &str) {
        self.field(label, text.as_bytes());
    }
    fn number(&mut self, label: &str, value: u64) {
        self.field(label, &value.to_le_bytes());
    }
    fn boolean(&mut self, label: &str, value: bool) {
        self.field(label, &[u8::from(value)]);
    }
    fn optional(&mut self, marker: &str, label: &str, value: &Option<String>) {
        self.boolean(marker, value.is_some());
        if let Some(value) = value {
            self.text(label, value);
        }
    }
    fn strings(&mut self, count: &str, label: &str, values: &[String]) {
        self.number(count, values.len() as u64);
        for value in values {
            self.text(label, value);
        }
    }
    fn finish(self) -> String {
        format!("b3:{}", self.0.finalize().to_hex())
    }
}
pub(super) fn profile(p: &Profile) -> String {
    let mut f = Frame::new("lattice.workflow-profile.identity.v1");
    f.text("schema", &p.schema);
    f.text("graph_schema", &p.graph_schema);
    f.strings("platform_count", "platform", &p.platforms);
    f.strings("node_kind_count", "node_kind", &p.node_kinds);
    f.strings("effect_class_count", "effect_class", &p.effect_classes);
    f.strings("adapter_kind_count", "adapter_kind", &p.adapter_kinds);
    f.boolean("cancel", p.controls.cancel);
    f.boolean("pause", p.controls.pause);
    f.boolean("resume", p.controls.resume);
    for (label, value) in [
        ("maximum_nodes", p.limits.maximum_nodes),
        ("maximum_edges", p.limits.maximum_edges),
        ("maximum_log_references", p.limits.maximum_log_references),
        ("maximum_log_read_bytes", p.limits.maximum_log_read_bytes),
        ("maximum_total_log_bytes", p.limits.maximum_total_log_bytes),
    ] {
        f.number(label, value);
    }
    f.strings("non_claim_count", "non_claim", &p.non_claims);
    f.finish()
}
pub(super) fn graph(g: &Graph) -> String {
    let mut f = Frame::new("lattice.workflow-graph.identity.v1");
    f.text("schema", &g.schema);
    f.number("node_count", g.nodes.len() as u64);
    for n in &g.nodes {
        for (label, value) in [
            ("node_id", &n.id),
            ("node_kind", &n.kind),
            ("effect_class", &n.effect_class),
            ("adapter_kind", &n.adapter_kind),
            ("operation_identity", &n.operation_identity),
        ] {
            f.text(label, value);
        }
    }
    f.number("edge_count", g.edges.len() as u64);
    for edge in &g.edges {
        f.text("edge_source", &edge.source);
        f.text("edge_target", &edge.target);
    }
    f.finish()
}
pub(super) fn request(r: &Request) -> String {
    let mut f = Frame::new("lattice.workflow-request.identity.v2");
    for (label, value) in [
        ("schema", &r.schema),
        ("idempotency_key", &r.idempotency_key),
        ("profile_identity", &r.profile_identity),
        ("platform", &r.platform),
    ] {
        f.text(label, value);
    }
    for (label, value) in [
        "effect_identity",
        "attempt_identity",
        "candidate_identity",
        "pipeline_revision_identity",
        "workflow_policy_identity",
    ]
    .into_iter()
    .zip(r.kiln.values())
    {
        f.text(label, value);
    }
    f.boolean("has_source_context", r.source.is_some());
    if let Some(source) = &r.source {
        f.text("source_provider", &source.provider);
        f.text("source_repository", &source.repository);
        f.text("source_revision", &source.revision);
    }
    f.text("graph_identity", &r.graph.identity);
    f.boolean("control_cancel", r.controls.cancel);
    f.boolean("control_pause", r.controls.pause);
    f.boolean("control_resume", r.controls.resume);
    f.number(
        "maximum_log_references",
        r.observation.maximum_log_references,
    );
    f.number(
        "maximum_log_read_bytes",
        r.observation.maximum_log_read_bytes,
    );
    f.number(
        "maximum_total_log_bytes",
        r.observation.maximum_total_log_bytes,
    );
    f.finish()
}
pub(super) fn observation(o: &Observation) -> String {
    let mut f = Frame::new("lattice.workflow-observation.identity.v1");
    for (label, value) in [
        ("schema", &o.schema),
        ("request_identity", &o.request_identity),
        ("idempotency_key", &o.idempotency_key),
        ("workflow_identity", &o.workflow_identity),
        ("execution_identity", &o.execution_identity),
    ] {
        f.text(label, value);
    }
    f.number("sequence", o.sequence);
    f.text("state", &o.state);
    f.boolean("terminal", o.terminal);
    f.boolean("successful", o.successful);
    f.number("node_count", o.nodes.len() as u64);
    for n in &o.nodes {
        f.text("node_id", &n.node_id);
        f.text("node_state", &n.state);
        f.boolean("node_terminal", n.terminal);
        f.boolean("node_successful", n.successful);
        f.text("adapter_identity", &n.adapter_identity);
        f.optional(
            "has_node_receipt",
            "node_receipt_identity",
            &n.receipt_identity,
        );
    }
    f.number("log_count", o.logs.len() as u64);
    for l in &o.logs {
        f.text("log_node_id", &l.node_id);
        f.text("log_stream", &l.stream);
        f.text("log_reference_identity", &l.reference_identity);
        f.number("log_cursor", l.cursor);
        f.number("log_bytes", l.bytes);
        f.boolean("log_truncated", l.truncated);
        f.boolean("log_gap_before", l.gap_before);
        f.boolean("log_retained", l.retained);
    }
    f.optional("has_receipt", "receipt_identity", &o.receipt_identity);
    f.finish()
}
