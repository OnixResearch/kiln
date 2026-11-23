//! Scoped append-only run board, typed posts, `@mention` routing, read cursors,
//! and deterministic replayable mention graph projection.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::{
    DEFAULT_READ_LIMIT, MAX_BOARD_POSTS, MAX_POST_BODY_BYTES, MAX_POST_MENTIONS, MAX_READ_LIMIT,
    SwarmMember, SwarmRole, SwarmRun, valid_body_text, valid_commit_sha, valid_member_name,
};
use crate::fleet::refusal;
use crate::types::CoreResult;

/// Typed kind of one swarm board entry.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoardPostKind {
    Brief,
    Status,
    Handoff,
    Review,
    Verdict,
    Decision,
    Failover,
    Watchdog,
}

impl BoardPostKind {
    /// Privileged kinds emitted only by typed control-plane transitions,
    /// never by raw untrusted board posts.
    #[must_use]
    pub const fn is_privileged(self) -> bool {
        matches!(self, Self::Verdict | Self::Failover | Self::Watchdog)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Brief => "brief",
            Self::Status => "status",
            Self::Handoff => "handoff",
            Self::Review => "review",
            Self::Verdict => "verdict",
            Self::Decision => "decision",
            Self::Failover => "failover",
            Self::Watchdog => "watchdog",
        }
    }
}

/// Explicit `@mention` target carried by a board post.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(tag = "kind", content = "target", rename_all = "snake_case")]
pub enum MentionTarget {
    Member(String),
    Role(SwarmRole),
    All,
}

impl MentionTarget {
    /// Validate a mention target's syntax.
    ///
    /// # Errors
    /// Refuses malformed member names.
    pub fn validate(&self) -> CoreResult<()> {
        if let Self::Member(name) = self
            && !valid_member_name(name)
        {
            return Err(refusal(
                "fleet_swarm_mention",
                "mentions",
                "mentioned member name must match [a-z0-9][a-z0-9-]*",
            ));
        }
        Ok(())
    }

    /// True if this mention wakes `member`.
    #[must_use]
    pub fn matches_member(&self, member: &SwarmMember) -> bool {
        match self {
            Self::Member(name) => name == &member.name,
            Self::Role(role) => *role == member.role,
            Self::All => true,
        }
    }
}

/// One append-only message on a swarm run board.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoardPost {
    pub seq: u64,
    pub sender: String,
    pub sender_role: SwarmRole,
    pub kind: BoardPostKind,
    pub mentions: Vec<MentionTarget>,
    pub body: String,
    pub posted_ms: u64,
    #[serde(default)]
    pub in_reply_to: Option<u64>,
    #[serde(default)]
    pub candidate_commit: Option<String>,
}

/// Append-only run board with monotonic sequence numbers starting at 1.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwarmBoard {
    pub next_seq: u64,
    pub posts: Vec<BoardPost>,
}

impl Default for SwarmBoard {
    fn default() -> Self {
        Self {
            next_seq: 1,
            posts: Vec::new(),
        }
    }
}

impl SwarmBoard {
    /// Highest sequence number recorded on this board, or 0 if empty.
    #[must_use]
    pub fn head_seq(&self) -> u64 {
        self.posts.last().map_or(0, |post| post.seq)
    }

    /// Oldest posts with `seq > since_seq`, capped at `limit`.
    #[must_use]
    pub fn read_since(&self, since_seq: u64, limit: usize) -> Vec<BoardPost> {
        let cap = limit.clamp(1, MAX_READ_LIMIT);
        self.posts
            .iter()
            .filter(|post| post.seq > since_seq)
            .take(cap)
            .cloned()
            .collect()
    }

    /// Exact posts matching `seqs`, returned in monotonic sequence order
    /// without duplicates. Missing sequence numbers are omitted.
    #[must_use]
    pub fn read_seqs(&self, seqs: &[u64]) -> Vec<BoardPost> {
        self.posts
            .iter()
            .filter(|post| seqs.contains(&post.seq))
            .take(MAX_READ_LIMIT)
            .cloned()
            .collect()
    }

