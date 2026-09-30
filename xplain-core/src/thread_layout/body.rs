//! A comment thread flattened to display lines: turns, answers, follow-ups (F-ASK-05).

use super::text::{BodyKind, BodyLine, plain_line, rich_lines};
use crate::comments::{self, Answer, AnswerStatus, Comment};
use crate::textutil;

/// Status label in the answer divider (F-ASK-05).
pub fn status_label(s: AnswerStatus) -> &'static str {
    match s {
        AnswerStatus::Pending => "waiting…",
        AnswerStatus::Streaming => "streaming…",
        AnswerStatus::Done => "done",
        AnswerStatus::Error => "error",
        AnswerStatus::Cancelled => "cancelled",
    }
}

/// Divider text without the leading `─ ` (F-ASK-05): `answer · <agent> · <status>`.
pub fn answer_head(a: &Answer) -> String {
    let agent = a.agent.as_deref().filter(|n| !n.is_empty()).unwrap_or("agent");
    format!("answer · {agent} · {}", status_label(a.status))
}

/// All turns flattened to display lines (`threadBody`): turn 1 message (with code blocks), answers, follow-ups.
pub fn thread_body(c: &Comment, width: usize) -> Vec<BodyLine> {
    let mut out = Vec::new();
    let mut blk = 0usize;
    let fallback = [comments::Turn { message: c.message.clone(), answer: None, prior: Vec::new() }];
    let turns: &[comments::Turn] = if c.turns.is_empty() { &fallback } else { &c.turns };
    for (i, tu) in turns.iter().enumerate() {
        if i == 0 {
            out.extend(rich_lines(&c.message, width, BodyKind::Msg, &mut blk));
        } else {
            for l in textutil::wrap_text(&format!("follow-up: {}", tu.message), width) {
                out.push(plain_line(l, BodyKind::Fu));
            }
        }
        for a in tu.prior.iter().chain(tu.answer.iter()) {
            let live = comments::is_live(a);
            let err = a.status == AnswerStatus::Error;
            let bare = a.text.is_empty() && live;
            let body: String = if err {
                if a.text.is_empty() { "failed".to_string() } else { a.text.clone() }
            } else if bare {
                let t = if a.status == AnswerStatus::Pending {
                    "waiting for agent…"
                } else {
                    "agent working…"
                };
                t.to_string()
            } else {
                a.text.clone()
            };
            out.push(BodyLine { err, ..plain_line(answer_head(a), BodyKind::Div) });
            if body.is_empty() {
                continue;
            }
            if err || bare {
                for l in textutil::wrap_text(&body, width) {
                    out.push(BodyLine {
                        err,
                        live: bare.then_some(a.status),
                        ..plain_line(l, BodyKind::Ans)
                    });
                }
            } else {
                out.extend(rich_lines(&body, width, BodyKind::Ans, &mut blk));
            }
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::comments::AnswerStatus;
    use crate::comments::Turn;
    use crate::comments::testutil::{answer, comment};
    use crate::thread_layout::*;

    #[test]
    fn f_ask_05_thread_body_turns_dividers_and_live_lines() {
        let mut c = comment("q1", 1, 1, "question");
        c.turns[0].prior.push(answer(AnswerStatus::Done, "first"));
        c.turns[0].answer =
            Some(Answer { agent: Some("bot".into()), ..answer(AnswerStatus::Done, "second") });
        c.turns.push(Turn {
            message: "and more".into(),
            answer: Some(answer(AnswerStatus::Pending, "")),
            prior: vec![],
        });
        c.turns.push(Turn {
            message: "again".into(),
            answer: Some(answer(AnswerStatus::Streaming, "")),
            prior: vec![],
        });
        let t: Vec<_> = thread_body(&c, 40).into_iter().map(|l| (l.kind, l.text, l.live)).collect();
        assert_eq!(
            t,
            [
                (BodyKind::Msg, "question".to_string(), None),
                (BodyKind::Div, "answer · agent · done".to_string(), None),
                (BodyKind::Ans, "first".to_string(), None),
                (BodyKind::Div, "answer · bot · done".to_string(), None),
                (BodyKind::Ans, "second".to_string(), None),
                (BodyKind::Fu, "follow-up: and more".to_string(), None),
                (BodyKind::Div, "answer · agent · waiting…".to_string(), None),
                (BodyKind::Ans, "waiting for agent…".to_string(), Some(AnswerStatus::Pending)),
                (BodyKind::Fu, "follow-up: again".to_string(), None),
                (BodyKind::Div, "answer · agent · streaming…".to_string(), None),
                (BodyKind::Ans, "agent working…".to_string(), Some(AnswerStatus::Streaming)),
            ]
        );
    }

    #[test]
    fn f_ask_05_follow_up_lines_never_code_blocks() {
        let mut c = comment("q1", 1, 1, "m");
        c.turns.push(Turn { message: "```\nx\n```".into(), answer: None, prior: vec![] });
        assert!(thread_body(&c, 40).iter().all(|l| l.kind != BodyKind::Btn));
    }
}
