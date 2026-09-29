//! MCP HTTP server: sockets only.
//!
//! Spec: F-MCPSRV-01 (bind 127.0.0.1:<port>, token file IO using core `plan_token`, dir 0700 / file 0600),
//! F-MCPSRV-02 (socket-level: body size cap, connection close), F-MCPSRV-06 (connection drop while parked),
//! Test seams (`reqs` counted on arrival, `done` when response written or connection gone and handler
//! finished), F-MCPUI-03 (drain replies before close), Messages (`cannot listen ...`, `cannot write ...`).
//! Owner: component C (mcp/exec).
//! Must not: interpret requests. It reads a request, sends `Event::McpHttp`, and writes whatever
//! `Effect::HttpReply` says for that `ConnId`. All checks, JSON-RPC and tool logic are in core.

use std::collections::HashMap;
use std::convert::Infallible;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::{Body, Frame, Incoming, SizeHint};
use hyper::header::{HeaderName, HeaderValue};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{oneshot, watch};
use tokio::task::{JoinHandle, JoinSet};
use xplain_core::errors::IoReason;
use xplain_core::event::Event;
use xplain_core::mcp::{ConnId, HttpRequest, HttpResponse, McpEndpoint, port_busy_message};

use crate::exec::{PendingWork, WorkGuard};

/// Request body cap (F-MCPSRV-02.5).
const BODY_CAP: usize = 1_048_576;
/// Bounded wait for in-flight responses at stop (a stuck request is cut after this).
const STOP_GRACE: Duration = Duration::from_millis(2000);

/// Ids stay unique across server restarts so a stale `HttpReply` never hits a new connection.
static NEXT_CONN: AtomicU64 = AtomicU64::new(1);

/// Process-wide request counters reported in every barrier reply (`<reqs>`, `<done>`).
#[derive(Debug, Clone, Default)]
pub struct HttpCounters {
    pub received: Arc<AtomicU64>,
    pub done: Arc<AtomicU64>,
}

impl HttpCounters {
    /// Frozen copy of the current values (independent atomics).
    pub fn snapshot(&self) -> HttpCounters {
        use std::sync::atomic::Ordering::SeqCst;
        HttpCounters {
            received: Arc::new(AtomicU64::new(self.received.load(SeqCst))),
            done: Arc::new(AtomicU64::new(self.done.load(SeqCst))),
        }
    }
}

/// A reply handed to a waiting handler; the guard keeps `PendingWork` open until it is written.
struct Reply {
    response: HttpResponse,
    work: Option<WorkGuard>,
}

struct Shared {
    tx: UnboundedSender<Event>,
    counters: HttpCounters,
    pending: PendingWork,
    parked: Mutex<HashMap<ConnId, oneshot::Sender<Reply>>>,
    stopping: std::sync::atomic::AtomicBool,
}

impl Shared {
    fn parked(&self) -> MutexGuard<'_, HashMap<ConnId, oneshot::Sender<Reply>>> {
        self.parked.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub struct McpServer {
    shared: Arc<Shared>,
    shutdown: watch::Sender<bool>,
    accept: JoinHandle<()>,
}

impl McpServer {
    /// Resolve/create token file, bind, spawn accept loop. `Err` = final user-facing message.
    pub async fn start(
        port: u16,
        state_dir: &str, // token via `crate::token::ensure_token`

        tx: UnboundedSender<Event>,
        counters: HttpCounters,
        pending: PendingWork,
    ) -> Result<(McpServer, McpEndpoint), String> {
        let token = crate::token::ensure_token(state_dir).await?;
        let listener = TcpListener::bind(("127.0.0.1", port)).await.map_err(|e| {
            if e.kind() == io::ErrorKind::AddrInUse {
                port_busy_message(port)
            } else {
                format!("cannot listen on 127.0.0.1:{port}: {}", IoReason::from_io_error(&e).as_str())
            }
        })?;
        let actual = listener.local_addr().map(|a| a.port()).map_err(|e| {
            format!("cannot listen on 127.0.0.1:{port}: {}", IoReason::from_io_error(&e).as_str())
        })?;
        let shared = Arc::new(Shared {
            tx,
            counters,
            pending,
            parked: Mutex::new(HashMap::new()),
            stopping: Default::default(),
        });
        let (shutdown, shutdown_rx) = watch::channel(false);
        let accept = tokio::spawn(accept_loop(listener, shared.clone(), shutdown_rx));
        let endpoint = McpEndpoint { url: format!("http://127.0.0.1:{actual}/mcp"), token, port: actual };
        Ok((McpServer { shared, shutdown, accept }, endpoint))
    }