    /// Sequence numbers after `since_seq` whose explicit mentions target
    /// `member` (excluding posts sent by `member` itself).
    #[must_use]
    pub fn unread_mentions_for(&self, member: &SwarmMember, since_seq: u64) -> Vec<u64> {
        self.posts
            .iter()
            .filter(|post| {
                post.seq > since_seq
                    && post.sender != member.name
                    && post
                        .mentions
                        .iter()
                        .any(|mention| mention.matches_member(member))
            })
            .map(|post| post.seq)
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn append(
        &mut self,
        sender: String,
        sender_role: SwarmRole,
        kind: BoardPostKind,
        mentions: Vec<MentionTarget>,
        body: String,
        in_reply_to: Option<u64>,
        candidate_commit: Option<String>,
        posted_ms: u64,
    ) -> CoreResult<BoardPost> {
        if self.posts.len() >= MAX_BOARD_POSTS {
            return Err(refusal(
                "fleet_swarm_board_limit",
                "board.posts",
                "swarm board reached its post bound",
            ));
        }
        if mentions.len() > MAX_POST_MENTIONS {
            return Err(refusal(
                "fleet_swarm_mention",
                "mentions",
                "board post exceeds the mention bound",
            ));
        }
        for mention in &mentions {
            mention.validate()?;
        }
        if !valid_body_text(&body, MAX_POST_BODY_BYTES) {
            return Err(refusal(
                "fleet_swarm_body",
                "body",
                "board post body is empty, oversized, or contains NUL",
            ));
        }
        if let Some(reply_seq) = in_reply_to
            && (reply_seq == 0 || reply_seq > self.head_seq())
        {
            return Err(refusal(
                "fleet_swarm_reply",
                "in_reply_to",
                "in_reply_to does not refer to an existing board sequence",
            ));
        }
        if let Some(commit) = &candidate_commit
            && !valid_commit_sha(commit)
        {
            return Err(refusal(
                "fleet_swarm_commit",
                "candidate_commit",
                "candidate commit must be a 40- or 64-character lowercase hex id",
            ));
        }
        let seq = self.next_seq;
        let next_seq = seq.checked_add(1).ok_or_else(|| {
            refusal(
                "fleet_swarm_board_limit",
                "board.next_seq",
                "swarm board sequence numbers are exhausted",
            )
        })?;
        let post = BoardPost {
            seq,
            sender,
            sender_role,
            kind,
            mentions,
            body,
            posted_ms,
            in_reply_to,
            candidate_commit,
        };
        self.next_seq = next_seq;
        self.posts.push(post.clone());
        Ok(post)
    }
}

/// Response payload for reading a swarm board.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoardReadResult {
    pub head_seq: u64,
    pub posts: Vec<BoardPost>,
    pub unread_mentions: Vec<u64>,
}

/// One node in the replayable mention graph.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MentionGraphNode {
    pub member: String,
    pub role: SwarmRole,
    pub state: String,
    pub post_count: u64,
    pub mentions_in: u64,
    pub mentions_out: u64,
}

/// One weighted directed edge `from -> to` in the replayable mention graph.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MentionEdge {
    pub from: String,
    pub to: String,
    pub weight: u64,
    pub last_seq: u64,
    pub kinds: Vec<BoardPostKind>,
}

/// Deterministic directed weighted mention graph up to `up_to_seq`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MentionGraph {
    pub up_to_seq: u64,
    pub head_seq: u64,
    pub nodes: Vec<MentionGraphNode>,
    pub edges: Vec<MentionEdge>,
}

// r[impl kiln.fleet.swarm.model] r[impl kiln.fleet.swarm.model.scenario.untrusted_board]
/// Append one untrusted data post to the swarm board.
///
/// Enforces the untrusted-data invariant:
/// - refuses privileged post kinds (`Verdict`, `Failover`, `Watchdog`), which
///   may only be recorded by typed control-plane transitions,
/// - never mutates claims, scopes, roles, budgets, or run phase.
///
/// # Errors
/// Refuses unknown or inactive senders, privileged kinds, invalid mentions,
/// empty/oversized/NUL bodies, invalid reply references, or malformed commits.
#[allow(clippy::too_many_arguments)]
pub fn post_to_board(
    run: &SwarmRun,
    sender: &str,
    kind: BoardPostKind,
    mentions: Vec<MentionTarget>,
    body: String,
    in_reply_to: Option<u64>,
    candidate_commit: Option<String>,
    now_ms: u64,
) -> CoreResult<(SwarmRun, BoardPost)> {
    if kind.is_privileged() {
        return Err(refusal(
            "fleet_swarm_post_kind",
            "kind",
            "verdict, failover, and watchdog board posts may only be emitted by their typed control-plane transitions",
        ));
    }
    let member = run.members.get(sender).ok_or_else(|| {
        refusal(
            "fleet_swarm_member_unknown",
            "sender",
            "no such member in this swarm run",
        )
    })?;
    if !member.state.is_active() {
        return Err(refusal(
            "fleet_swarm_member_ended",
            "sender",
            "retired or terminal members cannot post to the board",
        ));
    }
    let sender_role = member.role;
    let mut next = run.clone();
    let post = next.board.append(
        sender.to_string(),
        sender_role,
        kind,
        mentions,
        body,
        in_reply_to,
        candidate_commit,
        now_ms,
    )?;
    if let Some(updated_member) = next.members.get_mut(sender) {
        updated_member.last_post_seq = Some(post.seq);
        updated_member.observed_ms = updated_member.observed_ms.max(now_ms);
    }
    next.updated_ms = next.updated_ms.max(now_ms);
    Ok((next, post))
}