    /// Write the reply for a parked or fresh request.
    pub fn reply(&self, conn: ConnId, response: HttpResponse) {
        self.reply_with(conn, response, None);
    }

    /// Like [`reply`](Self::reply); when `pending` is given and the request is still waiting, the work
    /// counts as pending until the response was written. Returns whether a waiting request took it.
    pub fn reply_tracked(&self, conn: ConnId, response: HttpResponse, pending: &PendingWork) -> bool {
        self.reply_with(conn, response, Some(pending))
    }

    fn reply_with(&self, conn: ConnId, response: HttpResponse, pending: Option<&PendingWork>) -> bool {
        let Some(slot) = self.shared.parked().remove(&conn) else {
            return false;
        };
        let work = pending.map(PendingWork::guard);
        slot.send(Reply { response, work }).is_ok()
    }

    /// Close listener and connections; resolves after all earlier `reply` calls were written.
    pub async fn stop(self) {
        self.shared.stopping.store(true, Ordering::SeqCst);
        let _ = self.shutdown.send(true);
        let _ = self.accept.await;
    }
}

async fn accept_loop(listener: TcpListener, shared: Arc<Shared>, mut shutdown: watch::Receiver<bool>) {
    let mut conns: JoinSet<()> = JoinSet::new();
    let conn_shutdown = shutdown.clone();
    loop {
        tokio::select! {
            _ = wait_true(&mut shutdown) => break,
            accepted = listener.accept() => match accepted {
                Ok((stream, peer)) => {
                    conns.spawn(serve_conn(stream, peer, shared.clone(), conn_shutdown.clone()));
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(10)).await,
            },
            Some(_) = conns.join_next(), if !conns.is_empty() => {}
        }
    }
    drop(listener);
    let drained =
        tokio::time::timeout(STOP_GRACE, async { while conns.join_next().await.is_some() {} }).await;
    if drained.is_err() {
        conns.abort_all();
        while conns.join_next().await.is_some() {}
    }
}

async fn serve_conn(
    stream: TcpStream,
    peer: SocketAddr,
    shared: Arc<Shared>,
    mut shutdown: watch::Receiver<bool>,
) {
    let _ = stream.set_nodelay(true);
    let stream = Arc::new(stream);
    let io = TokioIo::new(SharedIo(stream.clone()));
    let remote_port = peer.port();
    let svc = service_fn(move |req| handle(req, shared.clone(), stream.clone(), remote_port));
    let mut builder = http1::Builder::new();
    builder.header_read_timeout(None);
    let conn = builder.serve_connection(io, svc);
    tokio::pin!(conn);
    tokio::select! {
        _ = conn.as_mut() => return,
        _ = wait_true(&mut shutdown) => conn.as_mut().graceful_shutdown(),
    }
    let _ = conn.await;
}

async fn wait_true(rx: &mut watch::Receiver<bool>) {
    let _ = rx.wait_for(|v| *v).await;
}

/// Resolves when the peer closed or the socket failed. Peeks only, so hyper still sees every byte.
async fn peer_gone(stream: &TcpStream) {
    let mut probe = [0u8; 1];
    loop {
        if stream.readable().await.is_err() {
            return;
        }
        match stream.peek(&mut probe).await {
            Ok(0) | Err(_) => return,
            // Pipelined bytes are waiting: not a disconnect, look again shortly.
            Ok(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
}

/// Increments `done` exactly once, when dropped (response written, or handler gone).
struct DoneGuard(Arc<AtomicU64>);

impl Drop for DoneGuard {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// Response body that keeps its guards alive until hyper is finished with it (written or dropped).
struct GuardedBody {
    inner: Full<Bytes>,
    _done: DoneGuard,
    _work: Option<WorkGuard>,
}

impl Body for GuardedBody {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Infallible>>> {
        Pin::new(&mut self.inner).poll_frame(cx)
    }
    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }
    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

async fn handle(
    req: Request<Incoming>,
    shared: Arc<Shared>,
    stream: Arc<TcpStream>,
    remote_port: u16,
) -> Result<Response<GuardedBody>, io::Error> {
    // Counted on arrival, before the body is read. The pending guard comes first so a request that is
    // counted is always covered by pending work until its event is queued (barrier `reqs` snapshot).
    let work = shared.pending.guard();
    shared.counters.received.fetch_add(1, Ordering::SeqCst);
    let done = DoneGuard(shared.counters.done.clone());
    let conn = ConnId(NEXT_CONN.fetch_add(1, Ordering::SeqCst));

    let (parts, mut body) = req.into_parts();
    let (data, body_too_large) = read_capped(&mut body).await?;
    let path = parts.uri.path_and_query().map_or_else(|| parts.uri.to_string(), |p| p.as_str().to_string());
    let headers = parts
        .headers
        .iter()
        .map(|(k, v)| (k.as_str().to_ascii_lowercase(), String::from_utf8_lossy(v.as_bytes()).into_owned()))
        .collect();
    let request = HttpRequest {
        conn,
        method: parts.method.as_str().to_string(),
        path,
        headers,
        body: data,
        body_too_large,
        remote_port,
        entropy: rand::random(),
    };

    let (slot, mut waiting) = oneshot::channel();
    shared.parked().insert(conn, slot);
    // From here on, dropping the handler while still parked (hyper drops it when the connection dies)
    // reports the disconnect.
    let _parked = ParkedGuard { shared: shared.clone(), conn };
    if shared.tx.send(Event::McpHttp(request)).is_err() {
        return Err(io::Error::other("event channel closed"));
    }
    drop(work);

    let reply = tokio::select! {
        r = &mut waiting => r.ok(),
        _ = peer_gone(&stream) => waiting.try_recv().ok(),
    };
    let Some(Reply { response, work }) = reply else {
        return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "connection gone"));
    };
    Ok(build_response(response, done, work))
}

/// Reports `McpConnClosed` when the handler ends while its request is still parked (no reply taken yet).
struct ParkedGuard {
    shared: Arc<Shared>,
    conn: ConnId,
}

impl Drop for ParkedGuard {
    fn drop(&mut self) {
        let unanswered = self.shared.parked().remove(&self.conn).is_some();
        if unanswered && !self.shared.stopping.load(Ordering::SeqCst) {
            let _ = self.shared.tx.send(Event::McpConnClosed(self.conn));
        }
    }
}

/// Read the body up to [`BODY_CAP`]; beyond it stop reading and flag it (nothing more is buffered).
async fn read_capped(body: &mut Incoming) -> Result<(Vec<u8>, bool), io::Error> {
    let mut data: Vec<u8> = Vec::new();
    while let Some(frame) = body.frame().await {
        let frame = frame.map_err(io::Error::other)?;
        if let Ok(chunk) = frame.into_data() {
            let room = BODY_CAP - data.len();
            if chunk.len() > room {
                data.extend_from_slice(&chunk[..room]);
                return Ok((data, true));
            }
            data.extend_from_slice(&chunk);
        }
    }
    Ok((data, false))
}

fn build_response(r: HttpResponse, done: DoneGuard, work: Option<WorkGuard>) -> Response<GuardedBody> {
    let mut res =
        Response::new(GuardedBody { inner: Full::new(Bytes::from(r.body)), _done: done, _work: work });
    *res.status_mut() = StatusCode::from_u16(r.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    for (k, v) in r.headers {
        if let (Ok(name), Ok(value)) = (HeaderName::from_bytes(k.as_bytes()), HeaderValue::from_str(&v)) {
            res.headers_mut().append(name, value);
        }
    }
    res
}

/// Stream shared between hyper and [`peer_gone`] (which only peeks), so a parked request notices a
/// disconnect even though hyper does not read while a response is pending.
struct SharedIo(Arc<TcpStream>);

impl AsyncRead for SharedIo {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        loop {
            std::task::ready!(self.0.poll_read_ready(cx))?;
            match self.0.try_read(buf.initialize_unfilled()) {
                Ok(n) => {
                    buf.advance(n);
                    return Poll::Ready(Ok(()));
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => return Poll::Ready(Err(e)),
            }
        }
    }
}

impl AsyncWrite for SharedIo {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        loop {
            std::task::ready!(self.0.poll_write_ready(cx))?;
            match self.0.try_write(buf) {
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                r => return Poll::Ready(r),
            }
        }
    }
    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    /// Writes are unbuffered; the socket closes when the last handle drops.
    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering::SeqCst;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

    struct Fixture {
        server: McpServer,
        port: u16,
        rx: UnboundedReceiver<Event>,
        counters: HttpCounters,
        pending: PendingWork,
        dir: std::path::PathBuf,
    }

    async fn fixture(name: &str) -> Fixture {
        let dir = std::env::temp_dir().join(format!("xplain-http-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("mcp.json"), "{\"token\":\"0123456789abcdef0123\"}");
        let (tx, rx) = unbounded_channel();
        let counters = HttpCounters::default();
        let pending = PendingWork::default();
        let started =
            McpServer::start(0, &dir.to_string_lossy(), tx, counters.clone(), pending.clone()).await;
        let (server, ep) = match started {
            Ok(x) => x,
            Err(e) => unreachable!("start failed: {e}"),
        };
        assert_eq!(ep.url, format!("http://127.0.0.1:{}/mcp", ep.port));
        assert_eq!(ep.token, "0123456789abcdef0123");
        Fixture { server, port: ep.port, rx, counters, pending, dir }
    }

    async fn next_http(rx: &mut UnboundedReceiver<Event>) -> HttpRequest {
        match tokio::time::timeout(Duration::from_secs(5), rx.recv()).await {
            Ok(Some(Event::McpHttp(r))) => r,
            other => unreachable!("expected McpHttp, got {other:?}"),
        }
    }

    fn resp(status: u16, body: &str) -> HttpResponse {
        HttpResponse {
            status,
            headers: vec![("content-type".into(), "application/json".into())],
            body: body.as_bytes().to_vec(),
        }
    }

    async fn read_all(s: &mut TcpStream) -> String {
        let mut out = Vec::new();
        let _ = tokio::time::timeout(Duration::from_secs(5), s.read_to_end(&mut out)).await;
        String::from_utf8_lossy(&out).into_owned()
    }

    async fn settle(c: &HttpCounters, done: u64) {
        for _ in 0..200 {
            if c.done.load(SeqCst) >= done {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[tokio::test]
    async fn request_roundtrip_fields_and_counters() {
        let mut f = fixture("rt").await;
        let mut s = TcpStream::connect(("127.0.0.1", f.port)).await.unwrap_or_else(|e| unreachable!("{e}"));
        let local = s.local_addr().map(|a| a.port()).unwrap_or(0);
        let body = "{\"a\":1}";
        let req = format!(
            "POST /mcp?x=1 HTTP/1.1\r\nHost: localhost\r\nX-Custom: Val\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = s.write_all(req.as_bytes()).await;
        let r = next_http(&mut f.rx).await;
        assert_eq!(f.counters.received.load(SeqCst), 1);
        assert_eq!(r.method, "POST");
        assert_eq!(r.path, "/mcp?x=1");
        assert_eq!(r.body, body.as_bytes());
        assert!(!r.body_too_large);
        assert_eq!(r.remote_port, local);
        assert!(r.headers.contains(&("x-custom".to_string(), "Val".to_string())));
        assert!(r.headers.contains(&("host".to_string(), "localhost".to_string())));
        assert_eq!(f.pending.count(), 0);
        f.server.reply(r.conn, resp(200, "{\"ok\":true}"));
        let text = read_all(&mut s).await;
        assert!(text.starts_with("HTTP/1.1 200 OK"), "{text}");
        assert!(text.contains("content-type: application/json"));
        assert!(text.ends_with("{\"ok\":true}"));
        settle(&f.counters, 1).await;
        assert_eq!(f.counters.done.load(SeqCst), 1);
        f.server.stop().await;
        let _ = std::fs::remove_dir_all(&f.dir);
    }

    #[tokio::test]
    async fn entropy_differs_and_conn_ids_unique() {
        let mut f = fixture("ent").await;
        let mut socks = Vec::new();
        for _ in 0..2 {
            let mut s =
                TcpStream::connect(("127.0.0.1", f.port)).await.unwrap_or_else(|e| unreachable!("{e}"));
            let _ = s.write_all(b"GET /x HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
            socks.push(s);
        }
        let a = next_http(&mut f.rx).await;
        let b = next_http(&mut f.rx).await;
        assert_ne!(a.conn, b.conn);
        assert_ne!(a.entropy, b.entropy);
        f.server.reply(a.conn, resp(200, "a"));
        f.server.reply(b.conn, resp(200, "b"));
        f.server.stop().await;
        let _ = std::fs::remove_dir_all(&f.dir);
    }

    #[tokio::test]
    async fn parked_connection_drop_reports_closed() {
        let mut f = fixture("drop").await;
        let mut s = TcpStream::connect(("127.0.0.1", f.port)).await.unwrap_or_else(|e| unreachable!("{e}"));
        let _ = s.write_all(b"POST /mcp HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n").await;
        let r = next_http(&mut f.rx).await;
        drop(s);
        match tokio::time::timeout(Duration::from_secs(5), f.rx.recv()).await {
            Ok(Some(Event::McpConnClosed(c))) => assert_eq!(c, r.conn),
            other => unreachable!("expected McpConnClosed, got {other:?}"),
        }
        settle(&f.counters, 1).await;
        assert_eq!(f.counters.done.load(SeqCst), 1);
        assert_eq!(f.counters.received.load(SeqCst), 1);
        // late reply is ignored
        f.server.reply(r.conn, resp(200, "late"));
        f.server.stop().await;
        let _ = std::fs::remove_dir_all(&f.dir);
    }

    #[tokio::test]
    async fn oversize_body_flagged_not_buffered() {
        let mut f = fixture("big").await;
        let mut s = TcpStream::connect(("127.0.0.1", f.port)).await.unwrap_or_else(|e| unreachable!("{e}"));
        let total = BODY_CAP + 500_000;
        let head = format!("POST /mcp HTTP/1.1\r\nHost: localhost\r\nContent-Length: {total}\r\n\r\n");
        let _ = s.write_all(head.as_bytes()).await;
        let chunk = vec![b'a'; 64 * 1024];
        let mut sent = 0;
        while sent < BODY_CAP + 1000 {
            if s.write_all(&chunk).await.is_err() {
                break;
            }
            sent += chunk.len();
        }
        let r = next_http(&mut f.rx).await;
        assert!(r.body_too_large);
        assert_eq!(r.body.len(), BODY_CAP);
        let mut rep = resp(413, "{\"error\":\"body too large\"}");
        rep.headers.push(("connection".into(), "close".into()));
        f.server.reply(r.conn, rep);
        let mut buf = [0u8; 64];
        let n = tokio::time::timeout(Duration::from_secs(5), s.read(&mut buf)).await;
        if let Ok(Ok(n)) = n {
            assert!(String::from_utf8_lossy(&buf[..n]).starts_with("HTTP/1.1 413"));
        }
        f.server.stop().await;
        let _ = std::fs::remove_dir_all(&f.dir);
    }

    #[tokio::test]
    async fn stop_drains_earlier_replies() {
        let mut f = fixture("stop").await;
        let mut s = TcpStream::connect(("127.0.0.1", f.port)).await.unwrap_or_else(|e| unreachable!("{e}"));
        let _ = s.write_all(b"POST /mcp HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n").await;
        let r = next_http(&mut f.rx).await;
        assert!(f.server.reply_tracked(r.conn, resp(200, "{\"status\":\"closed\"}"), &f.pending));
        let port = f.port;
        f.server.stop().await;
        assert_eq!(f.pending.count(), 0);
        let text = read_all(&mut s).await;
        assert!(text.starts_with("HTTP/1.1 200 OK"), "{text}");
        assert!(text.ends_with("{\"status\":\"closed\"}"));
        assert!(TcpStream::connect(("127.0.0.1", port)).await.is_err());
        let _ = std::fs::remove_dir_all(&f.dir);
    }

    #[tokio::test]
    async fn port_busy_and_zero_port() {
        let f = fixture("busy").await;
        let (tx, _rx) = unbounded_channel();
        let e = McpServer::start(
            f.port,
            &f.dir.to_string_lossy(),
            tx,
            HttpCounters::default(),
            PendingWork::default(),
        )
        .await
        .err()
        .unwrap_or_default();
        assert_eq!(e, port_busy_message(f.port));
        f.server.stop().await;
        let _ = std::fs::remove_dir_all(&f.dir);
    }
}