// r[impl kiln.fleet.swarm.model]
/// Read posts from a swarm run board by sequence range or explicit sequence list.
///
/// # Errors
/// Refuses an unknown `member` filter name.
pub fn read_board(
    run: &SwarmRun,
    member: Option<&str>,
    since: Option<u64>,
    seqs: Option<&[u64]>,
    limit: Option<usize>,
) -> CoreResult<BoardReadResult> {
    let resolved_member = match member {
        Some(name) => Some(run.members.get(name).ok_or_else(|| {
            refusal(
                "fleet_swarm_member_unknown",
                "member",
                "no such member in this swarm run",
            )
        })?),
        None => None,
    };
    let head_seq = run.board.head_seq();
    let posts = if let Some(exact) = seqs {
        run.board.read_seqs(exact)
    } else {
        let cursor = since.unwrap_or_else(|| resolved_member.map_or(0, |m| m.last_read_seq));
        run.board
            .read_since(cursor, limit.unwrap_or(DEFAULT_READ_LIMIT))
    };
    let unread_mentions = match resolved_member {
        Some(m) => run.board.unread_mentions_for(m, m.last_read_seq),
        None => Vec::new(),
    };
    Ok(BoardReadResult {
        head_seq,
        posts,
        unread_mentions,
    })
}

// r[impl kiln.fleet.swarm.model.scenario.mention_graph]
/// Deterministically compute directed weighted edges from board posts with
/// `seq <= up_to_seq` (or all posts when `up_to_seq` is `None`).
#[must_use]
pub fn mention_graph(run: &SwarmRun, up_to_seq: Option<u64>) -> MentionGraph {
    let head_seq = run.board.head_seq();
    let cutoff = up_to_seq.map_or(head_seq, |limit| limit.min(head_seq));

    let mut post_counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut mentions_in: BTreeMap<String, u64> = BTreeMap::new();
    let mut mentions_out: BTreeMap<String, u64> = BTreeMap::new();
    let mut edge_acc: BTreeMap<(String, String), (u64, u64, Vec<BoardPostKind>)> = BTreeMap::new();

    for post in run.board.posts.iter().filter(|post| post.seq <= cutoff) {
        *post_counts.entry(post.sender.clone()).or_insert(0) += 1;
        let targets = resolve_targets(run, &post.sender, &post.mentions);
        for target in targets {
            *mentions_out.entry(post.sender.clone()).or_insert(0) += 1;
            *mentions_in.entry(target.clone()).or_insert(0) += 1;
            let entry = edge_acc
                .entry((post.sender.clone(), target))
                .or_insert_with(|| (0, 0, Vec::new()));
            entry.0 += 1;
            entry.1 = entry.1.max(post.seq);
            if !entry.2.contains(&post.kind) {
                entry.2.push(post.kind);
                entry.2.sort();
            }
        }
    }

    let nodes = run
        .members
        .values()
        .map(|member| MentionGraphNode {
            member: member.name.clone(),
            role: member.role,
            state: member.state.label().to_string(),
            post_count: post_counts.get(&member.name).copied().unwrap_or(0),
            mentions_in: mentions_in.get(&member.name).copied().unwrap_or(0),
            mentions_out: mentions_out.get(&member.name).copied().unwrap_or(0),
        })
        .collect();

    let edges = edge_acc
        .into_iter()
        .map(|((from, to), (weight, last_seq, kinds))| MentionEdge {
            from,
            to,
            weight,
            last_seq,
            kinds,
        })
        .collect();

    MentionGraph {
        up_to_seq: cutoff,
        head_seq,
        nodes,
        edges,
    }
}

fn resolve_targets(run: &SwarmRun, sender: &str, mentions: &[MentionTarget]) -> Vec<String> {
    let mut resolved = Vec::new();
    for mention in mentions {
        match mention {
            MentionTarget::Member(name) => {
                if name != sender && !resolved.contains(name) {
                    resolved.push(name.clone());
                }
            }
            MentionTarget::Role(role) => {
                let mut matched_any = false;
                for member in run.members.values() {
                    if member.role == *role && member.name != sender {
                        matched_any = true;
                        if !resolved.contains(&member.name) {
                            resolved.push(member.name.clone());
                        }
                    }
                }
                if !matched_any {
                    let label = alloc::format!("@{}", role.as_str());
                    if !resolved.contains(&label) {
                        resolved.push(label);
                    }
                }
            }
            MentionTarget::All => {
                let mut matched_any = false;
                for member in run.members.values() {
                    if member.name != sender {
                        matched_any = true;
                        if !resolved.contains(&member.name) {
                            resolved.push(member.name.clone());
                        }
                    }
                }
                if !matched_any {
                    let label = "@all".to_string();
                    if !resolved.contains(&label) {
                        resolved.push(label);
                    }
                }
            }
        }
    }
    resolved.sort();
    resolved
}
